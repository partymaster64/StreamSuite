use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::Value;
use tauri::async_runtime::JoinHandle;

use crate::config::Store;
use crate::hub::{Hub, Platform};

#[derive(Debug, Clone)]
pub struct YoutubeLive {
    pub chat_id: String,
    pub video_id: String,
}

pub struct AppState {
    pub app: tauri::AppHandle,
    pub store: Store,
    pub hub: Arc<Hub>,
    pub http: reqwest::Client,
    pub overlays_dir: PathBuf,
    pub tasks: Mutex<HashMap<Platform, JoinHandle<()>>>,
    pub youtube_live: Mutex<Option<YoutubeLive>>,
    /// Verhindert parallele Token-Refreshes.
    pub twitch_refresh: tokio::sync::Mutex<()>,
    pub youtube_refresh: tokio::sync::Mutex<()>,
    /// Tatsächlich gebundener Port des Overlay-Servers.
    pub server_port: Mutex<Option<u16>>,
    /// Offene Sendeaufträge an das TikTok-Fenster (id -> Antwort).
    pub tiktok_pending: Mutex<HashMap<String, tokio::sync::oneshot::Sender<Result<(), String>>>>,
    /// Vom TikTok-Fenster erkannter Benutzername.
    pub tiktok_detected_user: Mutex<Option<String>>,
}

pub type Shared = Arc<AppState>;

// ---------- kleine JSON-Helfer (Plattform-APIs liefern Zahlen mal als String, mal als Number) ----------

pub fn s(v: &Value, ptr: &str) -> String {
    match v.pointer(ptr) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

pub fn opt_s(v: &Value, ptr: &str) -> Option<String> {
    let x = s(v, ptr);
    if x.is_empty() { None } else { Some(x) }
}

pub fn u(v: &Value, ptr: &str) -> u64 {
    match v.pointer(ptr) {
        Some(Value::Number(n)) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)).unwrap_or(0),
        Some(Value::String(x)) => x.parse().unwrap_or(0),
        Some(Value::Bool(b)) => *b as u64,
        _ => 0,
    }
}

pub fn b(v: &Value, ptr: &str) -> bool {
    match v.pointer(ptr) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(x)) => x == "true" || x == "1",
        _ => false,
    }
}

pub fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}
