//! TikTok-LIVE-Events über eine direkte Webcast-WebSocket-Verbindung.
//!
//! Protokoll-Implementierung: [PirateTok/live-rs](https://github.com/PirateTok/live-rs) (0BSD).
//! Keine Signier-Server, kein API-Key, kein Drittanbieter: Die App spricht direkt mit TikTok
//! (anonymer `ttwid`-Cookie, Protobuf-Frames, Heartbeats). Schreibende Aktionen (Chat, Moderation,
//! Einstellungen) laufen getrennt davon über die eingeloggte Sitzung, siehe `tiktok_control.rs`.

use std::time::Duration;

use piratetok_live_rs::TikTokLive;
use piratetok_live_rs::errors::TikTokLiveError;
use piratetok_live_rs::http::api::{FetchParams, fetch_room_id};
use piratetok_live_rs::structs::TikTokLiveEvent;
use piratetok_live_rs::structs::proto::{EmoteData, Image, UserIdentity, UserIdentityContext, WebcastGiftMessage};
use serde_json::json;

use crate::hub::{Event, Part, Platform, User};
use crate::state::Shared;

/// Kontext des laufenden eigenen LIVE-Raums – Grundlage für die Steuerfunktionen.
#[derive(Debug, Clone)]
pub struct TiktokRoom {
    pub room_id: String,
    pub anchor_id: String,
    pub username: String,
}

/// Wie oft geprüft wird, ob der eigene Kanal live gegangen ist.
const OFFLINE_POLL: Duration = Duration::from_secs(60);

pub async fn run(st: Shared) {
    let mut backoff = 5u64;
    loop {
        let user = st.store.get().tiktok.username.trim().trim_start_matches('@').to_string();
        if user.is_empty() {
            st.hub.set_status(Platform::Tiktok, "unconfigured", false, "Bitte bei TikTok anmelden", None);
            return;
        }
        let account = Some(format!("@{user}"));
        st.hub.set_status(Platform::Tiktok, "connecting", false, format!("Prüfe, ob @{user} live ist …"), account.clone());

        let wait = match session(&st, &user, &account).await {
            Ok(Ended::Offline) => {
                backoff = 5;
                st.hub.set_status(
                    Platform::Tiktok,
                    "connected",
                    false,
                    format!("@{user} ist nicht live – prüfe jede Minute erneut"),
                    account,
                );
                OFFLINE_POLL.as_secs()
            }
            Ok(Ended::Finished) => {
                backoff = 5;
                st.hub.set_status(Platform::Tiktok, "connected", false, "Livestream beendet", account);
                OFFLINE_POLL.as_secs()
            }
            Ok(Ended::Lost) => {
                backoff = 5;
                st.hub.set_status(Platform::Tiktok, "connecting", false, "Verbindung verloren – verbinde neu …", account);
                5
            }
            Err(TikTokLiveError::UserNotFound(_)) => {
                st.hub.set_status(Platform::Tiktok, "error", false, format!("TikTok-Konto @{user} nicht gefunden"), account);
                return;
            }
            Err(e) => {
                let w = backoff;
                backoff = (backoff * 2).min(300);
                let reason = match &e {
                    TikTokLiveError::TikTokBlocked(_) => "TikTok blockiert die Anfrage vorübergehend".to_string(),
                    other => other.to_string(),
                };
                st.hub.set_status(Platform::Tiktok, "error", false, format!("{reason} – neuer Versuch in {w}s"), account);
                w
            }
        };
        clear_room(&st);
        st.hub.reset_platform_stats(Platform::Tiktok);
        tokio::time::sleep(Duration::from_secs(wait)).await;
    }
}

enum Ended {
    /// Kanal ist (noch) nicht live.
    Offline,
    /// TikTok hat das Ende des LIVE gemeldet.
    Finished,
    /// Verbindung nach mehreren Wiederholungen abgebrochen.
    Lost,
}

