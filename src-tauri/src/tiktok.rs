//! TikTok-LIVE-Integration über den EulerStream WebSocket
//! (https://github.com/EulerStream/TikTok-Api-Sdk). TikTok bietet keine offizielle LIVE-Event-API;
//! EulerStream liefert die Webcast-Events bereits dekodiert als JSON.

use std::time::Duration;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

use crate::hub::{Event, Platform, User};
use crate::state::{Shared, b, opt_s, s, u};

const WS_URL: &str = "wss://ws.eulerstream.com";

enum End {
    Closed(u16, String),
}

pub async fn run(st: Shared) {
    let mut backoff = 5u64;
    loop {
        let all = st.store.get();
        let api_key = all.eulerstream_key();
        let user = all.tiktok.username.trim().trim_start_matches('@').to_string();
        if user.is_empty() {
            st.hub.set_status(Platform::Tiktok, "unconfigured", false, "Bitte bei TikTok anmelden", None);
            return;
        }
        if api_key.is_empty() {
            st.hub.set_status(
                Platform::Tiktok,
                "error",
                false,
                "Kein EulerStream-Key hinterlegt (Einstellungen → Erweitert)",
                Some(format!("@{user}")),
            );
            return;
        }
        let account = Some(format!("@{user}"));
        st.hub.set_status(Platform::Tiktok, "connecting", false, "Verbinde mit EulerStream …", account.clone());
        let url = url::Url::parse_with_params(
            WS_URL,
            &[
                ("uniqueId", user.as_str()),
                ("apiKey", api_key.as_str()),
                ("features.bundleEvents", "true"),
            ],
        )
        .expect("gültige URL");

        let wait = match session(&st, url.as_str(), account.clone()).await {
            Ok(End::Closed(code, reason)) => {
                st.hub.reset_platform_stats(Platform::Tiktok);
                match code {
                    4404 => {
                        backoff = 5;
                        st.hub.set_status(
                            Platform::Tiktok,
                            "connected",
                            false,
                            format!("@{user} ist nicht live – prüfe jede Minute erneut"),
                            account,
                        );
                        60
                    }
                    4005 => {
                        st.hub.set_status(Platform::Tiktok, "connected", false, "Livestream beendet", account);
                        60
                    }
                    4401 | 4403 => {
                        st.hub.set_status(
                            Platform::Tiktok,
                            "error",
                            false,
                            format!("EulerStream: API-Key ungültig oder keine Berechtigung ({code})"),
                            account,
                        );
                        return;
                    }
                    4400 => {
                        st.hub.set_status(Platform::Tiktok, "error", false, format!("Ungültige Optionen: {reason}"), account);
                        return;
                    }
                    4429 => {
                        st.hub.set_status(
                            Platform::Tiktok,
                            "error",
                            false,
                            "Zu viele gleichzeitige Verbindungen für diesen API-Key – neuer Versuch in 60 s",
                            account,
                        );
                        60
                    }
                    _ => {
                        let w = backoff;
                        backoff = (backoff * 2).min(120);
                        st.hub.set_status(
                            Platform::Tiktok,
                            "error",
                            false,
                            format!("Verbindung getrennt ({code} {reason}) – neuer Versuch in {w}s"),
                            account,
                        );
                        w
                    }
                }
            }
            Err(e) => {
                let w = backoff;
                backoff = (backoff * 2).min(120);
                st.hub.set_status(Platform::Tiktok, "error", false, format!("{e:#} – neuer Versuch in {w}s"), account);
                st.hub.reset_platform_stats(Platform::Tiktok);
                w
            }
        };
        tokio::time::sleep(Duration::from_secs(wait)).await;
    }
}

async fn session(st: &Shared, url: &str, account: Option<String>) -> anyhow::Result<End> {
    let (ws, _) = tokio_tungstenite::connect_async(url).await.context("Verbindung zu EulerStream fehlgeschlagen")?;
    let (mut write, mut read) = ws.split();
    st.hub.set_status(Platform::Tiktok, "connected", false, "Verbunden – warte auf Raumdaten …", account.clone());
    let mut live_set = false;

    while let Some(msg) = read.next().await {
        let text = match msg? {
            Message::Text(t) => t.as_str().to_string(),
            Message::Binary(b) => match String::from_utf8(b.to_vec()) {
                Ok(t) => t,
                Err(_) => continue,
            },
            Message::Ping(p) => {
                write.send(Message::Pong(p)).await?;
                continue;
            }
            Message::Close(frame) => {
                let (code, reason) = frame.map(|f| (u16::from(f.code), f.reason.to_string())).unwrap_or((1000, String::new()));
                return Ok(End::Closed(code, reason));
            }
            _ => continue,
        };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let messages = match v.get("messages").and_then(|m| m.as_array()) {
            Some(list) => list.clone(),
            None => vec![v],
        };
        for m in &messages {
            if !live_set {
                live_set = true;
                st.hub.set_status(Platform::Tiktok, "connected", true, "Live – empfange Events", account.clone());
                // LIVE-Seite im Hintergrund vorladen, damit Chat-Senden sofort funktioniert
                if st.store.get().tiktok.session_id.is_some() {
                    match crate::tiktok_web::ensure_chat_window(st) {
                        Ok(_) => crate::tiktok_web::reload_chat_window(st),
                        Err(e) => log::warn!("TikTok-Chatfenster: {e}"),
                    }
                }
            }
            handle(st, m, &account);
        }
    }
    Ok(End::Closed(1006, "Verbindung verloren".into()))
}

