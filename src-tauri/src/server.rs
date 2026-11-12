//! Lokaler HTTP/WebSocket-Server für OBS-Browserquellen und -Docks.
//! Läuft ausschließlich auf 127.0.0.1.
//!
//!   /                     Übersicht
//!   /overlay/chat         Chat-Overlay (transparent)
//!   /overlay/alerts       Alert-Overlay (transparent)
//!   /dock                 Multichat-Dock für OBS (Benutzerdefinierte Browser-Docks)
//!   /custom/<name>/       Eigene Templates aus dem Overlay-Ordner
//!   /sdk.js               JavaScript-SDK für eigene Overlays
//!   /ws                   WebSocket mit allen Events
//!   /api/history|stats|status|custom   JSON
//!   POST /api/chat        Chatnachricht senden (nur vom Dock / eigenen Overlays)

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::json;

use crate::state::Shared;

const SDK_JS: &str = include_str!("../overlays/sdk.js");
const CHAT_HTML: &str = include_str!("../overlays/chat.html");
const ALERTS_HTML: &str = include_str!("../overlays/alerts.html");
const DOCK_HTML: &str = include_str!("../overlays/dock.html");
const INDEX_HTML: &str = include_str!("../overlays/index.html");

/// Beispiel-Templates, die beim ersten Start in den Overlay-Ordner kopiert werden.
const TEMPLATES: &[(&str, &str)] = &[
    ("beispiel-chat/index.html", include_str!("../overlays/templates/beispiel-chat/index.html")),
    ("beispiel-chat/style.css", include_str!("../overlays/templates/beispiel-chat/style.css")),
    ("beispiel-chat/script.js", include_str!("../overlays/templates/beispiel-chat/script.js")),
    ("beispiel-alerts/index.html", include_str!("../overlays/templates/beispiel-alerts/index.html")),
    ("beispiel-alerts/style.css", include_str!("../overlays/templates/beispiel-alerts/style.css")),
    ("beispiel-alerts/script.js", include_str!("../overlays/templates/beispiel-alerts/script.js")),
    ("README.md", include_str!("../overlays/templates/README.md")),
];

pub fn install_templates(dir: &Path) {
    for (rel, content) in TEMPLATES {
        let path = dir.join(rel);
        if path.exists() {
            continue;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, content);
    }
}

pub async fn start(st: Shared) {
    let preferred = st.store.get().server_port;
    let router = Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/overlay/chat", get(|| async { Html(CHAT_HTML) }))
        .route("/overlay/alerts", get(|| async { Html(ALERTS_HTML) }))
        .route("/dock", get(|| async { Html(DOCK_HTML) }))
        .route("/sdk.js", get(sdk))
        .route("/ws", get(ws_handler))
        .route("/api/history", get(history))
        .route("/api/stats", get(stats))
        .route("/api/status", get(status))
        .route("/api/custom", get(custom_list))
        .route("/api/chat", post(send_chat))
        .route("/custom/{name}", get(custom_redirect))
        .route("/custom/{name}/", get(custom_index))
        .route("/custom/{name}/{*path}", get(custom_file))
        .with_state(st.clone());

    let mut listener = None;
    for port in preferred..preferred.saturating_add(20) {
        if let Ok(l) = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await {
            listener = Some((l, port));
            break;
        }
    }
    let Some((listener, port)) = listener else {
        log::error!("Overlay-Server konnte keinen freien Port finden");
        return;
    };
    *st.server_port.lock() = Some(port);
    st.hub.emit_raw("ss://server", json!({ "port": port }));
    if let Err(e) = axum::serve(listener, router).await {
        log::error!("Overlay-Server beendet: {e}");
    }
}

async fn sdk() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript; charset=utf-8")], SDK_JS)
}

async fn history(State(st): State<Shared>) -> impl IntoResponse {
    axum::Json(st.hub.history())
}

async fn stats(State(st): State<Shared>) -> impl IntoResponse {
    axum::Json(st.hub.stats_json())
}

async fn status(State(st): State<Shared>) -> impl IntoResponse {
    axum::Json(st.hub.statuses())
}

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(st): State<Shared>) -> Response {
    ws.on_upgrade(move |socket| ws_session(socket, st))
}

async fn ws_session(mut socket: WebSocket, st: Shared) {
    let mut rx = st.hub.subscribe();
    let hello = json!({
        "op": "hello",
        "data": { "history": st.hub.history(), "stats": st.hub.stats_json(), "status": st.hub.statuses() }
    });
    if socket.send(Message::Text(hello.to_string().into())).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(ev) => {
                    let msg = json!({ "op": "event", "data": ev }).to_string();
                    if socket.send(Message::Text(msg.into())).await.is_err() { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(t))) if t.as_str().contains("\"ping\"") => {
                    let _ = socket.send(Message::Text(json!({"op":"pong"}).to_string().into())).await;
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Chat senden
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ChatBody {
    message: String,
    #[serde(default = "all")]
    platform: String,
}
fn all() -> String {
    "all".into()
}

/// Erlaubt nur Anfragen ohne Origin (OBS) oder von einer lokalen Seite – schützt vor CSRF aus fremden Webseiten.
fn local_origin(headers: &HeaderMap) -> bool {
    match headers.get(header::ORIGIN).and_then(|o| o.to_str().ok()) {
        None => true,
        Some(o) => {
            let host = url::Url::parse(o).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
            host == "127.0.0.1" || host == "localhost"
        }
    }
}

async fn send_chat(State(st): State<Shared>, headers: HeaderMap, body: axum::Json<ChatBody>) -> Response {
    if !local_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let results = crate::send_chat_all(&st, &body.platform, &body.message).await;
    axum::Json(results).into_response()
}

// ---------------------------------------------------------------------------
// Eigene Templates
// ---------------------------------------------------------------------------

async fn custom_list(State(st): State<Shared>) -> impl IntoResponse {
    axum::Json(crate::list_custom(&st))
}

async fn custom_redirect(AxPath(name): AxPath<String>) -> Response {
    let mut r = StatusCode::PERMANENT_REDIRECT.into_response();
    if let Ok(v) = HeaderValue::from_str(&format!("/custom/{name}/")) {
        r.headers_mut().insert(header::LOCATION, v);
    }
    r
}

async fn custom_index(State(st): State<Shared>, AxPath(name): AxPath<String>) -> Response {
    serve_custom(&st, &name, "index.html").await
}

async fn custom_file(State(st): State<Shared>, AxPath((name, path)): AxPath<(String, String)>) -> Response {
    let path = if path.is_empty() || path.ends_with('/') { format!("{path}index.html") } else { path };
    serve_custom(&st, &name, &path).await
}

fn safe_join(base: &Path, name: &str, rel: &str) -> Option<PathBuf> {
    let base = base.canonicalize().ok()?;
    let candidate = base.join(name).join(rel).canonicalize().ok()?;
    candidate.starts_with(&base).then_some(candidate)
}

async fn serve_custom(st: &Shared, name: &str, rel: &str) -> Response {
    let Some(path) = safe_join(&st.overlays_dir, name, rel) else {
        return (StatusCode::NOT_FOUND, "Nicht gefunden").into_response();
    };
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [
                    (header::CONTENT_TYPE, mime.to_string()),
                    (header::CACHE_CONTROL, "no-cache".to_string()),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "Nicht gefunden").into_response(),
    }
}
