//! Bettet die App-Zugangsdaten (Twitch Client-ID, Google OAuth-Client) beim Build ein,
//! damit Endnutzer sich nur noch per Klick anmelden müssen.
//!
//! Quelle (in dieser Reihenfolge): Umgebungsvariablen, sonst `credentials.json` neben dieser Datei.
//! Vorlage: `credentials.example.json`. Die echte Datei ist in .gitignore eingetragen.

use std::collections::HashMap;

const KEYS: &[(&str, &str)] = &[
    ("SS_TWITCH_CLIENT_ID", "twitch_client_id"),
    ("SS_GOOGLE_CLIENT_ID", "google_client_id"),
    ("SS_GOOGLE_CLIENT_SECRET", "google_client_secret"),
];

fn main() {
    println!("cargo:rerun-if-changed=credentials.json");
    let file: HashMap<String, String> = std::fs::read_to_string("credentials.json")
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    for (env, key) in KEYS {
        println!("cargo:rerun-if-env-changed={env}");
        let value = std::env::var(env)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| file.get(*key).cloned())
            .unwrap_or_default();
        println!("cargo:rustc-env={env}={}", value.trim());
    }
    tauri_build::build()
}