fn tt_user(v: &Value) -> User {
    let unique = s(v, "/uniqueId");
    let nick = s(v, "/nickname");
    User {
        id: opt_s(v, "/userId").unwrap_or_else(|| unique.clone()),
        display_name: if nick.is_empty() { unique.clone() } else { nick },
        name: unique,
        avatar: opt_s(v, "/profilePicture/url/0"),
        ..Default::default()
    }
}

fn handle(st: &Shared, m: &Value, account: &Option<String>) {
    let typ = s(m, "/type");
    let d = &m["data"];
    let ev = match typ.as_str() {
        "WebcastChatMessage" => {
            let mut user = tt_user(&d["user"]);
            user.is_mod = b(d, "/userIdentity/isModeratorOfAnchor");
            user.is_subscriber = b(d, "/userIdentity/isSubscriberOfAnchor");
            user.is_owner = b(d, "/userIdentity/isAnchor");
            if user.is_mod {
                user.badges.push("moderator".into());
            }
            if user.is_subscriber {
                user.badges.push("subscriber".into());
            }
            let mut ev = Event::new(Platform::Tiktok, "chat").user(user).message(s(d, "/comment"));
            ev.data = json!({ "message_id": s(d, "/common/msgId") });
            ev
        }
        "WebcastGiftMessage" => {
            let gift = &d["giftDetails"];
            let streakable = b(gift, "/combo") || u(gift, "/giftType") == 1;
            let repeat_end = b(d, "/repeatEnd");
            if streakable && !repeat_end {
                return; // Streak läuft noch – erst am Ende ein Alert
            }
            let repeat = u(d, "/repeatCount").max(1);
            let diamonds = u(gift, "/diamondCount");
            let coins = diamonds * repeat;
            if coins < st.store.get().alerts.tiktok_min_coins {
                return;
            }
            let mut ev = Event::new(Platform::Tiktok, "gift").user(tt_user(&d["user"])).amount(repeat as f64);
            ev.coins = Some(coins);
            ev.gift_name = opt_s(gift, "/giftName");
            ev.gift_image = opt_s(gift, "/icon/url/0");
            ev.data = json!({ "gift_id": u(d, "/giftId"), "diamonds_per_gift": diamonds });
            ev
        }
        "WebcastLikeMessage" => {
            st.hub.add_likes(Platform::Tiktok, Some(tt_user(&d["user"])), u(d, "/likeCount"));
            let total = u(d, "/totalLikeCount");
            if total > 0 {
                st.hub.set_total_likes(Platform::Tiktok, total);
            }
            return;
        }
        "WebcastSocialMessage" => {
            let display = d["common"]["displayText"].to_string().to_lowercase();
            if display.contains("follow") {
                Event::new(Platform::Tiktok, "follow").user(tt_user(&d["user"]))
            } else if display.contains("share") {
                Event::new(Platform::Tiktok, "share").user(tt_user(&d["user"]))
            } else {
                return;
            }
        }
        "WebcastSubNotifyMessage" => Event::new(Platform::Tiktok, "subscribe").user(tt_user(&d["user"])),
        "WebcastMemberMessage" => Event::new(Platform::Tiktok, "join").user(tt_user(&d["user"])),
        "WebcastRoomUserSeqMessage" => {
            st.hub.set_viewers(Platform::Tiktok, Some(u(d, "/viewerCount")));
            return;
        }
        "WebcastControlMessage" => {
            let action = u(d, "/action");
            if action == 3 || action == 4 {
                st.hub.set_status(Platform::Tiktok, "connected", false, "Livestream beendet", account.clone());
                st.hub.set_viewers(Platform::Tiktok, None);
            }
            return;
        }
        "room.status" => {
            let state = s(d, "/state");
            let live = state == "connected";
            let msg = match state.as_str() {
                "connected" => "Live – empfange Events".to_string(),
                "offline" => "Nicht live".to_string(),
                "ended" => "Livestream beendet".to_string(),
                other => format!("Status: {other}"),
            };
            st.hub.set_status(Platform::Tiktok, "connected", live, msg, account.clone());
            return;
        }
        _ => return,
    };
    st.hub.publish(ev);
}
