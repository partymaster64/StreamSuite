//! Startet/stoppt die Hintergrund-Tasks der einzelnen Plattformen.

use crate::hub::Platform;
use crate::state::Shared;
use crate::{tiktok, twitch, youtube};

pub const ALL: [Platform; 3] = [Platform::Twitch, Platform::Youtube, Platform::Tiktok];

pub fn start(st: &Shared, p: Platform) {
    stop(st, p);
    let st2 = st.clone();
    let handle = match p {
        Platform::Twitch => tauri::async_runtime::spawn(twitch::run(st2)),
        Platform::Youtube => tauri::async_runtime::spawn(youtube::run(st2)),
        Platform::Tiktok => tauri::async_runtime::spawn(tiktok::run(st2)),
        Platform::System => return,
    };
    st.tasks.lock().insert(p, handle);
}

pub fn stop(st: &Shared, p: Platform) {
    if let Some(h) = st.tasks.lock().remove(&p) {
        h.abort();
    }
    if p == Platform::Youtube {
        *st.youtube_live.lock() = None;
    }
}

/// Initialer Status + Autostart aller aktivierten und eingerichteten Plattformen.
pub fn start_enabled(st: &Shared) {
    let cfg = st.store.get();
    for p in ALL {
        let (enabled, ready, account) = match p {
            Platform::Twitch => (cfg.twitch.enabled, cfg.twitch.access_token.is_some(), cfg.twitch.display_name.clone()),
            Platform::Youtube => (cfg.youtube.enabled, cfg.youtube.refresh_token.is_some(), cfg.youtube.channel_title.clone()),
            Platform::Tiktok => (
                cfg.tiktok.enabled,
                !cfg.tiktok.username.trim().is_empty(),
                Some(format!("@{}", cfg.tiktok.username.trim().trim_start_matches('@'))).filter(|a| a.len() > 1),
            ),
            Platform::System => continue,
        };
        if !ready {
            st.hub.set_status(p, "unconfigured", false, "Noch nicht eingerichtet", None);
        } else if enabled {
            start(st, p);
        } else {
            st.hub.set_status(p, "disconnected", false, "Getrennt", account);
        }
    }
}

pub fn set_enabled(st: &Shared, p: Platform, enabled: bool) {
    st.store.update(|s| match p {
        Platform::Twitch => s.twitch.enabled = enabled,
        Platform::Youtube => s.youtube.enabled = enabled,
        Platform::Tiktok => s.tiktok.enabled = enabled,
        Platform::System => {}
    });
}

pub fn disconnect(st: &Shared, p: Platform) {
    stop(st, p);
    set_enabled(st, p, false);
    let cfg = st.store.get();
    let account = match p {
        Platform::Twitch => cfg.twitch.display_name,
        Platform::Youtube => cfg.youtube.channel_title,
        Platform::Tiktok => Some(format!("@{}", cfg.tiktok.username)),
        Platform::System => None,
    };
    st.hub.reset_platform_stats(p);
    st.hub.set_status(p, "disconnected", false, "Getrennt", account);
}

/// Trennt und entfernt die gespeicherten Zugangsdaten (Tokens).
pub fn logout(st: &Shared, p: Platform) {
    stop(st, p);
    if p == Platform::Tiktok {
        crate::tiktok_web::logout(st);
    }
    st.store.update(|s| match p {
        Platform::Twitch => {
            s.twitch.enabled = false;
            s.twitch.access_token = None;
            s.twitch.refresh_token = None;
            s.twitch.user_id = None;
            s.twitch.login = None;
            s.twitch.display_name = None;
        }
        Platform::Youtube => {
            s.youtube.enabled = false;
            s.youtube.access_token = None;
            s.youtube.refresh_token = None;
            s.youtube.expires_at = 0;
            s.youtube.channel_id = None;
            s.youtube.channel_title = None;
        }
        Platform::Tiktok => {
            s.tiktok.enabled = false;
            s.tiktok.session_id = None;
            s.tiktok.username = String::new();
        }
        Platform::System => {}
    });
    st.hub.reset_platform_stats(p);
    st.hub.set_status(p, "unconfigured", false, "Abgemeldet", None);
}
