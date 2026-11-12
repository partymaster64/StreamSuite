//! YouTube-Integration über die offizielle YouTube Data API v3.
//! - Anmeldung: OAuth 2.0 für Desktop-Apps (Loopback-Redirect auf 127.0.0.1 + PKCE)
//! - Live-Chat: liveChatMessages.list (Polling im vom Server vorgegebenen Intervall)
//! - Alerts: Super Chats/Sticker, Mitgliedschaften, neue Abonnenten (myRecentSubscribers), Likes (Differenz der Like-Zahl)
//! - Titel / Beschreibung / Kategorie: videos.update auf den aktiven oder geplanten Livestream

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Method;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::hub::{Event, Platform, User};
use crate::state::{Shared, YoutubeLive, b, now_secs, opt_s, s, u};

const API: &str = "https://www.googleapis.com/youtube/v3";
const SCOPE: &str = "https://www.googleapis.com/auth/youtube";

// ---------------------------------------------------------------------------
// HTTP / Auth
// ---------------------------------------------------------------------------

async fn ensure_token(st: &Shared, force: bool) -> anyhow::Result<String> {
    let cfg = st.store.get().youtube;
    if !force {
        if let Some(at) = &cfg.access_token {
            if cfg.expires_at - 60 > now_secs() {
                return Ok(at.clone());
            }
        }
    }
    let _guard = st.youtube_refresh.lock().await;
    let cfg = st.store.get().youtube;
    if !force {
        if let Some(at) = &cfg.access_token {
            if cfg.expires_at - 60 > now_secs() {
                return Ok(at.clone());
            }
        }
    }
    let rt = cfg.refresh_token.clone().ok_or_else(|| anyhow!("YouTube ist nicht verbunden"))?;
    let (client_id, client_secret) = st.store.get().google_client();
    let res = st
        .http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", rt.as_str()),
        ])
        .send()
        .await?;
    let v: Value = res.json().await?;
    if let Some(at) = opt_s(&v, "/access_token") {
        let exp = now_secs() + u(&v, "/expires_in") as i64;
        st.store.update(|s| {
            s.youtube.access_token = Some(at.clone());
            s.youtube.expires_at = exp;
        });
        return Ok(at);
    }
    if s(&v, "/error") == "invalid_grant" {
        st.store.update(|s| {
            s.youtube.access_token = None;
            s.youtube.refresh_token = None;
        });
        bail!("YouTube-Sitzung abgelaufen – bitte neu verbinden");
    }
    bail!("Token-Erneuerung fehlgeschlagen: {}", s(&v, "/error_description"))
}

/// Fehler der YouTube API inkl. `reason` (z. B. quotaExceeded, liveChatEnded).
#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub reason: String,
    pub message: String,
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.reason == "quotaExceeded" {
            return write!(f, "YouTube API-Kontingent für heute erschöpft");
        }
        write!(f, "YouTube API {} ({}): {}", self.status, self.reason, self.message)
    }
}
impl std::error::Error for ApiError {}

fn reason_of(e: &anyhow::Error) -> String {
    e.downcast_ref::<ApiError>().map(|a| a.reason.clone()).unwrap_or_default()
}

pub async fn api(st: &Shared, method: Method, path: &str, query: &[(&str, String)], body: Option<Value>) -> anyhow::Result<Value> {
    for attempt in 0..2 {
        let token = ensure_token(st, attempt > 0).await?;
        let mut req = st.http.request(method.clone(), format!("{API}/{path}")).query(query).bearer_auth(token);
        if let Some(b) = &body {
            req = req.json(b);
        }
        let res = req.send().await?;
        let status = res.status().as_u16();
        if status == 401 && attempt == 0 {
            continue;
        }
        let text = res.text().await.unwrap_or_default();
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if !(200..300).contains(&status) {
            return Err(ApiError {
                status,
                reason: s(&v, "/error/errors/0/reason"),
                message: opt_s(&v, "/error/message").unwrap_or(text),
            }
            .into());
        }
        return Ok(v);
    }
    bail!("YouTube-Authentifizierung fehlgeschlagen")
}

