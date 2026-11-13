//! Steuerung des eigenen TikTok-LIVE: Raum-Infos, Kommentar-Einstellungen, Moderation und Wortfilter.
//!
//! - Lesen (Raum-Infos) geht ohne Anmeldung direkt an die Webcast-API.
//! - Schreibende Aktionen brauchen die eigene Creator-Sitzung. Sie laufen im versteckten, eingeloggten
//!   TikTok-Fenster (`tiktok_web::page_request`), damit TikTok die Anfrage wie aus dem Browser erhält.
//!   Es ist kein externer Signier-Dienst beteiligt.
//!
//! Endpunkte & Parameter entsprechen dem TikTok-LIVE-Studio-Protokoll, wie es
//! [Loukious/TikTokStreamKeyGenerator](https://github.com/Loukious/TikTokStreamKeyGenerator) dokumentiert
//! (`Libs/live_management.py`), hier über den Web-Client (aid 1988).
//!
//! Titel und Thema eines laufenden LIVE lassen sich darüber NICHT ändern: TikTok legt sie beim Erstellen
//! des Raums fest (`webcast/room/create/`), und das geht nur mit dem signierten LIVE-Studio-Client.

use anyhow::{Context, anyhow, bail};
use piratetok_live_rs::http::api::{FetchParams, fetch_room_info};
use serde_json::{Value, json};

use crate::state::{Shared, b, opt_s, s, u};
use crate::tiktok::TiktokRoom;
use crate::tiktok_web::{PageBody, page_request};

fn room(st: &Shared) -> anyhow::Result<TiktokRoom> {
    st.tiktok_room
        .lock()
        .clone()
        .ok_or_else(|| anyhow!("Du bist auf TikTok gerade nicht live"))
}

fn require_session(st: &Shared) -> anyhow::Result<()> {
    if st.store.get().tiktok.session_id.is_none() {
        bail!("Für diese Funktion bitte unter Verbindungen bei TikTok anmelden");
    }
    Ok(())
}

fn numeric_id(id: &str, what: &str) -> anyhow::Result<String> {
    let id = id.trim();
    if id.is_empty() || id.len() > 20 || !id.bytes().all(|c| c.is_ascii_digit()) || id.bytes().all(|c| c == b'0') {
        bail!("Ungültige {what}");
    }
    Ok(id.to_string())
}

/// Aktuelle Daten des eigenen LIVE-Raums (Titel, Thema, Zuschauer, Kommentar-Einstellungen).
pub async fn room_info(st: &Shared) -> anyhow::Result<Value> {
    let Some(room) = st.tiktok_room.lock().clone() else {
        return Ok(json!({ "live": false }));
    };
    let info = fetch_room_info(&room.room_id, FetchParams::default())
        .await
        .context("Raum-Infos konnten nicht geladen werden")?;
    let v: Value = serde_json::from_str(&info.raw_json).unwrap_or(Value::Null);
    let d = &v["data"];
    if let Some(sec) = opt_s(d, "/owner/sec_uid") {
        *st.tiktok_sec_uid.lock() = Some(sec);
    }
    Ok(json!({
        "live": u(d, "/status") == 2,
        "room_id": room.room_id,
        "title": info.title,
        "topic": opt_s(d, "/hashtag/title"),
        "viewers": info.viewers,
        "likes": u(d, "/like_count").max(info.likes.max(0) as u64),
        "total_viewers": info.total_viewers,
        "started_at": u(d, "/create_time"),
        "share_url": opt_s(d, "/share_url"),
        "cover": opt_s(d, "/cover/url_list/0"),
        "comments_enabled": b(d, "/room_auth/Chat"),
        "comment_filter": b(d, "/room_auth/ChatL2"),
        "logged_in": st.store.get().tiktok.session_id.is_some(),
    }))
}

