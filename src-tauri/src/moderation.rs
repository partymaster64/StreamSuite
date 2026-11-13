//! Plattformübergreifende Chat-Moderation (Multichat in der App und OBS-Dock).
//!
//! Aktionen: `delete` (Nachricht löschen), `timeout` (mit `duration` in Sekunden), `ban`, `unban`,
//! `mod` / `unmod`, `vip` / `unvip` (nur Twitch). TikTok nutzt die vorhandene Steuerung in
//! `tiktok_control` (`timeout` = stummschalten, `ban` = aus dem LIVE entfernen).

use anyhow::{anyhow, bail};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::hub::Platform;
use crate::state::{Shared, s};
use crate::{tiktok_control, twitch, youtube};

#[derive(Debug, Clone, Deserialize)]
pub struct ModRequest {
    pub platform: String,
    pub action: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub message_id: Option<String>,
    /// Sekunden (Timeout). TikTok: 0 = für den ganzen LIVE.
    #[serde(default)]
    pub duration: Option<u64>,
}

/// Auf welchen Plattformen gerade moderiert werden kann (angemeldet + Chat aktiv).
pub fn capabilities(st: &Shared) -> Value {
    let cfg = st.store.get();
    let connected = |p: Platform| st.hub.statuses().iter().any(|s| s.platform == p && s.state == "connected");
    json!({
        "twitch": cfg.twitch.access_token.is_some() && connected(Platform::Twitch),
        "youtube": st.youtube_live.lock().is_some(),
        "tiktok": cfg.tiktok.session_id.is_some() && st.tiktok_room.lock().is_some(),
    })
}

pub async fn moderate(st: &Shared, req: &ModRequest) -> anyhow::Result<()> {
    let platform = Platform::parse(&req.platform).ok_or_else(|| anyhow!("Unbekannte Plattform: {}", req.platform))?;
    if req.action != "delete" && req.user_id.trim().is_empty() {
        bail!("Kein Nutzer angegeben");
    }
    match platform {
        Platform::Twitch => twitch_action(st, req).await.map_err(twitch_hint),
        Platform::Youtube => youtube_action(st, req).await,
        Platform::Tiktok => tiktok_action(st, req).await,
        Platform::System => bail!("System-Nachrichten lassen sich nicht moderieren"),
    }
}

fn message_id(req: &ModRequest) -> anyhow::Result<String> {
    req.message_id.clone().filter(|m| !m.is_empty()).ok_or_else(|| anyhow!("Keine Nachrichten-ID"))
}

// ---------------------------------------------------------------------------
// Twitch (Helix, eigener Kanal: broadcaster_id = moderator_id)
// ---------------------------------------------------------------------------

/// Ältere Logins haben die Moderations-Berechtigungen noch nicht.
fn twitch_hint(e: anyhow::Error) -> anyhow::Error {
    let msg = e.to_string();
    if msg.contains("scope") || msg.contains("401") {
        anyhow!("Für die Moderation fehlen Twitch-Berechtigungen – bitte unter Verbindungen bei Twitch ab- und wieder anmelden")
    } else {
        e
    }
}