/// Startet den OAuth-Flow im Systembrowser. Gibt die Auth-URL zurück (falls der Browser nicht aufgeht).
pub async fn start_auth(st: Shared) -> anyhow::Result<String> {
    let (client_id, client_secret) = st.store.get().google_client();
    if client_id.is_empty() || client_secret.is_empty() {
        bail!("In diesem Build ist kein Google OAuth-Client hinterlegt (siehe Einstellungen → Erweitert)");
    }
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let redirect = format!("http://127.0.0.1:{port}");
    let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state_param = uuid::Uuid::new_v4().simple().to_string();
    let url = url::Url::parse_with_params(
        "https://accounts.google.com/o/oauth2/v2/auth",
        &[
            ("client_id", client_id.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state_param.as_str()),
        ],
    )?
    .to_string();

    let st2 = st.clone();
    tauri::async_runtime::spawn(async move {
        let result = async {
            let code = tokio::time::timeout(Duration::from_secs(300), wait_for_code(listener, &state_param))
                .await
                .map_err(|_| anyhow!("Zeitüberschreitung – Anmeldung nicht abgeschlossen"))??;
            let res = st2
                .http
                .post("https://oauth2.googleapis.com/token")
                .form(&[
                    ("code", code.as_str()),
                    ("client_id", client_id.as_str()),
                    ("client_secret", client_secret.as_str()),
                    ("redirect_uri", redirect.as_str()),
                    ("grant_type", "authorization_code"),
                    ("code_verifier", verifier.as_str()),
                ])
                .send()
                .await?;
            let v: Value = res.json().await?;
            let at = opt_s(&v, "/access_token").ok_or_else(|| anyhow!("{}", s(&v, "/error_description")))?;
            let rt = opt_s(&v, "/refresh_token");
            let exp = now_secs() + u(&v, "/expires_in") as i64;
            st2.store.update(|s| {
                s.youtube.access_token = Some(at);
                if rt.is_some() {
                    s.youtube.refresh_token = rt;
                }
                s.youtube.expires_at = exp;
                s.youtube.enabled = true;
            });
            let ch = api(&st2, Method::GET, "channels", &[("part", "snippet".into()), ("mine", "true".into())], None).await?;
            let item = &ch["items"][0];
            if !item.is_object() {
                bail!("Mit diesem Google-Konto ist kein YouTube-Kanal verknüpft");
            }
            let title = s(item, "/snippet/title");
            let id = s(item, "/id");
            st2.store.update(|s| {
                s.youtube.channel_id = Some(id);
                s.youtube.channel_title = Some(title.clone());
            });
            anyhow::Ok(title)
        }
        .await;
        match result {
            Ok(title) => {
                auth_result(&st2, true, &format!("Verbunden als {title}"));
                crate::connections::start(&st2, Platform::Youtube);
            }
            Err(e) => auth_result(&st2, false, &format!("{e:#}")),
        }
    });

    let _ = st.app.opener().open_url(url.clone(), None::<&str>);
    Ok(url)
}

fn auth_result(st: &Shared, ok: bool, msg: &str) {
    st.hub.emit_raw("ss://auth", json!({ "platform": "youtube", "ok": ok, "message": msg }));
}