async fn session(st: &Shared, user: &str, account: &Option<String>) -> Result<Ended, TikTokLiveError> {
    // Raum-ID + Anchor-ID ermitteln (wirft HostNotOnline, wenn nicht live)
    let room = match fetch_room_id(user, FetchParams::default()).await {
        Ok(r) => r,
        Err(TikTokLiveError::HostNotOnline(_)) => return Ok(Ended::Offline),
        Err(e) => return Err(e),
    };

    let mut stream = match TikTokLive::builder(user).max_retries(5).connect().await {
        Ok(s) => s,
        Err(TikTokLiveError::HostNotOnline(_)) => return Ok(Ended::Offline),
        Err(e) => return Err(e),
    };

    *st.tiktok_room.lock() = Some(TiktokRoom {
        room_id: room.room_id.clone(),
        anchor_id: room.anchor_id.clone(),
        username: user.to_string(),
    });

    loop {
        let event = match stream.next_event().await {
            Ok(ev) => ev,
            Err(_) => return Ok(Ended::Lost),
        };
        match event {
            TikTokLiveEvent::Connected { .. } => {
                st.hub.set_status(Platform::Tiktok, "connected", true, "Live – empfange Events", account.clone());
                // LIVE-Seite im Hintergrund vorladen, damit Chat-Senden & Steuerung sofort funktionieren
                if st.store.get().tiktok.session_id.is_some() {
                    match crate::tiktok_web::ensure_chat_window(st) {
                        Ok(_) => crate::tiktok_web::reload_chat_window(st),
                        Err(e) => log::warn!("TikTok-Chatfenster: {e}"),
                    }
                }
            }
            TikTokLiveEvent::Reconnecting {
                attempt,
                max_retries,
                delay_secs,
            } => {
                st.hub.set_status(
                    Platform::Tiktok,
                    "connecting",
                    true,
                    format!("Verbindung unterbrochen – Versuch {attempt}/{max_retries} in {delay_secs}s"),
                    account.clone(),
                );
            }
            TikTokLiveEvent::Disconnected => return Ok(Ended::Lost),
            TikTokLiveEvent::LiveEnded(_) => {
                st.hub.set_viewers(Platform::Tiktok, None);
                return Ok(Ended::Finished);
            }
            other => {
                // Nach einer Wiederholung kommt kein neues `Connected` – erstes Event setzt den Status zurück
                if !is_live_status(st) {
                    st.hub.set_status(Platform::Tiktok, "connected", true, "Live – empfange Events", account.clone());
                }
                handle(st, other);
            }
        }
    }
}

fn is_live_status(st: &Shared) -> bool {
    st.hub
        .statuses()
        .iter()
        .any(|s| s.platform == Platform::Tiktok && s.live && s.state == "connected")
}

pub fn clear_room(st: &Shared) {
    *st.tiktok_room.lock() = None;
}

fn image_url(img: &Option<Image>) -> Option<String> {
    img.as_ref().and_then(|i| i.url_list.first().cloned()).filter(|u| !u.is_empty())
}

fn tt_user(u: Option<&UserIdentity>) -> User {
    let Some(u) = u else {
        return User {
            display_name: "Unbekannt".into(),
            ..Default::default()
        };
    };
    let unique = u.unique_id.clone();
    let id = if u.user_id != 0 {
        u.user_id.to_string()
    } else if !u.id_str.is_empty() {
        u.id_str.clone()
    } else {
        unique.clone()
    };
    User {
        id,
        display_name: if u.nickname.is_empty() { unique.clone() } else { u.nickname.clone() },
        name: unique,
        avatar: image_url(&u.avatar_thumb),
        ..Default::default()
    }
}

/// Rollen aus dem Identitätskontext einer Chatnachricht übernehmen.
fn apply_identity(user: &mut User, ctx: Option<&UserIdentityContext>) {
    let Some(ctx) = ctx else { return };
    user.is_owner = ctx.is_anchor;
    user.is_mod = ctx.is_moderator_of_anchor;
    user.is_subscriber = ctx.is_subscriber_of_anchor;
    if user.is_owner {
        user.badges.push("owner".into());
    }
    if user.is_mod {
        user.badges.push("moderator".into());
    }
    if user.is_subscriber {
        user.badges.push("subscriber".into());
    }
}

/// Kommentar mit eingebetteten Sticker-Emotes in Text-/Emote-Teile zerlegen.
fn chat_parts(comment: &str, emotes: &[EmoteData]) -> Vec<Part> {
    if emotes.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = comment.chars().collect();
    let mut sorted: Vec<&EmoteData> = emotes.iter().collect();
    sorted.sort_by_key(|e| e.place_in_comment);
    let mut parts = Vec::new();
    let mut pos = 0usize;
    for e in sorted {
        let at = (e.place_in_comment.max(0) as usize).clamp(pos, chars.len());
        if at > pos {
            parts.push(Part {
                kind: "text",
                text: chars[pos..at].iter().collect(),
                url: None,
            });
        }
        pos = at;
        let details = e.emote.as_ref();
        parts.push(Part {
            kind: "emote",
            text: details.map(|d| d.emote_id.clone()).unwrap_or_default(),
            url: details.and_then(|d| image_url(&d.image)),
        });
    }
    if pos < chars.len() {
        parts.push(Part {
            kind: "text",
            text: chars[pos..].iter().collect(),
            url: None,
        });
    }
    parts
}