fn form(fields: Vec<(&str, String)>) -> PageBody {
    PageBody::Form(fields.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// JSON-Objekt als Formularfelder (für den Fallback ohne CORS-Vorabcheck).
fn json_as_form(v: &Value) -> Vec<(String, String)> {
    v.as_object()
        .map(|o| {
            o.iter()
                .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// Webcast-Aufruf in der eingeloggten Seite inkl. Auswertung des TikTok-Statuscodes.
async fn call(st: &Shared, action: &str, method: &str, path: &str, query: &[(&str, String)], body: PageBody) -> anyhow::Result<Value> {
    require_session(st)?;
    let json_fallback = match &body {
        PageBody::Json(v) => Some(json_as_form(v)),
        _ => None,
    };
    let (mut status, mut text) = page_request(st, method, path, query, body).await?;
    // JSON-POSTs lösen einen CORS-Vorabcheck aus. Lehnt TikTok den ab, wurde die Anfrage nie gesendet →
    // einmal als Formular wiederholen. (Die JSON-Aktionen – stummschalten/entfernen – sind ohnehin idempotent.)
    if status == 0
        && let Some(fields) = json_fallback
    {
        (status, text) = page_request(st, method, path, query, PageBody::Form(fields)).await?;
    }
    match status {
        200..=299 => {}
        0 => bail!("{action}: keine Antwort von TikTok (Netzwerk oder vom Browser blockiert)"),
        -3 => bail!("{action}: TikTok-Seite nicht geladen – bist du eingeloggt? (Verbindungen → TikTok-LIVE-Fenster anzeigen)"),
        s if s < 0 => bail!("{action}: Zeitüberschreitung im TikTok-Fenster"),
        s => bail!("{action}: TikTok antwortet mit HTTP {s}"),
    }
    if text.trim().is_empty() {
        bail!("{action}: TikTok hat die Anfrage abgelehnt (leere Antwort – Sitzung abgelaufen oder Sicherheitsprüfung nötig)");
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| anyhow!("{action}: unerwartete Antwort von TikTok"))?;
    let code = u(&v, "/status_code");
    if v.get("status_code").is_none() {
        bail!("{action}: unerwartete Antwort von TikTok");
    }
    if code != 0 {
        let reason = ["/data/prompts", "/data/message", "/status_msg", "/message", "/data/description"]
            .iter()
            .find_map(|p| opt_s(&v, p))
            .unwrap_or_else(|| "TikTok hat die Aktion abgelehnt".into());
        bail!("{action}: {reason} (Code {code})");
    }
    Ok(v.get("data").cloned().unwrap_or(Value::Null))
}

/// Kommentare im LIVE erlauben/sperren (`switch_type` 3) bzw. Kommentarfilter (`switch_type` 4).
pub async fn set_room_switch(st: &Shared, setting: &str, enabled: bool) -> anyhow::Result<()> {
    let (switch_type, action) = match setting {
        "comments" => ("3", if enabled { "Kommentare erlauben" } else { "Kommentare sperren" }),
        "comment_filter" => ("4", "Kommentarfilter ändern"),
        other => bail!("Unbekannte Einstellung: {other}"),
    };
    let room = room(st)?;
    call(
        st,
        action,
        "POST",
        "room/switch/update/",
        &[],
        form(vec![
            ("room_id", room.room_id),
            ("switch_type", switch_type.into()),
            ("switch_value", enabled.to_string()),
        ]),
    )
    .await?;
    Ok(())
}

/// Zuschauer stummschalten, freigeben, aus dem LIVE werfen/blockieren oder wieder zulassen.
/// `duration`: Sekunden für `mute` (0 = für den ganzen LIVE).
pub async fn moderate(st: &Shared, user_id: &str, action: &str, duration: Option<u64>) -> anyhow::Result<()> {
    let user_id = numeric_id(user_id, "Nutzer-ID")?;
    let room = room(st)?;
    if user_id == room.anchor_id {
        bail!("Du kannst dich nicht selbst moderieren");
    }
    match action {
        "mute" => {
            let duration = duration.unwrap_or(300);
            call(
                st,
                "Stummschalten",
                "POST",
                "room/silence/",
                &[],
                PageBody::Json(json!({
                    "room_id": room.room_id,
                    "user_id": user_id,
                    "duration": duration,
                    "silence_type": i32::from(duration != 0),
                })),
            )
            .await?;
        }
        "unmute" => {
            call(
                st,
                "Stummschaltung aufheben",
                "POST",
                "room/unsilence/",
                &[],
                PageBody::Json(json!({ "room_id": room.room_id, "user_id": user_id })),
            )
            .await?;
        }
        "block" => {
            call(
                st,
                "Aus dem LIVE entfernen",
                "POST",
                "room/kick/user/",
                &[],
                PageBody::Json(json!({ "room_id": room.room_id, "kick_uid": user_id })),
            )
            .await?;
        }
        "unblock" => {
            call(
                st,
                "Wieder zulassen",
                "POST",
                "room/unkick/user/",
                &[],
                form(vec![("room_id", room.room_id), ("kick_uid", user_id)]),
            )
            .await?;
        }
        other => bail!("Unbekannte Moderationsaktion: {other}"),
    }
    Ok(())
}

/// Moderatoren des eigenen Kanals.
pub async fn moderators(st: &Shared) -> anyhow::Result<Value> {
    let room = room(st)?;
    let data = call(
        st,
        "Moderatoren laden",
        "GET",
        "user/admin/list/",
        &[("anchor_id", room.anchor_id), ("room_id", room.room_id)],
        PageBody::None,
    )
    .await?;
    let list = data.as_array().cloned().unwrap_or_default();
    Ok(Value::Array(
        list.iter()
            .map(|e| {
                let u = &e["user_info"];
                let id = opt_s(u, "/id_str").unwrap_or_else(|| s(u, "/id"));
                json!({
                    "id": id,
                    "name": opt_s(u, "/display_id").unwrap_or_else(|| id.clone()),
                    "display_name": opt_s(u, "/nickname").or_else(|| opt_s(u, "/display_id")).unwrap_or_else(|| id.clone()),
                    "avatar": opt_s(u, "/avatar_thumb/url_list/0"),
                })
            })
            .collect(),
    ))
}

pub async fn set_moderator(st: &Shared, user_id: &str, enabled: bool) -> anyhow::Result<()> {
    let user_id = numeric_id(user_id, "Nutzer-ID")?;
    let room = room(st)?;
    if user_id == room.anchor_id {
        bail!("Du kannst dich nicht selbst zum Moderator machen");
    }
    call(
        st,
        if enabled { "Moderator hinzufügen" } else { "Moderator entfernen" },
        "POST",
        "user/admin/update/",
        &[],
        form(vec![
            ("anchor_id", room.anchor_id),
            ("to_user_id", user_id),
            ("update_type", if enabled { "3" } else { "2" }.into()),
            ("current_room_id", room.room_id),
        ]),
    )
    .await?;
    Ok(())
}

async fn sec_uid(st: &Shared) -> anyhow::Result<String> {
    if let Some(sec) = st.tiktok_sec_uid.lock().clone() {
        return Ok(sec);
    }
    room_info(st).await?;
    st.tiktok_sec_uid
        .lock()
        .clone()
        .ok_or_else(|| anyhow!("Konto-Kennung (sec_uid) konnte nicht ermittelt werden"))
}

/// Gesperrte Wörter (Kommentarfilter) des laufenden LIVE.
pub async fn blocked_words(st: &Shared) -> anyhow::Result<Value> {
    let room = room(st)?;
    let sec = sec_uid(st).await?;
    let data = call(
        st,
        "Gesperrte Wörter laden",
        "GET",
        "room/get_sensitive_word/",
        &[("sec_anchor_id", sec), ("room_id", room.room_id)],
        PageBody::None,
    )
    .await?;
    let words = data["word"].as_array().cloned().unwrap_or_default();
    Ok(Value::Array(
        words
            .iter()
            .filter_map(|w| {
                let id = opt_s(w, "/id")?;
                Some(json!({ "id": id, "word": s(w, "/content") }))
            })
            .collect(),
    ))
}

pub async fn add_blocked_word(st: &Shared, word: &str) -> anyhow::Result<Value> {
    let word = word.trim();
    if word.is_empty() {
        bail!("Bitte ein Wort eingeben");
    }
    if word.encode_utf16().count() > 30 {
        bail!("Ein gesperrtes Wort darf höchstens 30 Zeichen lang sein");
    }
    let room = room(st)?;
    let sec = sec_uid(st).await?;
    let data = call(
        st,
        "Wort sperren",
        "POST",
        "room/add_sensitive_word/",
        &[],
        form(vec![("word", word.to_string()), ("sec_anchor_id", sec), ("room_id", room.room_id)]),
    )
    .await?;
    Ok(json!({ "id": s(&data, "/id"), "word": word }))
}

pub async fn delete_blocked_word(st: &Shared, word_id: &str) -> anyhow::Result<()> {
    let word_id = numeric_id(word_id, "Wort-ID")?;
    let room = room(st)?;
    let sec = sec_uid(st).await?;
    call(
        st,
        "Wort freigeben",
        "POST",
        "room/del_sensitive_word/",
        &[],
        form(vec![("word_id", word_id), ("sec_anchor_id", sec), ("room_id", room.room_id)]),
    )
    .await?;
    Ok(())
}
