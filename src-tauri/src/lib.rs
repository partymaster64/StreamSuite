mod config;
mod connections;
mod hub;
mod server;
mod state;
mod tiktok;
mod tiktok_web;
mod tray;
mod twitch;
mod youtube;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use hub::{Event, Hub, Platform, User};
use state::{AppState, Shared};

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn platform(p: &str) -> CmdResult<Platform> {
    Platform::parse(p).ok_or_else(|| format!("Unbekannte Plattform: {p}"))
}

// ---------------------------------------------------------------------------
// Einstellungen
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_settings(st: tauri::State<Shared>) -> Value {
    st.store.get().redacted()
}

fn merge(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(d), Value::Object(s)) => {
            for (k, v) in s {
                merge(d.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (d, s) => *d = s.clone(),
    }
}

/// Teil-Update der Einstellungen. Tokens & Kontodaten können hierüber nicht verändert werden.
#[tauri::command]
fn save_settings(st: tauri::State<Shared>, patch: Value) -> CmdResult<Value> {
    let mut patch = patch;
    const PROTECTED: &[&str] = &[
        "access_token",
        "refresh_token",
        "expires_at",
        "user_id",
        "login",
        "display_name",
        "channel_id",
        "channel_title",
        "authorized",
        "enabled",
        "session_id",
        "credentials_ok",
    ];
    if let Some(obj) = patch.as_object_mut() {
        for section in ["twitch", "youtube", "tiktok"] {
            if let Some(Value::Object(sec)) = obj.get_mut(section) {
                for k in PROTECTED {
                    sec.remove(*k);
                }
            }
        }
    }
    let current = st.store.get();
    let mut v = serde_json::to_value(&current).map_err(err)?;
    merge(&mut v, &patch);
    let next: config::Settings = serde_json::from_value(v).map_err(err)?;
    let saved = st.store.update(|s| *s = next);
    Ok(saved.redacted())
}

// ---------------------------------------------------------------------------
// Verbindungen
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_statuses(st: tauri::State<Shared>) -> Value {
    json!(st.hub.statuses())
}

#[tauri::command]
fn connect_platform(st: tauri::State<Shared>, platform: String) -> CmdResult<()> {
    let p = self::platform(&platform)?;
    let cfg = st.store.get();
    let ready = match p {
        Platform::Twitch => cfg.twitch.access_token.is_some(),
        Platform::Youtube => cfg.youtube.refresh_token.is_some(),
        Platform::Tiktok => !cfg.tiktok.username.trim().is_empty(),
        Platform::System => false,
    };
    if !ready {
        return Err("Bitte zuerst anmelden".into());
    }
    connections::set_enabled(&st, p, true);
    connections::start(&st, p);
    Ok(())
}

#[tauri::command]
fn disconnect_platform(st: tauri::State<Shared>, platform: String) -> CmdResult<()> {
    connections::disconnect(&st, self::platform(&platform)?);
    Ok(())
}

// async: Cookie-Operationen dürfen unter Windows nicht im synchronen Command laufen
#[tauri::command]
async fn logout_platform(st: tauri::State<'_, Shared>, platform: String) -> CmdResult<()> {
    connections::logout(st.inner(), self::platform(&platform)?);
    Ok(())
}

#[tauri::command]
async fn tiktok_login(st: tauri::State<'_, Shared>) -> CmdResult<()> {
    tiktok_web::open_login(st.inner()).map_err(err)
}

#[tauri::command]
async fn tiktok_show_window(st: tauri::State<'_, Shared>) -> CmdResult<()> {
    tiktok_web::show_chat_window(st.inner()).map_err(err)
}

/// Benutzername manuell setzen (falls der Login im eingebetteten Fenster nicht klappt).
#[tauri::command]
fn tiktok_set_username(st: tauri::State<Shared>, username: String) -> CmdResult<()> {
    let user = username.trim().trim_start_matches('@').to_string();
    if user.is_empty() {
        return Err("Benutzername fehlt".into());
    }
    st.store.update(|s| {
        s.tiktok.username = user;
        s.tiktok.enabled = true;
    });
    connections::start(&st, Platform::Tiktok);
    Ok(())
}

#[tauri::command]
async fn twitch_device_auth(st: tauri::State<'_, Shared>) -> CmdResult<twitch::DeviceAuth> {
    let st = st.inner().clone();
    let auth = twitch::start_device_auth(st.clone()).await.map_err(err)?;
    let _ = st.app.opener().open_url(auth.verification_uri.clone(), None::<&str>);
    Ok(auth)
}

#[tauri::command]
async fn youtube_auth(st: tauri::State<'_, Shared>) -> CmdResult<String> {
    youtube::start_auth(st.inner().clone()).await.map_err(err)
}

// ---------------------------------------------------------------------------
// Events / Chat
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_history(st: tauri::State<Shared>) -> Value {
    st.hub.history()
}

#[tauri::command]
fn get_stats(st: tauri::State<Shared>) -> Value {
    st.hub.stats_json()
}

#[tauri::command]
fn clear_history(st: tauri::State<Shared>) {
    st.hub.clear_history();
}

/// Sendet eine Chatnachricht an eine oder alle Plattformen. TikTok unterstützt kein Senden.
pub async fn send_chat_all(st: &Shared, target: &str, message: &str) -> Value {
    let message = message.trim();
    let mut out = serde_json::Map::new();
    if message.is_empty() {
        return Value::Object(out);
    }
    let statuses: HashMap<Platform, bool> = st
        .hub
        .statuses()
        .into_iter()
        .map(|s| (s.platform, s.state == "connected"))
        .collect();
    let want = |p: Platform, key: &str| (target == "all" && statuses.get(&p).copied().unwrap_or(false)) || target == key;
    if want(Platform::Twitch, "twitch") {
        let r = twitch::send_chat(st, message).await;
        out.insert("twitch".into(), result_json(r));
    }
    if want(Platform::Youtube, "youtube") && (target == "youtube" || st.youtube_live.lock().is_some()) {
        let r = youtube::send_chat(st, message).await;
        out.insert("youtube".into(), result_json(r));
    }
    let tiktok_logged_in = st.store.get().tiktok.session_id.is_some();
    let tiktok_live = st.hub.statuses().iter().any(|s| s.platform == Platform::Tiktok && s.live);
    if target == "tiktok" || (target == "all" && tiktok_logged_in && tiktok_live) {
        let r = tiktok_web::send_chat(st, message).await;
        out.insert("tiktok".into(), result_json(r));
    }
    Value::Object(out)
}

fn result_json(r: anyhow::Result<()>) -> Value {
    match r {
        Ok(()) => json!({ "ok": true }),
        Err(e) => json!({ "ok": false, "error": format!("{e:#}") }),
    }
}

#[tauri::command]
async fn send_chat(st: tauri::State<'_, Shared>, platform: String, message: String) -> CmdResult<Value> {
    Ok(send_chat_all(st.inner(), &platform, &message).await)
}

// ---------------------------------------------------------------------------
// Stream-Infos (Titel, Kategorie, Beschreibung)
// ---------------------------------------------------------------------------

#[tauri::command]
async fn get_stream_info(st: tauri::State<'_, Shared>) -> CmdResult<Value> {
    let st = st.inner().clone();
    let cfg = st.store.get();
    let (tw, yt) = tokio::join!(
        async {
            if cfg.twitch.access_token.is_none() {
                return json!({ "connected": false });
            }
            match twitch::get_channel_info(&st).await {
                Ok(v) => json!({ "connected": true, "info": v }),
                Err(e) => json!({ "connected": true, "error": e.to_string() }),
            }
        },
        async {
            if cfg.youtube.refresh_token.is_none() {
                return json!({ "connected": false });
            }
            match youtube::get_info(&st).await {
                Ok(v) => json!({ "connected": true, "info": v }),
                Err(e) => json!({ "connected": true, "error": e.to_string() }),
            }
        }
    );
    Ok(json!({ "twitch": tw, "youtube": yt }))
}

#[derive(Deserialize)]
struct StreamInfoUpdate {
    platforms: Vec<String>,
    title: Option<String>,
    description: Option<String>,
    twitch_category_id: Option<String>,
    youtube_category_id: Option<String>,
}

#[tauri::command]
async fn update_stream_info(st: tauri::State<'_, Shared>, update: StreamInfoUpdate) -> CmdResult<Value> {
    let st = st.inner().clone();
    let title = update.title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let mut out = serde_json::Map::new();
    let has = |p: &str| update.platforms.iter().any(|x| x == p);
    let (tw, yt) = tokio::join!(
        async {
            if !has("twitch") {
                return None;
            }
            Some(twitch::update_channel_info(&st, title.clone(), update.twitch_category_id.clone().filter(|c| !c.is_empty())).await)
        },
        async {
            if !has("youtube") {
                return None;
            }
            Some(youtube::update_info(&st, title.clone(), update.description.clone(), update.youtube_category_id.clone()).await)
        }
    );
    if let Some(r) = tw {
        out.insert("twitch".into(), result_json(r));
    }
    if let Some(r) = yt {
        out.insert("youtube".into(), result_json(r));
    }
    Ok(Value::Object(out))
}

#[tauri::command]
async fn search_twitch_categories(st: tauri::State<'_, Shared>, query: String) -> CmdResult<Value> {
    if query.trim().is_empty() {
        return Ok(json!([]));
    }
    twitch::search_categories(st.inner(), query.trim()).await.map_err(err)
}

#[tauri::command]
async fn youtube_categories(st: tauri::State<'_, Shared>) -> CmdResult<Value> {
    youtube::categories(st.inner()).await.map_err(err)
}

// ---------------------------------------------------------------------------
// Overlays & Server
// ---------------------------------------------------------------------------

pub fn list_custom(st: &Shared) -> Value {
    let mut list = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&st.overlays_dir) {
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() && path.join("index.html").exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    list.push(name.to_string());
                }
            }
        }
    }
    list.sort();
    json!(list)
}