fn gift_event(st: &Shared, g: &WebcastGiftMessage) -> Option<Event> {
    let details = g.gift_details.as_ref();
    let streakable = details.map(|d| d.combo || d.gift_type == 1).unwrap_or(false);
    if streakable && g.repeat_end != 1 {
        return None; // Streak läuft noch – erst am Ende ein Alert
    }
    let repeat = (g.repeat_count.max(1)) as u64;
    let diamonds = details.map(|d| d.diamond_count.max(0) as u64).unwrap_or(0);
    let coins = diamonds * repeat;
    if coins < st.store.get().alerts.tiktok_min_coins {
        return None;
    }
    let mut ev = Event::new(Platform::Tiktok, "gift").user(tt_user(g.user.as_ref())).amount(repeat as f64);
    ev.coins = Some(coins);
    ev.gift_name = details.map(|d| d.gift_name.clone()).filter(|n| !n.is_empty());
    ev.gift_image = details.and_then(|d| image_url(&d.icon).or_else(|| image_url(&d.gift_image)));
    ev.data = json!({ "gift_id": g.gift_id, "diamonds_per_gift": diamonds });
    Some(ev)
}

fn handle(st: &Shared, event: TikTokLiveEvent) {
    let ev = match event {
        TikTokLiveEvent::Chat(m) => {
            let mut user = tt_user(m.user.as_ref());
            apply_identity(&mut user, m.user_identity.as_ref());
            let mut ev = Event::new(Platform::Tiktok, "chat").user(user).message(m.comment.clone());
            ev.parts = chat_parts(&m.comment, &m.emotes);
            ev.data = json!({ "message_id": m.common.as_ref().map(|c| c.msg_id.to_string()).unwrap_or_default() });
            ev
        }
        TikTokLiveEvent::EmoteChat(m) => {
            let mut user = tt_user(m.user.as_ref());
            apply_identity(&mut user, m.user_identity.as_ref());
            let mut ev = Event::new(Platform::Tiktok, "chat").user(user);
            ev.parts = chat_parts("", &m.emote_list);
            if ev.parts.is_empty() {
                return;
            }
            ev.data = json!({ "message_id": m.common.as_ref().map(|c| c.msg_id.to_string()).unwrap_or_default() });
            ev
        }
        TikTokLiveEvent::ImDelete(m) => {
            for id in m.delete_msg_ids_list {
                let mut ev = Event::new(Platform::Tiktok, "delete");
                ev.data = json!({ "message_id": id.to_string() });
                st.hub.publish(ev);
            }
            return;
        }
        TikTokLiveEvent::Gift(g) => match gift_event(st, &g) {
            Some(ev) => ev,
            None => return,
        },
        TikTokLiveEvent::Like(m) => {
            st.hub.add_likes(Platform::Tiktok, Some(tt_user(m.user.as_ref())), m.like_count.max(0) as u64);
            if m.total_like_count > 0 {
                st.hub.set_total_likes(Platform::Tiktok, m.total_like_count as u64);
            }
            return;
        }
        TikTokLiveEvent::Follow(m) => Event::new(Platform::Tiktok, "follow").user(tt_user(m.user.as_ref())),
        TikTokLiveEvent::Share(m) => Event::new(Platform::Tiktok, "share").user(tt_user(m.user.as_ref())),
        TikTokLiveEvent::Join(m) => Event::new(Platform::Tiktok, "join").user(tt_user(m.user.as_ref())),
        TikTokLiveEvent::SubNotify(m) => {
            let user = tt_user(m.user.as_ref().or(m.sender.as_ref()));
            let ev = Event::new(Platform::Tiktok, "subscribe").user(user);
            if m.sub_month > 1 { ev.amount(m.sub_month as f64) } else { ev }
        }
        TikTokLiveEvent::RoomUserSeq(m) => {
            st.hub.set_viewers(Platform::Tiktok, Some(m.viewer_count.max(0) as u64));
            return;
        }
        _ => return,
    };
    st.hub.publish(ev);
}
