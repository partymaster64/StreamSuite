//! Zentraler Event-Hub: vereinheitlicht Chat, Alerts und Statistiken aller Plattformen,
//! hält einen Verlauf und verteilt Events an die App (Tauri-Events) und an die Overlays (WebSocket).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;

pub const CHAT_HISTORY: usize = 300;
pub const ALERT_HISTORY: usize = 150;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    Youtube,
    Tiktok,
    System,
}

impl Platform {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "twitch" => Some(Self::Twitch),
            "youtube" => Some(Self::Youtube),
            "tiktok" => Some(Self::Tiktok),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct User {
    pub id: String,
    pub name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub badges: Vec<String>,
    pub is_mod: bool,
    pub is_subscriber: bool,
    pub is_owner: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Part {
    #[serde(rename = "type")]
    pub kind: &'static str, // "text" | "emote"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Einheitliches Event-Format – genau so landet es auch in den Overlays.
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: String,
    pub platform: Platform,
    /// chat | delete | follow | subscribe | resub | gift_sub | membership | cheer | superchat |
    /// supersticker | gift | like | share | raid | join | stats
    #[serde(rename = "type")]
    pub kind: String,
    /// "chat" | "alert" | "system"
    pub category: &'static str,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    /// Generische Menge: Likes, Bits, Monate, Raid-Zuschauer, Geschenk-Anzahl …
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_display: Option<String>,
    /// TikTok-Coins (Diamanten) gesamt für dieses Event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coins: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gift_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gift_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub data: Value,
    /// true bei Test-Alerts aus der App.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,
}

impl Event {
    pub fn new(platform: Platform, kind: &str) -> Self {
        let category = match kind {
            "chat" | "delete" => "chat",
            "join" | "stats" => "system",
            _ => "alert",
        };
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            platform,
            kind: kind.to_string(),
            category,
            timestamp: chrono::Utc::now().timestamp_millis(),
            user: None,
            message: None,
            parts: Vec::new(),
            amount: None,
            currency: None,
            amount_display: None,
            coins: None,
            gift_name: None,
            gift_image: None,
            tier: None,
            data: Value::Null,
            test: false,
        }
    }
    pub fn user(mut self, u: User) -> Self {
        self.user = Some(u);
        self
    }
    pub fn message(mut self, m: impl Into<String>) -> Self {
        let m = m.into();
        if !m.is_empty() {
            self.message = Some(m);
        }
        self
    }
    pub fn amount(mut self, a: f64) -> Self {
        self.amount = Some(a);
        self
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PlatformStats {
    pub viewers: Option<u64>,
    pub total_likes: Option<u64>,
    pub session_likes: u64,
    pub session_follows: u64,
    pub session_subs: u64,
    pub session_coins: u64,
    pub session_bits: u64,
    pub session_messages: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnStatus {
    pub platform: Platform,
    /// unconfigured | disconnected | connecting | connected | error
    pub state: String,
    pub live: bool,
    pub message: String,
    pub account: Option<String>,
}

#[derive(Default)]
struct LikeBucket {
    total: u64,
    per_platform: HashMap<&'static str, u64>,
    per_user: HashMap<String, (User, &'static str, u64)>,
}

pub struct Hub {
    app: AppHandle,
    tx: broadcast::Sender<Event>,
    chat: Mutex<VecDeque<Event>>,
    alerts: Mutex<VecDeque<Event>>,
    stats: Mutex<HashMap<Platform, PlatformStats>>,
    status: Mutex<HashMap<Platform, ConnStatus>>,
    likes: Mutex<LikeBucket>,
}

fn pkey(p: Platform) -> &'static str {
    match p {
        Platform::Twitch => "twitch",
        Platform::Youtube => "youtube",
        Platform::Tiktok => "tiktok",
        Platform::System => "system",
    }
}

impl Hub {
    pub fn new(app: AppHandle) -> Arc<Self> {
        let (tx, _) = broadcast::channel(1024);
        Arc::new(Self {
            app,
            tx,
            chat: Mutex::new(VecDeque::new()),
            alerts: Mutex::new(VecDeque::new()),
            stats: Mutex::new(HashMap::new()),
            status: Mutex::new(HashMap::new()),
            likes: Mutex::new(LikeBucket::default()),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub fn publish(&self, ev: Event) {
        // Session-Statistiken
        if !ev.test {
            let mut stats = self.stats.lock();
            let s = stats.entry(ev.platform).or_default();
            match ev.kind.as_str() {
                "chat" => s.session_messages += 1,
                "follow" => s.session_follows += 1,
                "subscribe" | "resub" | "membership" => s.session_subs += 1,
                "gift_sub" => s.session_subs += ev.amount.unwrap_or(1.0) as u64,
                "gift" => s.session_coins += ev.coins.unwrap_or(0),
                "cheer" => s.session_bits += ev.amount.unwrap_or(0.0) as u64,
                _ => {}
            }
        }
        match ev.category {
            "chat" => push_capped(&mut self.chat.lock(), ev.clone(), CHAT_HISTORY),
            "alert" => push_capped(&mut self.alerts.lock(), ev.clone(), ALERT_HISTORY),
            _ => {}
        }
        let _ = self.app.emit("ss://event", &ev);
        let _ = self.tx.send(ev);
    }

    pub fn history(&self) -> Value {
        json!({
            "chat": self.chat.lock().iter().cloned().collect::<Vec<_>>(),
            "alerts": self.alerts.lock().iter().cloned().collect::<Vec<_>>(),
        })
    }

    pub fn clear_history(&self) {
        self.chat.lock().clear();
        self.alerts.lock().clear();
    }

    // ---------- Statistiken ----------

    pub fn stats_json(&self) -> Value {
        let stats = self.stats.lock();
        let mut total_viewers = 0u64;
        let mut total_likes = 0u64;
        let mut per = serde_json::Map::new();
        for (p, s) in stats.iter() {
            total_viewers += s.viewers.unwrap_or(0);
            total_likes += s.total_likes.unwrap_or(0);
            per.insert(pkey(*p).to_string(), serde_json::to_value(s).unwrap_or_default());
        }
        json!({ "viewers": total_viewers, "likes": total_likes, "platforms": per })
    }

    fn emit_stats(&self) {
        let mut ev = Event::new(Platform::System, "stats");
        ev.data = self.stats_json();
        self.publish(ev);
    }

    pub fn set_viewers(&self, p: Platform, viewers: Option<u64>) {
        let changed = {
            let mut stats = self.stats.lock();
            let s = stats.entry(p).or_default();
            let changed = s.viewers != viewers;
            s.viewers = viewers;
            changed
        };
        if changed {
            self.emit_stats();
        }
    }

    pub fn set_total_likes(&self, p: Platform, likes: u64) {
        let changed = {
            let mut stats = self.stats.lock();
            let s = stats.entry(p).or_default();
            let changed = s.total_likes != Some(likes);
            s.total_likes = Some(likes);
            changed
        };
        if changed {
            self.emit_stats();
        }
    }

    pub fn reset_platform_stats(&self, p: Platform) {
        if let Some(s) = self.stats.lock().get_mut(&p) {
            s.viewers = None;
        }
        self.emit_stats();
    }

    // ---------- Kombinierte Likes ----------

    /// Likes einer Plattform registrieren; werden periodisch zu einem Alert zusammengefasst.
    pub fn add_likes(&self, p: Platform, user: Option<User>, count: u64) {
        if count == 0 {
            return;
        }
        {
            let mut stats = self.stats.lock();
            stats.entry(p).or_default().session_likes += count;
        }
        let mut b = self.likes.lock();
        b.total += count;
        *b.per_platform.entry(pkey(p)).or_insert(0) += count;
        if let Some(u) = user {
            let key = format!("{}:{}", pkey(p), u.id);
            let e = b.per_user.entry(key).or_insert((u, pkey(p), 0));
            e.2 += count;
        }
    }

    /// Wird vom Like-Aggregator-Task aufgerufen.
    pub fn flush_likes(&self, min_count: u64) {
        let bucket = {
            let mut b = self.likes.lock();
            if b.total == 0 || b.total < min_count {
                return;
            }
            std::mem::take(&mut *b)
        };
        let mut users: Vec<_> = bucket.per_user.into_values().collect();
        users.sort_by(|a, b| b.2.cmp(&a.2));
        let top: Vec<Value> = users
            .iter()
            .take(10)
            .map(|(u, p, c)| json!({ "user": u, "platform": p, "count": c }))
            .collect();
        let mut ev = Event::new(Platform::System, "like").amount(bucket.total as f64);
        if let Some((u, _, _)) = users.first() {
            ev.user = Some(u.clone());
        }
        ev.data = json!({ "platforms": bucket.per_platform, "top": top });
        self.emit_stats();
        self.publish(ev);
    }

    // ---------- Verbindungsstatus ----------

    pub fn set_status(&self, p: Platform, state: &str, live: bool, message: impl Into<String>, account: Option<String>) {
        let st = ConnStatus {
            platform: p,
            state: state.to_string(),
            live,
            message: message.into(),
            account,
        };
        self.status.lock().insert(p, st.clone());
        let _ = self.app.emit("ss://status", &st);
    }

    pub fn statuses(&self) -> Vec<ConnStatus> {
        self.status.lock().values().cloned().collect()
    }

    pub fn emit_raw<S: Serialize + Clone>(&self, name: &str, payload: S) {
        let _ = self.app.emit(name, payload);
    }
}

fn push_capped(q: &mut VecDeque<Event>, ev: Event, cap: usize) {
    q.push_back(ev);
    while q.len() > cap {
        q.pop_front();
    }
}
