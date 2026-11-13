//! Twitch-Integration über offizielle APIs:
//! - Anmeldung: OAuth Device Code Flow (kein Client-Secret nötig, ideal für lokale Desktop-Apps)
//! - Echtzeit: EventSub WebSocket (Chat, Follows, Subs, Bits, Raids)
//! - Helix API: Kanalinfos (Titel/Kategorie), Zuschauerzahl, Chat senden

use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use reqwest::Method;
use serde::Serialize;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use crate::hub::{Event, Part, Platform, User};
use crate::state::{Shared, b, opt_s, s, u};

const HELIX: &str = "https://api.twitch.tv/helix";
const EVENTSUB_URL: &str = "wss://eventsub.wss.twitch.tv/ws?keepalive_timeout_seconds=30";
pub const SCOPES: &str = "user:read:chat user:write:chat moderator:read:followers channel:read:subscriptions bits:read channel:manage:broadcast moderator:manage:banned_users moderator:manage:chat_messages channel:manage:moderators channel:manage:vips";

// ---------------------------------------------------------------------------
// HTTP / Auth
// ---------------------------------------------------------------------------

pub async fn helix(st: &Shared, method: Method, path: &str, query: &[(&str, String)], body: Option<Value>) -> anyhow::Result<Value> {
    for attempt in 0..2 {
        let all = st.store.get();
        let client_id = all.twitch_client_id();
        let token = all.twitch.access_token.clone().ok_or_else(|| anyhow!("Twitch ist nicht verbunden"))?;
        let mut req = st
            .http
            .request(method.clone(), format!("{HELIX}/{path}"))
            .query(query)
            .bearer_auth(&token)
            .header("Client-Id", &client_id);
        if let Some(b) = &body {
            req = req.json(b);
        }
        let res = req.send().await?;
        let status = res.status();
        if status.as_u16() == 401 && attempt == 0 {
            refresh(st, &token).await?;
            continue;
        }
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            let msg = serde_json::from_str::<Value>(&text).map(|v| s(&v, "/message")).unwrap_or(text);
            bail!("Twitch API {}: {}", status.as_u16(), msg);
        }
        return Ok(if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text)? });
    }
    bail!("Twitch-Authentifizierung fehlgeschlagen")
}

async fn refresh(st: &Shared, old_token: &str) -> anyhow::Result<()> {
    let _guard = st.twitch_refresh.lock().await;
    let all = st.store.get();
    let client_id = all.twitch_client_id();
    let cfg = all.twitch;
    if cfg.access_token.as_deref() != Some(old_token) {
        return Ok(()); // bereits von einem anderen Task erneuert
    }
    let rt = cfg.refresh_token.clone().ok_or_else(|| anyhow!("Kein Refresh-Token – bitte neu anmelden"))?;
    let res = st
        .http
        .post("https://id.twitch.tv/oauth2/token")
        .form(&[("client_id", client_id.as_str()), ("grant_type", "refresh_token"), ("refresh_token", rt.as_str())])
        .send()
        .await?;
    let v: Value = res.json().await?;
    match v.get("access_token").and_then(|t| t.as_str()) {
        Some(at) => {
            let at = at.to_string();
            let new_rt = opt_s(&v, "/refresh_token");
            st.store.update(|s| {
                s.twitch.access_token = Some(at);
                if new_rt.is_some() {
                    s.twitch.refresh_token = new_rt;
                }
            });
            Ok(())
        }
        None => {
            st.store.update(|s| {
                s.twitch.access_token = None;
                s.twitch.refresh_token = None;
            });
            bail!("Twitch-Sitzung abgelaufen – bitte neu verbinden ({})", s(&v, "/message"))
        }
    }
}

