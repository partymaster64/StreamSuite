//! Persistente Einstellungen (JSON-Datei im App-Konfigurationsordner).
//! Alles bleibt lokal auf dem Rechner des Nutzers.

use std::path::PathBuf;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server_port: u16,
    /// Beim Schließen des Fensters im Hintergrund (Tray/Dock) weiterlaufen.
    pub close_to_tray: bool,
    pub twitch: TwitchSettings,
    pub youtube: YoutubeSettings,
    pub tiktok: TiktokSettings,
    pub alerts: AlertSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_port: 4848,
            close_to_tray: true,
            twitch: TwitchSettings::default(),
            youtube: YoutubeSettings::default(),
            tiktok: TiktokSettings::default(),
            alerts: AlertSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TwitchSettings {
    pub enabled: bool,
    pub client_id: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub user_id: Option<String>,
    pub login: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct YoutubeSettings {
    pub enabled: bool,
    pub client_id: String,
    pub client_secret: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    /// Unix-Zeitstempel (Sekunden) wann der Access-Token abläuft.
    pub expires_at: i64,
    pub channel_id: Option<String>,
    pub channel_title: Option<String>,
    /// Minimales Abfrageintervall für den Live-Chat (ms). Schont das API-Kontingent.
    pub min_chat_poll_ms: u64,
    /// Intervall für die Abfrage neuer Abonnenten (Sekunden).
    pub subscriber_poll_secs: u64,
}

impl Default for YoutubeSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            client_id: String::new(),
            client_secret: String::new(),
            access_token: None,
            refresh_token: None,
            expires_at: 0,
            channel_id: None,
            channel_title: None,
            min_chat_poll_ms: 8000,
            subscriber_poll_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TiktokSettings {
    pub enabled: bool,
    pub username: String,
    /// Sitzungs-Cookie nach dem Login im TikTok-Fenster (nur Marker, das Fenster hat eigene Cookies).
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertSettings {
    /// Likes werden plattformübergreifend gesammelt und alle N Sekunden als ein Alert ausgegeben.
    pub like_interval_secs: u64,
    /// Mindestanzahl Likes damit ein kombinierter Like-Alert ausgelöst wird.
    pub like_min_count: u64,
    /// TikTok-Geschenke unter diesem Coin-Wert lösen keinen Alert aus (werden aber gezählt).
    pub tiktok_min_coins: u64,
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            like_interval_secs: 15,
            like_min_count: 1,
            tiktok_min_coins: 1,
        }
    }
}

impl Settings {
    /// Kopie ohne Geheimnisse – für das Frontend.
    pub fn redacted(&self) -> serde_json::Value {
        let mut s = self.clone();
        let tw_auth = s.twitch.access_token.is_some();
        let yt_auth = s.youtube.refresh_token.is_some();
        let tt_auth = s.tiktok.session_id.is_some();
        let (tw_ready, yt_ready) = (
            !self.twitch_client_id().is_empty(),
            !self.google_client().0.is_empty() && !self.google_client().1.is_empty(),
        );
        s.twitch.access_token = None;
        s.twitch.refresh_token = None;
        s.youtube.access_token = None;
        s.youtube.refresh_token = None;
        s.tiktok.session_id = None;
        let mut v = serde_json::to_value(&s).unwrap_or_default();
        v["twitch"]["authorized"] = tw_auth.into();
        v["youtube"]["authorized"] = yt_auth.into();
        v["tiktok"]["authorized"] = tt_auth.into();
        // Sind App-Zugangsdaten vorhanden (eingebettet oder eigene)?
        v["twitch"]["credentials_ok"] = tw_ready.into();
        v["youtube"]["credentials_ok"] = yt_ready.into();
        // TikTok braucht keine App-Zugangsdaten (direkte Webcast-Verbindung)
        v["tiktok"]["credentials_ok"] = true.into();
        v["embedded"] = serde_json::json!({
            "twitch": !EMBEDDED_TWITCH_CLIENT_ID.is_empty(),
            "youtube": !EMBEDDED_GOOGLE_CLIENT_ID.is_empty() && !EMBEDDED_GOOGLE_CLIENT_SECRET.is_empty(),
        });
        v
    }

    pub fn twitch_client_id(&self) -> String {
        pick(&self.twitch.client_id, EMBEDDED_TWITCH_CLIENT_ID)
    }

    /// (client_id, client_secret) – eigene Werte nur, wenn beide gesetzt sind.
    pub fn google_client(&self) -> (String, String) {
        let (id, secret) = (self.youtube.client_id.trim(), self.youtube.client_secret.trim());
        if !id.is_empty() && !secret.is_empty() {
            (id.to_string(), secret.to_string())
        } else {
            (EMBEDDED_GOOGLE_CLIENT_ID.to_string(), EMBEDDED_GOOGLE_CLIENT_SECRET.to_string())
        }
    }
}

// Beim Build eingebettete App-Zugangsdaten (siehe build.rs / credentials.json).
const EMBEDDED_TWITCH_CLIENT_ID: &str = env!("SS_TWITCH_CLIENT_ID");
const EMBEDDED_GOOGLE_CLIENT_ID: &str = env!("SS_GOOGLE_CLIENT_ID");
const EMBEDDED_GOOGLE_CLIENT_SECRET: &str = env!("SS_GOOGLE_CLIENT_SECRET");

fn pick(own: &str, embedded: &str) -> String {
    let own = own.trim();
    if own.is_empty() { embedded.to_string() } else { own.to_string() }
}

pub struct Store {
    path: PathBuf,
    inner: RwLock<Settings>,
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        let settings = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
            .unwrap_or_default();
        Self {
            path,
            inner: RwLock::new(settings),
        }
    }

    pub fn get(&self) -> Settings {
        self.inner.read().clone()
    }

    pub fn update<F: FnOnce(&mut Settings)>(&self, f: F) -> Settings {
        let snapshot = {
            let mut guard = self.inner.write();
            f(&mut guard);
            guard.clone()
        };
        if let Err(e) = self.save(&snapshot) {
            log::error!("Einstellungen konnten nicht gespeichert werden: {e}");
        }
        snapshot
    }

    fn save(&self, s: &Settings) -> anyhow::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