async fn twitch_action(st: &Shared, req: &ModRequest) -> anyhow::Result<()> {
    let me = st.store.get().twitch.user_id.ok_or_else(|| anyhow!("Twitch ist nicht verbunden"))?;
    let user = req.user_id.clone();
    let base = |extra: &[(&'static str, String)]| {
        let mut q = vec![("broadcaster_id", me.clone()), ("moderator_id", me.clone())];
        q.extend_from_slice(extra);
        q
    };
    match req.action.as_str() {
        "delete" => {
            twitch::helix(st, Method::DELETE, "moderation/chat", &base(&[("message_id", message_id(req)?)]), None).await?;
        }
        "timeout" | "ban" => {
            let mut data = json!({ "user_id": user });
            if req.action == "timeout" {
                // Twitch: 1 s bis 14 Tage
                data["duration"] = req.duration.unwrap_or(600).clamp(1, 1_209_600).into();
            }
            twitch::helix(st, Method::POST, "moderation/bans", &base(&[]), Some(json!({ "data": data }))).await?;
        }
        "unban" => {
            twitch::helix(st, Method::DELETE, "moderation/bans", &base(&[("user_id", user)]), None).await?;
        }
        "mod" | "unmod" => {
            let m = if req.action == "mod" { Method::POST } else { Method::DELETE };
            twitch::helix(st, m, "moderation/moderators", &[("broadcaster_id", me), ("user_id", user)], None).await?;
        }
        "vip" | "unvip" => {
            let m = if req.action == "vip" { Method::POST } else { Method::DELETE };
            twitch::helix(st, m, "channels/vips", &[("broadcaster_id", me), ("user_id", user)], None).await?;
        }
        other => bail!("Aktion „{other}“ gibt es bei Twitch nicht"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// YouTube (Data API v3, aktiver Live-Chat)
// ---------------------------------------------------------------------------

async fn youtube_action(st: &Shared, req: &ModRequest) -> anyhow::Result<()> {
    let live = st.youtube_live.lock().clone().ok_or_else(|| anyhow!("Kein aktiver YouTube-Livestream"))?;
    let user = req.user_id.clone();
    match req.action.as_str() {
        "delete" => {
            youtube::api(st, Method::DELETE, "liveChat/messages", &[("id", message_id(req)?)], None).await?;
        }
        "timeout" | "ban" => {
            let mut snippet = json!({
                "liveChatId": live.chat_id,
                "type": if req.action == "timeout" { "temporary" } else { "permanent" },
                "bannedUserDetails": { "channelId": user },
            });
            if req.action == "timeout" {
                snippet["banDurationSeconds"] = req.duration.unwrap_or(300).max(1).into();
            }
            let v = youtube::api(st, Method::POST, "liveChat/bans", &[("part", "snippet".into())], Some(json!({ "snippet": snippet })))
                .await?;
            let ban_id = s(&v, "/id");
            if !ban_id.is_empty() {
                st.youtube_bans.lock().insert(user, ban_id);
            }
        }
        "unban" => {
            // Die API hebt Banns nur über ihre ID auf – die kennen wir nur für selbst gesetzte Banns
            let ban_id = st.youtube_bans.lock().get(&user).cloned().ok_or_else(|| {
                anyhow!("Diesen Bann kann StreamSuite nicht aufheben (nicht in dieser Sitzung gesetzt) – bitte in YouTube Studio entsperren")
            })?;
            youtube::api(st, Method::DELETE, "liveChat/bans", &[("id", ban_id)], None).await?;
            st.youtube_bans.lock().remove(&user);
        }
        "mod" => {
            youtube::api(
                st,
                Method::POST,
                "liveChat/moderators",
                &[("part", "snippet".into())],
                Some(json!({ "snippet": { "liveChatId": live.chat_id, "moderatorDetails": { "channelId": user } } })),
            )
            .await?;
        }
        "unmod" => {
            let id = youtube_moderator_id(st, &live.chat_id, &user).await?;
            youtube::api(st, Method::DELETE, "liveChat/moderators", &[("id", id)], None).await?;
        }
        other => bail!("Aktion „{other}“ gibt es bei YouTube nicht"),
    }
    Ok(())
}

/// Die Moderator-Eintrags-ID zu einem Kanal suchen (für das Entfernen nötig).
async fn youtube_moderator_id(st: &Shared, chat_id: &str, channel_id: &str) -> anyhow::Result<String> {
    let mut page = String::new();
    for _ in 0..20 {
        let mut q = vec![("liveChatId", chat_id.to_string()), ("part", "snippet".into()), ("maxResults", "50".into())];
        if !page.is_empty() {
            q.push(("pageToken", page.clone()));
        }
        let v = youtube::api(st, Method::GET, "liveChat/moderators", &q, None).await?;
        let found = v["items"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|m| s(m, "/snippet/moderatorDetails/channelId") == channel_id)
            .map(|m| s(m, "/id"));
        if let Some(id) = found {
            return Ok(id);
        }
        page = s(&v, "/nextPageToken");
        if page.is_empty() {
            break;
        }
    }
    bail!("Dieser Nutzer ist kein Moderator in diesem Livestream")
}

// ---------------------------------------------------------------------------
// TikTok (vorhandene LIVE-Steuerung)
// ---------------------------------------------------------------------------

async fn tiktok_action(st: &Shared, req: &ModRequest) -> anyhow::Result<()> {
    let user = req.user_id.as_str();
    match req.action.as_str() {
        "timeout" => tiktok_control::moderate(st, user, "mute", req.duration).await,
        "untimeout" => tiktok_control::moderate(st, user, "unmute", None).await,
        "ban" => tiktok_control::moderate(st, user, "block", None).await,
        "unban" => tiktok_control::moderate(st, user, "unblock", None).await,
        "mod" => tiktok_control::set_moderator(st, user, true).await,
        "unmod" => tiktok_control::set_moderator(st, user, false).await,
        "delete" => bail!("TikTok erlaubt das Löschen einzelner Nachrichten über StreamSuite nicht"),
        other => bail!("Aktion „{other}“ gibt es bei TikTok nicht"),
    }
}