#[derive(Serialize, Clone)]
pub struct DeviceAuth {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

/// Startet den Device-Code-Flow. Der Nutzer bestätigt den Code im Browser,
/// die App pollt im Hintergrund bis das Token da ist.
pub async fn start_device_auth(st: Shared) -> anyhow::Result<DeviceAuth> {
    let client_id = st.store.get().twitch_client_id();
    if client_id.is_empty() {
        bail!("In diesem Build ist keine Twitch Client-ID hinterlegt (siehe Einstellungen → Erweitert)");
    }
    let res = st
        .http
        .post("https://id.twitch.tv/oauth2/device")
        .form(&[("client_id", client_id.as_str()), ("scopes", SCOPES)])
        .send()
        .await?;
    let v: Value = res.json().await?;
    let device_code = opt_s(&v, "/device_code").ok_or_else(|| anyhow!("Twitch: {}", s(&v, "/message")))?;
    let auth = DeviceAuth {
        user_code: s(&v, "/user_code"),
        verification_uri: s(&v, "/verification_uri"),
        expires_in: u(&v, "/expires_in"),
    };
    let mut interval = u(&v, "/interval").max(1);
    let expires = auth.expires_in.max(60);

    let st2 = st.clone();
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            if started.elapsed().as_secs() > expires {
                auth_result(&st2, false, "Der Code ist abgelaufen. Bitte erneut versuchen.");
                return;
            }
            let res = st2
                .http
                .post("https://id.twitch.tv/oauth2/token")
                .form(&[
                    ("client_id", client_id.as_str()),
                    ("scopes", SCOPES),
                    ("device_code", device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await;
            let Ok(res) = res else { continue };
            let v: Value = res.json().await.unwrap_or_default();
            if let Some(at) = opt_s(&v, "/access_token") {
                let rt = opt_s(&v, "/refresh_token");
                st2.store.update(|s| {
                    s.twitch.access_token = Some(at);
                    s.twitch.refresh_token = rt;
                    s.twitch.enabled = true;
                });
                match helix(&st2, Method::GET, "users", &[], None).await {
                    Ok(users) => {
                        let user = &users["data"][0];
                        let (id, login, name) = (s(user, "/id"), s(user, "/login"), s(user, "/display_name"));
                        st2.store.update(|s| {
                            s.twitch.user_id = Some(id);
                            s.twitch.login = Some(login);
                            s.twitch.display_name = Some(name.clone());
                        });
                        auth_result(&st2, true, &format!("Verbunden als {name}"));
                        crate::connections::start(&st2, Platform::Twitch);
                    }
                    Err(e) => auth_result(&st2, false, &format!("Benutzer konnte nicht geladen werden: {e}")),
                }
                return;
            }
            match s(&v, "/message").as_str() {
                "authorization_pending" => {}
                "slow_down" => interval += 5,
                other => {
                    auth_result(&st2, false, &format!("Anmeldung fehlgeschlagen: {other}"));
                    return;
                }
            }
        }
    });
    Ok(auth)
}

fn auth_result(st: &Shared, ok: bool, msg: &str) {
    st.hub.emit_raw("ss://auth", json!({ "platform": "twitch", "ok": ok, "message": msg }));
}

// ---------------------------------------------------------------------------
// EventSub WebSocket
// ---------------------------------------------------------------------------

pub async fn run(st: Shared) {
    let mut backoff = 2u64;
    let mut url = EVENTSUB_URL.to_string();
    let mut reconnect = false;
    loop {
        let cfg = st.store.get().twitch;
        if cfg.access_token.is_none() || cfg.user_id.is_none() {
            st.hub.set_status(Platform::Twitch, "unconfigured", false, "Nicht angemeldet", None);
            return;
        }
        st.hub
            .set_status(Platform::Twitch, "connecting", false, "Verbinde mit Twitch EventSub …", cfg.display_name.clone());
        match session(&st, &url, reconnect).await {
            Ok(next) => {
                url = next;
                reconnect = true;
                backoff = 2;
            }
            Err(e) => {
                let msg = format!("{e:#}");
                if st.store.get().twitch.access_token.is_none() {
                    st.hub.set_status(Platform::Twitch, "error", false, msg, cfg.display_name);
                    return;
                }
                st.hub.set_status(
                    Platform::Twitch,
                    "error",
                    false,
                    format!("{msg} – neuer Versuch in {backoff}s"),
                    cfg.display_name,
                );
                st.hub.reset_platform_stats(Platform::Twitch);
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(60);
                url = EVENTSUB_URL.to_string();
                reconnect = false;
            }
        }
    }
}

/// Eine WebSocket-Sitzung. Gibt bei `session_reconnect` die neue URL zurück.
async fn session(st: &Shared, url: &str, reconnect: bool) -> anyhow::Result<String> {
    let (ws, _) = tokio_tungstenite::connect_async(url).await.context("WebSocket-Verbindung fehlgeschlagen")?;
    let (mut write, mut read) = ws.split();
    let mut keepalive = 30u64;
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    let mut connected_msg = String::from("Verbunden");
    let mut welcomed = false;

    loop {
        tokio::select! {
            next = tokio::time::timeout(Duration::from_secs(keepalive + 15), read.next()) => {
                let next = next.map_err(|_| anyhow!("Keepalive-Timeout"))?;
                let Some(msg) = next else { bail!("Verbindung wurde getrennt") };
                match msg? {
                    Message::Text(t) => {
                        let v: Value = serde_json::from_str(t.as_str())?;
                        match s(&v, "/metadata/message_type").as_str() {
                            "session_welcome" => {
                                keepalive = u(&v, "/payload/session/keepalive_timeout_seconds").max(10);
                                let session_id = s(&v, "/payload/session/id");
                                if !reconnect {
                                    let failed = subscribe_all(st, &session_id).await?;
                                    if !failed.is_empty() {
                                        connected_msg = format!("Verbunden (nicht verfügbar: {})", failed.join(", "));
                                    }
                                }
                                welcomed = true;
                                update_stream_status(st, &connected_msg).await;
                            }
                            "notification" => handle_notification(st, &v),
                            "session_reconnect" => {
                                let next = s(&v, "/payload/session/reconnect_url");
                                if !next.is_empty() { return Ok(next); }
                            }
                            "revocation" => {
                                log::warn!("Twitch EventSub revocation: {}", s(&v, "/payload/subscription/type"));
                            }
                            _ => {}
                        }
                    }
                    Message::Ping(p) => write.send(Message::Pong(p)).await?,
                    Message::Close(f) => bail!("Twitch hat die Verbindung geschlossen{}", f.map(|f| format!(": {}", f.reason)).unwrap_or_default()),
                    _ => {}
                }
            }
            _ = ticker.tick() => {
                if welcomed { update_stream_status(st, &connected_msg).await; }
            }
        }
    }
}

async fn subscribe_all(st: &Shared, session_id: &str) -> anyhow::Result<Vec<String>> {
    let uid = st.store.get().twitch.user_id.unwrap_or_default();
    let bc = json!({ "broadcaster_user_id": uid });
    let bc_user = json!({ "broadcaster_user_id": uid, "user_id": uid });
    let subs: Vec<(&str, &str, Value)> = vec![
        ("channel.chat.message", "1", bc_user.clone()),
        ("channel.chat.message_delete", "1", bc_user.clone()),
        ("channel.follow", "2", json!({ "broadcaster_user_id": uid, "moderator_user_id": uid })),
        ("channel.subscribe", "1", bc.clone()),
        ("channel.subscription.message", "1", bc.clone()),
        ("channel.subscription.gift", "1", bc.clone()),
        ("channel.cheer", "1", bc.clone()),
        ("channel.raid", "1", json!({ "to_broadcaster_user_id": uid })),
    ];
    let mut failed = Vec::new();
    for (typ, version, condition) in subs {
        let body = json!({
            "type": typ, "version": version, "condition": condition,
            "transport": { "method": "websocket", "session_id": session_id }
        });
        if let Err(e) = helix(st, Method::POST, "eventsub/subscriptions", &[], Some(body)).await {
            let msg = e.to_string();
            if msg.contains("401") || msg.contains("nicht verbunden") {
                return Err(e);
            }
            if !msg.contains("409") {
                log::warn!("EventSub {typ}: {msg}");
                failed.push(typ.trim_start_matches("channel.").to_string());
            }
        }
    }
    Ok(failed)
}

async fn update_stream_status(st: &Shared, msg: &str) {
    let cfg = st.store.get().twitch;
    let uid = cfg.user_id.clone().unwrap_or_default();
    let live = match helix(st, Method::GET, "streams", &[("user_id", uid)], None).await {
        Ok(v) => {
            let stream = &v["data"][0];
            if stream.is_object() {
                st.hub.set_viewers(Platform::Twitch, Some(u(stream, "/viewer_count")));
                true
            } else {
                st.hub.set_viewers(Platform::Twitch, None);
                false
            }
        }
        Err(_) => false,
    };
    st.hub.set_status(Platform::Twitch, "connected", live, msg, cfg.display_name);
}

fn tier_label(t: &str) -> String {
    match t {
        "1000" => "Tier 1".into(),
        "2000" => "Tier 2".into(),
        "3000" => "Tier 3".into(),
        "prime" => "Prime".into(),
        other => other.to_string(),
    }
}

fn simple_user(e: &Value, prefix: &str) -> User {
    let id = s(e, &format!("/{prefix}user_id"));
    let name = s(e, &format!("/{prefix}user_login"));
    let display = s(e, &format!("/{prefix}user_name"));
    if display.is_empty() {
        return User { id: "anonymous".into(), name: "anonymous".into(), display_name: "Anonym".into(), ..Default::default() };
    }
    User { id, name, display_name: display, ..Default::default() }
}

fn handle_notification(st: &Shared, v: &Value) {
    let typ = s(v, "/metadata/subscription_type");
    let e = &v["payload"]["event"];
    let ev = match typ.as_str() {
        "channel.chat.message" => {
            let badges: Vec<String> = e["badges"]
                .as_array()
                .map(|a| a.iter().map(|b| s(b, "/set_id")).collect())
                .unwrap_or_default();
            let user = User {
                id: s(e, "/chatter_user_id"),
                name: s(e, "/chatter_user_login"),
                display_name: s(e, "/chatter_user_name"),
                avatar: None,
                color: opt_s(e, "/color"),
                is_mod: badges.iter().any(|b| b == "moderator"),
                is_subscriber: badges.iter().any(|b| b == "subscriber" || b == "founder"),
                is_owner: badges.iter().any(|b| b == "broadcaster"),
                badges,
            };
            let parts = e["message"]["fragments"]
                .as_array()
                .map(|frags| {
                    frags
                        .iter()
                        .map(|f| {
                            if s(f, "/type") == "emote" {
                                Part {
                                    kind: "emote",
                                    text: s(f, "/text"),
                                    url: Some(format!(
                                        "https://static-cdn.jtvnw.net/emoticons/v2/{}/default/dark/2.0",
                                        s(f, "/emote/id")
                                    )),
                                }
                            } else {
                                Part { kind: "text", text: s(f, "/text"), url: None }
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut ev = Event::new(Platform::Twitch, "chat").user(user).message(s(e, "/message/text"));
            ev.parts = parts;
            ev.data = json!({ "message_id": s(e, "/message_id") });
            ev
        }
        "channel.chat.message_delete" => {
            let mut ev = Event::new(Platform::Twitch, "delete");
            ev.data = json!({ "message_id": s(e, "/message_id") });
            ev
        }
        "channel.follow" => Event::new(Platform::Twitch, "follow").user(simple_user(e, "")),
        "channel.subscribe" => {
            if b(e, "/is_gift") {
                return; // wird über channel.subscription.gift angezeigt
            }
            let mut ev = Event::new(Platform::Twitch, "subscribe").user(simple_user(e, "")).amount(1.0);
            ev.tier = Some(tier_label(&s(e, "/tier")));
            ev
        }
        "channel.subscription.message" => {
            let mut ev = Event::new(Platform::Twitch, "resub")
                .user(simple_user(e, ""))
                .message(s(e, "/message/text"))
                .amount(u(e, "/cumulative_months") as f64);
            ev.tier = Some(tier_label(&s(e, "/tier")));
            ev
        }
        "channel.subscription.gift" => {
            let mut ev = Event::new(Platform::Twitch, "gift_sub").user(simple_user(e, "")).amount(u(e, "/total") as f64);
            ev.tier = Some(tier_label(&s(e, "/tier")));
            ev
        }
        "channel.cheer" => Event::new(Platform::Twitch, "cheer")
            .user(simple_user(e, ""))
            .message(s(e, "/message"))
            .amount(u(e, "/bits") as f64),
        "channel.raid" => Event::new(Platform::Twitch, "raid")
            .user(simple_user(e, "from_broadcaster_"))
            .amount(u(e, "/viewers") as f64),
        _ => return,
    };
    st.hub.publish(ev);
}

// ---------------------------------------------------------------------------
// Kanalinfos & Chat
// ---------------------------------------------------------------------------

fn broadcaster(st: &Shared) -> anyhow::Result<String> {
    st.store.get().twitch.user_id.ok_or_else(|| anyhow!("Twitch ist nicht verbunden"))
}

pub async fn get_channel_info(st: &Shared) -> anyhow::Result<Value> {
    let id = broadcaster(st)?;
    let v = helix(st, Method::GET, "channels", &[("broadcaster_id", id)], None).await?;
    let c = &v["data"][0];
    Ok(json!({
        "title": s(c, "/title"),
        "category_id": s(c, "/game_id"),
        "category_name": s(c, "/game_name"),
        "tags": c["tags"],
    }))
}

pub async fn update_channel_info(st: &Shared, title: Option<String>, category_id: Option<String>) -> anyhow::Result<()> {
    let id = broadcaster(st)?;
    let mut body = serde_json::Map::new();
    if let Some(t) = title {
        body.insert("title".into(), t.into());
    }
    if let Some(c) = category_id {
        body.insert("game_id".into(), c.into());
    }
    if body.is_empty() {
        return Ok(());
    }
    helix(st, Method::PATCH, "channels", &[("broadcaster_id", id)], Some(Value::Object(body))).await?;
    Ok(())
}

pub async fn search_categories(st: &Shared, query: &str) -> anyhow::Result<Value> {
    let v = helix(st, Method::GET, "search/categories", &[("query", query.to_string()), ("first", "15".into())], None).await?;
    let list: Vec<Value> = v["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|c| {
            json!({
                "id": s(c, "/id"),
                "name": s(c, "/name"),
                "image": s(c, "/box_art_url").replace("{width}", "52").replace("{height}", "72"),
            })
        })
        .collect();
    Ok(Value::Array(list))
}

pub async fn send_chat(st: &Shared, text: &str) -> anyhow::Result<()> {
    let id = broadcaster(st)?;
    let v = helix(
        st,
        Method::POST,
        "chat/messages",
        &[],
        Some(json!({ "broadcaster_id": id, "sender_id": id, "message": text })),
    )
    .await?;
    if !b(&v, "/data/0/is_sent") {
        bail!("Nachricht wurde nicht gesendet: {}", s(&v, "/data/0/drop_reason/message"));
    }
    Ok(())
}