async fn wait_for_code(listener: TcpListener, expected_state: &str) -> anyhow::Result<String> {
    loop {
        let (mut sock, _) = listener.accept().await?;
        let mut buf = vec![0u8; 8192];
        let mut len = 0;
        while len < buf.len() {
            let n = sock.read(&mut buf[len..]).await?;
            if n == 0 {
                break;
            }
            len += n;
            if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let req = String::from_utf8_lossy(&buf[..len]);
        let path = req.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("/").to_string();
        let parsed = url::Url::parse(&format!("http://localhost{path}"))?;
        let get = |k: &str| parsed.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.to_string());

        let (ok, body, outcome) = if let Some(err) = get("error") {
            (false, format!("Anmeldung abgebrochen: {err}"), Some(Err(anyhow!("Anmeldung abgebrochen: {err}"))))
        } else if let Some(code) = get("code") {
            if get("state").as_deref() != Some(expected_state) {
                (false, "Ungültiger State-Parameter.".to_string(), None)
            } else {
                (true, "YouTube ist jetzt mit StreamSuite verbunden. Du kannst dieses Fenster schließen.".to_string(), Some(Ok(code)))
            }
        } else {
            let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            continue;
        };
        let html = format!(
            "<!doctype html><html lang=de><meta charset=utf-8><title>StreamSuite</title>\
             <body style=\"font-family:system-ui;display:grid;place-items:center;height:100vh;margin:0;background:#0a0a0a;color:#fafafa\">\
             <div style=\"text-align:center\"><h2>{}</h2><p>{}</p></div></body></html>",
            if ok { "Erfolgreich verbunden" } else { "Fehler" },
            body
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.shutdown().await;
        if let Some(o) = outcome {
            return o;
        }
    }
}

// ---------------------------------------------------------------------------
// Live-Verbindung
// ---------------------------------------------------------------------------

struct SubWatcher {
    known: Option<HashSet<String>>,
    next: Instant,
}

impl SubWatcher {
    async fn tick(&mut self, st: &Shared) {
        if Instant::now() < self.next {
            return;
        }
        let interval = st.store.get().youtube.subscriber_poll_secs.max(30);
        self.next = Instant::now() + Duration::from_secs(interval);
        let res = api(
            st,
            Method::GET,
            "subscriptions",
            &[
                ("part", "subscriberSnippet".into()),
                ("myRecentSubscribers", "true".into()),
                ("maxResults", "50".into()),
            ],
            None,
        )
        .await;
        let Ok(v) = res else { return };
        let items = v["items"].as_array().cloned().unwrap_or_default();
        let first_run = self.known.is_none();
        let known = self.known.get_or_insert_with(HashSet::new);
        // Älteste zuerst ausgeben
        for item in items.iter().rev() {
            let sn = &item["subscriberSnippet"];
            let id = s(sn, "/channelId");
            if id.is_empty() || !known.insert(id.clone()) || first_run {
                continue;
            }
            let title = s(sn, "/title");
            let user = User {
                id,
                name: title.clone(),
                display_name: title,
                avatar: opt_s(sn, "/thumbnails/default/url"),
                ..Default::default()
            };
            st.hub.publish(Event::new(Platform::Youtube, "subscribe").user(user));
        }
    }
}

pub async fn run(st: Shared) {
    let mut subs = SubWatcher { known: None, next: Instant::now() };
    let mut backoff = 5u64;
    loop {
        let cfg = st.store.get().youtube;
        let account = cfg.channel_title.clone();
        if cfg.refresh_token.is_none() {
            st.hub.set_status(Platform::Youtube, "unconfigured", false, "Nicht angemeldet", None);
            return;
        }
        match find_broadcast(&st, "active").await {
            Ok(Some(bc)) => {
                backoff = 5;
                if let Err(e) = live_session(&st, &bc, &mut subs).await {
                    st.hub.set_status(Platform::Youtube, "error", false, format!("{e:#}"), account);
                    tokio::time::sleep(Duration::from_secs(15)).await;
                }
            }
            Ok(None) => {
                backoff = 5;
                st.hub.set_viewers(Platform::Youtube, None);
                st.hub.set_status(
                    Platform::Youtube,
                    "connected",
                    false,
                    "Verbunden – kein aktiver Livestream (prüfe alle 30 s)",
                    account,
                );
                subs.tick(&st).await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
            Err(e) => {
                if st.store.get().youtube.refresh_token.is_none() {
                    st.hub.set_status(Platform::Youtube, "error", false, format!("{e:#}"), account);
                    return;
                }
                let wait = if reason_of(&e) == "quotaExceeded" { 900 } else { backoff };
                st.hub
                    .set_status(Platform::Youtube, "error", false, format!("{e:#} – neuer Versuch in {wait}s"), account);
                tokio::time::sleep(Duration::from_secs(wait)).await;
                backoff = (backoff * 2).min(120);
            }
        }
    }
}

async fn find_broadcast(st: &Shared, status: &str) -> anyhow::Result<Option<Value>> {
    let v = api(
        st,
        Method::GET,
        "liveBroadcasts",
        &[
            ("part", "id,snippet,status".into()),
            ("broadcastStatus", status.into()),
            ("broadcastType", "all".into()),
            ("maxResults", "5".into()),
        ],
        None,
    )
    .await?;
    Ok(v["items"].as_array().and_then(|a| a.first().cloned()))
}

async fn live_session(st: &Shared, bc: &Value, subs: &mut SubWatcher) -> anyhow::Result<()> {
    let chat_id = s(bc, "/snippet/liveChatId");
    let video_id = s(bc, "/id");
    let account = st.store.get().youtube.channel_title;
    *st.youtube_live.lock() = Some(YoutubeLive { chat_id: chat_id.clone(), video_id: video_id.clone() });
    st.hub.set_status(
        Platform::Youtube,
        "connected",
        true,
        format!("Live: {}", s(bc, "/snippet/title")),
        account.clone(),
    );

    let mut page_token: Option<String> = None;
    let mut first_page = true;
    let mut next_chat = Instant::now();
    let mut next_stats = Instant::now();
    let mut last_likes: Option<u64> = None;

    let result: anyhow::Result<()> = loop {
        let now = Instant::now();

        if now >= next_stats {
            next_stats = now + Duration::from_secs(30);
            match api(
                st,
                Method::GET,
                "videos",
                &[("part", "statistics,liveStreamingDetails".into()), ("id", video_id.clone())],
                None,
            )
            .await
            {
                Ok(v) => {
                    let item = &v["items"][0];
                    if !s(item, "/liveStreamingDetails/actualEndTime").is_empty() {
                        break Ok(());
                    }
                    let likes = u(item, "/statistics/likeCount");
                    if let Some(prev) = last_likes {
                        if likes > prev {
                            st.hub.add_likes(Platform::Youtube, None, likes - prev);
                        }
                    }
                    last_likes = Some(likes);
                    st.hub.set_total_likes(Platform::Youtube, likes);
                    let viewers = opt_s(item, "/liveStreamingDetails/concurrentViewers").and_then(|x| x.parse().ok());
                    st.hub.set_viewers(Platform::Youtube, viewers);
                }
                Err(e) if reason_of(&e) == "quotaExceeded" => break Err(e),
                Err(e) => log::warn!("YouTube-Statistik: {e}"),
            }
        }

        subs.tick(st).await;

        if now >= next_chat {
            let mut q = vec![
                ("liveChatId", chat_id.clone()),
                ("part", "snippet,authorDetails".into()),
                ("maxResults", "2000".into()),
                ("profileImageSize", "64".into()),
            ];
            if let Some(t) = &page_token {
                q.push(("pageToken", t.clone()));
            }
            match api(st, Method::GET, "liveChat/messages", &q, None).await {
                Ok(v) => {
                    for item in v["items"].as_array().cloned().unwrap_or_default() {
                        if let Some(ev) = map_chat_item(&item, first_page) {
                            st.hub.publish(ev);
                        }
                    }
                    first_page = false;
                    page_token = opt_s(&v, "/nextPageToken");
                    if !s(&v, "/offlineAt").is_empty() {
                        break Ok(());
                    }
                    let min = st.store.get().youtube.min_chat_poll_ms.max(2000);
                    let wait = u(&v, "/pollingIntervalMillis").max(min);
                    next_chat = Instant::now() + Duration::from_millis(wait);
                }
                Err(e) => {
                    let r = reason_of(&e);
                    if matches!(r.as_str(), "liveChatEnded" | "liveChatNotFound" | "liveChatDisabled" | "forbidden") {
                        break Ok(());
                    }
                    break Err(e);
                }
            }
        }

        let wake = next_chat.min(next_stats);
        let sleep = wake.saturating_duration_since(Instant::now()).clamp(Duration::from_millis(250), Duration::from_secs(5));
        tokio::time::sleep(sleep).await;
    };

    *st.youtube_live.lock() = None;
    st.hub.set_viewers(Platform::Youtube, None);
    result
}

fn map_chat_item(item: &Value, first_page: bool) -> Option<Event> {
    let sn = &item["snippet"];
    let a = &item["authorDetails"];
    let mut badges = Vec::new();
    if b(a, "/isChatOwner") {
        badges.push("owner".to_string());
    }
    if b(a, "/isChatModerator") {
        badges.push("moderator".to_string());
    }
    if b(a, "/isChatSponsor") {
        badges.push("member".to_string());
    }
    if b(a, "/isVerified") {
        badges.push("verified".to_string());
    }
    let user = User {
        id: s(a, "/channelId"),
        name: s(a, "/displayName"),
        display_name: s(a, "/displayName"),
        avatar: opt_s(a, "/profileImageUrl"),
        color: None,
        badges,
        is_mod: b(a, "/isChatModerator"),
        is_subscriber: b(a, "/isChatSponsor"),
        is_owner: b(a, "/isChatOwner"),
    };
    let ts = chrono::DateTime::parse_from_rfc3339(&s(sn, "/publishedAt"))
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis());

    let mut ev = match s(sn, "/type").as_str() {
        "textMessageEvent" => {
            let mut ev = Event::new(Platform::Youtube, "chat").user(user).message(s(sn, "/displayMessage"));
            ev.data = json!({ "message_id": s(item, "/id") });
            ev
        }
        "messageDeletedEvent" => {
            let mut ev = Event::new(Platform::Youtube, "delete");
            ev.data = json!({ "message_id": s(sn, "/messageDeletedDetails/deletedMessageId") });
            ev
        }
        // Alerts aus der ersten Seite sind historisch -> nicht erneut auslösen
        _ if first_page => return None,
        "superChatEvent" => {
            let d = &sn["superChatDetails"];
            let mut ev = Event::new(Platform::Youtube, "superchat")
                .user(user)
                .message(s(d, "/userComment"))
                .amount(u(d, "/amountMicros") as f64 / 1_000_000.0);
            ev.currency = opt_s(d, "/currency");
            ev.amount_display = opt_s(d, "/amountDisplayString");
            ev.tier = opt_s(d, "/tier");
            ev
        }
        "superStickerEvent" => {
            let d = &sn["superStickerDetails"];
            let mut ev = Event::new(Platform::Youtube, "supersticker")
                .user(user)
                .message(s(d, "/superStickerMetadata/altText"))
                .amount(u(d, "/amountMicros") as f64 / 1_000_000.0);
            ev.currency = opt_s(d, "/currency");
            ev.amount_display = opt_s(d, "/amountDisplayString");
            ev
        }
        "newSponsorEvent" => {
            let mut ev = Event::new(Platform::Youtube, "membership").user(user);
            ev.tier = opt_s(sn, "/newSponsorDetails/memberLevelName");
            ev
        }
        "memberMilestoneChatEvent" => {
            let d = &sn["memberMilestoneChatDetails"];
            let mut ev = Event::new(Platform::Youtube, "resub")
                .user(user)
                .message(s(d, "/userComment"))
                .amount(u(d, "/memberMonth") as f64);
            ev.tier = opt_s(d, "/memberLevelName");
            ev
        }
        "membershipGiftingEvent" => {
            let d = &sn["membershipGiftingDetails"];
            let mut ev = Event::new(Platform::Youtube, "gift_sub").user(user).amount(u(d, "/giftMembershipsCount") as f64);
            ev.tier = opt_s(d, "/giftMembershipsLevelName");
            ev
        }
        _ => return None,
    };
    ev.timestamp = ts;
    Some(ev)
}

// ---------------------------------------------------------------------------
// Titel / Beschreibung / Kategorie & Chat senden
// ---------------------------------------------------------------------------

async fn target_broadcast(st: &Shared) -> anyhow::Result<Option<(Value, &'static str)>> {
    if let Some(bc) = find_broadcast(st, "active").await? {
        return Ok(Some((bc, "live")));
    }
    Ok(find_broadcast(st, "upcoming").await?.map(|bc| (bc, "upcoming")))
}

async fn video_snippet(st: &Shared, id: &str) -> anyhow::Result<Value> {
    let v = api(st, Method::GET, "videos", &[("part", "snippet".into()), ("id", id.into())], None).await?;
    Ok(v["items"][0]["snippet"].clone())
}

pub async fn get_info(st: &Shared) -> anyhow::Result<Value> {
    let Some((bc, status)) = target_broadcast(st).await? else {
        return Ok(Value::Null);
    };
    let id = s(&bc, "/id");
    let sn = video_snippet(st, &id).await?;
    Ok(json!({
        "video_id": id,
        "status": status,
        "title": s(&sn, "/title"),
        "description": s(&sn, "/description"),
        "category_id": s(&sn, "/categoryId"),
    }))
}

pub async fn update_info(
    st: &Shared,
    title: Option<String>,
    description: Option<String>,
    category_id: Option<String>,
) -> anyhow::Result<()> {
    let Some((bc, _)) = target_broadcast(st).await? else {
        bail!("Kein aktiver oder geplanter YouTube-Livestream gefunden");
    };
    let id = s(&bc, "/id");
    let cur = video_snippet(st, &id).await?;
    let mut snippet = json!({
        "title": title.unwrap_or_else(|| s(&cur, "/title")),
        "description": description.unwrap_or_else(|| s(&cur, "/description")),
        "categoryId": category_id.filter(|c| !c.is_empty()).unwrap_or_else(|| s(&cur, "/categoryId")),
    });
    for key in ["tags", "defaultLanguage", "defaultAudioLanguage"] {
        if !cur[key].is_null() {
            snippet[key] = cur[key].clone();
        }
    }
    api(st, Method::PUT, "videos", &[("part", "snippet".into())], Some(json!({ "id": id, "snippet": snippet }))).await?;
    Ok(())
}

pub async fn categories(st: &Shared) -> anyhow::Result<Value> {
    let v = api(
        st,
        Method::GET,
        "videoCategories",
        &[("part", "snippet".into()), ("regionCode", "DE".into()), ("hl", "de".into())],
        None,
    )
    .await?;
    let list: Vec<Value> = v["items"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|c| b(c, "/snippet/assignable"))
        .map(|c| json!({ "id": s(c, "/id"), "name": s(c, "/snippet/title") }))
        .collect();
    Ok(Value::Array(list))
}

pub async fn send_chat(st: &Shared, text: &str) -> anyhow::Result<()> {
    let live = st.youtube_live.lock().clone().ok_or_else(|| anyhow!("Kein aktiver YouTube-Livestream"))?;
    api(
        st,
        Method::POST,
        "liveChat/messages",
        &[("part", "snippet".into())],
        Some(json!({ "snippet": {
            "liveChatId": live.chat_id,
            "type": "textMessageEvent",
            "textMessageDetails": { "messageText": text }
        }})),
    )
    .await?;
    Ok(())
}