#[tauri::command]
fn server_info(st: tauri::State<Shared>) -> Value {
    let port = *st.server_port.lock();
    json!({
        "port": port,
        "base_url": port.map(|p| format!("http://127.0.0.1:{p}")),
        "overlays_dir": st.overlays_dir.to_string_lossy(),
        "custom": list_custom(&st),
        "tray": tray::available(),
    })
}

#[tauri::command]
fn open_overlays_dir(st: tauri::State<Shared>) -> CmdResult<()> {
    let _ = std::fs::create_dir_all(&st.overlays_dir);
    st.app
        .opener()
        .open_path(st.overlays_dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(err)
}

#[tauri::command]
fn restore_templates(st: tauri::State<Shared>) {
    server::install_templates(&st.overlays_dir);
}

// ---------------------------------------------------------------------------
// Test-Alerts
// ---------------------------------------------------------------------------

fn demo_user(name: &str) -> User {
    User {
        id: format!("test-{}", name.to_lowercase()),
        name: name.to_lowercase(),
        display_name: name.to_string(),
        color: Some("#a78bfa".into()),
        ..Default::default()
    }
}

#[tauri::command]
fn test_event(st: tauri::State<Shared>, kind: String, platform: String) -> CmdResult<()> {
    let p = Platform::parse(&platform).unwrap_or(Platform::Twitch);
    let user = demo_user("TestUser");
    let mut ev = match kind.as_str() {
        "chat" => Event::new(p, "chat").user(user).message("Hallo aus StreamSuite! Das ist eine Testnachricht 👋"),
        "follow" => Event::new(p, "follow").user(user),
        "subscribe" => {
            let mut e = Event::new(p, "subscribe").user(user).amount(1.0);
            if p == Platform::Twitch {
                e.tier = Some("Tier 1".into());
            }
            e
        }
        "resub" => Event::new(p, "resub").user(user).amount(12.0).message("Ein Jahr schon!"),
        "gift_sub" => Event::new(p, "gift_sub").user(user).amount(5.0),
        "membership" => {
            let mut e = Event::new(Platform::Youtube, "membership").user(user);
            e.tier = Some("Mitglied".into());
            e
        }
        "cheer" => Event::new(Platform::Twitch, "cheer").user(user).amount(500.0).message("Cheer500 Weiter so!"),
        "superchat" => {
            let mut e = Event::new(Platform::Youtube, "superchat").user(user).amount(5.0).message("Toller Stream!");
            e.currency = Some("EUR".into());
            e.amount_display = Some("5,00 €".into());
            e
        }
        "gift" => {
            let mut e = Event::new(Platform::Tiktok, "gift").user(user).amount(10.0);
            e.coins = Some(10);
            e.gift_name = Some("Rose".into());
            e
        }
        "share" => Event::new(Platform::Tiktok, "share").user(user),
        "raid" => Event::new(Platform::Twitch, "raid").user(user).amount(42.0),
        "like" => {
            let mut e = Event::new(Platform::System, "like").user(user.clone()).amount(128.0);
            e.data = json!({
                "platforms": { "tiktok": 100, "youtube": 28 },
                "top": [{ "user": user, "platform": "tiktok", "count": 100 }]
            });
            e
        }
        other => return Err(format!("Unbekannter Test-Typ: {other}")),
    };
    ev.test = true;
    st.hub.publish(ev);
    Ok(())
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Zweiter Start holt nur das vorhandene Fenster nach vorne
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .app_handle()
                    .try_state::<Shared>()
                    .map(|st| st.store.get().close_to_tray)
                    .unwrap_or(false);
                if close_to_tray {
                    // Im Hintergrund weiterlaufen (Overlays & Verbindungen bleiben aktiv)
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let overlays_dir = data_dir.join("overlays");
            std::fs::create_dir_all(&overlays_dir)?;
            server::install_templates(&overlays_dir);

            let http = reqwest::Client::builder()
                .user_agent(concat!("StreamSuite/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(20))
                .build()?;

            let st: Shared = Arc::new(AppState {
                app: handle.clone(),
                store: config::Store::load(config_dir.join("settings.json")),
                hub: Hub::new(handle.clone()),
                http,
                overlays_dir,
                tasks: Mutex::new(HashMap::new()),
                youtube_live: Mutex::new(None),
                twitch_refresh: tokio::sync::Mutex::new(()),
                youtube_refresh: tokio::sync::Mutex::new(()),
                server_port: Mutex::new(None),
                tiktok_pending: Mutex::new(HashMap::new()),
                tiktok_detected_user: Mutex::new(None),
            });
            app.manage(st.clone());

            tauri::async_runtime::spawn(server::start(st.clone()));

            // Kombinierte Likes periodisch als Alert ausgeben
            let st_likes = st.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let a = st_likes.store.get().alerts;
                    tokio::time::sleep(Duration::from_secs(a.like_interval_secs.max(3))).await;
                    st_likes.hub.flush_likes(a.like_min_count);
                }
            });

            connections::start_enabled(&st);
            if let Err(e) = tray::setup(&handle) {
                log::warn!("Tray-Icon konnte nicht erstellt werden: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_statuses,
            connect_platform,
            disconnect_platform,
            logout_platform,
            twitch_device_auth,
            youtube_auth,
            get_history,
            get_stats,
            clear_history,
            send_chat,
            get_stream_info,
            update_stream_info,
            search_twitch_categories,
            youtube_categories,
            server_info,
            open_overlays_dir,
            restore_templates,
            test_event,
            tiktok_login,
            tiktok_show_window,
            tiktok_set_username,
        ])
        .build(tauri::generate_context!())
        .expect("Fehler beim Starten von StreamSuite")
        .run(|_app, _event| {
            // macOS: Klick auf das Dock-Icon zeigt das versteckte Fenster wieder an
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                tray::show_main(_app);
            }
        });
}
