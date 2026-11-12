//! Eingebettetes TikTok-Fenster:
//! - Login: Der Nutzer meldet sich ganz normal auf tiktok.com an (Fenster in der App).
//!   Der Benutzername wird automatisch erkannt.
//! - Chat senden: Ein verstecktes Fenster lädt die eigene LIVE-Seite und schreibt Nachrichten
//!   direkt in das Chatfeld – kostenlos und komplett lokal mit dem eigenen Konto.
//!
//! Sicherheit: tiktok.com erhält KEINEN Zugriff auf die Tauri-IPC. Die App steuert die Seite nur
//! per `eval`, Rückmeldungen kommen über den Dokumenttitel (`SSUSER:` / `SSCHAT:`).

use std::time::Duration;

use anyhow::{anyhow, bail};
use serde_json::{Value, json};
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::hub::Platform;
use crate::state::{Shared, s};

pub const LOGIN_LABEL: &str = "tiktok-login";
pub const CHAT_LABEL: &str = "tiktok-chat";

/// Läuft auf jeder TikTok-Seite: meldet den eingeloggten Benutzernamen und stellt `__ssSend` bereit.
const PAGE_SCRIPT: &str = r#"
(function () {
  if (!/(^|\.)tiktok\.com$/.test(location.hostname)) return;

  // Eingeloggten Nutzer melden
  var tries = 0;
  var timer = setInterval(function () {
    tries++;
    try {
      var el = document.getElementById("__UNIVERSAL_DATA_FOR_REHYDRATION__");
      if (el) {
        var d = JSON.parse(el.textContent);
        var u = d && d.__DEFAULT_SCOPE__ && d.__DEFAULT_SCOPE__["webapp.app-context"] && d.__DEFAULT_SCOPE__["webapp.app-context"].user;
        if (u && u.uniqueId) { document.title = "SSUSER:" + u.uniqueId; clearInterval(timer); return; }
      }
    } catch (e) {}
    if (tries > 60) clearInterval(timer);
  }, 1000);

  // Im versteckten Chat-Fenster: Video stumm schalten & pausieren (spart Ressourcen, kein Ton)
  if (window.__SS_SILENT) {
    setInterval(function () {
      document.querySelectorAll("video, audio").forEach(function (v) { v.muted = true; v.volume = 0; try { v.pause(); } catch (e) {} });
    }, 1500);
  }

  function report(id, ok, msg) { document.title = "SSCHAT:" + id + ":" + (ok ? "ok" : "err") + ":" + (msg || ""); }
  function sleep(ms) { return new Promise(function (r) { setTimeout(r, ms); }); }
  function findInput() {
    var sels = [
      '[data-e2e="comment-input"] [contenteditable]',
      '[data-e2e="comment-input"]',
      'div[contenteditable="plaintext-only"]',
      'div[contenteditable="true"][placeholder]',
      'div[contenteditable="true"]',
      'textarea[placeholder]'
    ];
    for (var i = 0; i < sels.length; i++) {
      var el = document.querySelector(sels[i]);
      if (el && (el.isContentEditable || el.tagName === "TEXTAREA")) return el;
    }
    return null;
  }
  function readText(el) { return ((el.tagName === "TEXTAREA" ? el.value : el.textContent) || "").trim(); }

  window.__ssSend = async function (id, text) {
    try {
      var el = null;
      for (var i = 0; i < 20 && !el; i++) { el = findInput(); if (!el) await sleep(500); }
      if (!el) return report(id, false, "Chatfeld nicht gefunden – bist du live und eingeloggt?");
      el.focus();
      if (el.tagName === "TEXTAREA") {
        var setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value").set;
        setter.call(el, text);
        el.dispatchEvent(new Event("input", { bubbles: true }));
      } else {
        document.execCommand("selectAll", false);
        document.execCommand("insertText", false, text);
        if (readText(el) !== text.trim()) { el.textContent = text; el.dispatchEvent(new InputEvent("input", { bubbles: true, data: text })); }
      }
      await sleep(200);
      var btn = document.querySelector('[data-e2e="comment-post"], [data-e2e="chat-send"], button[aria-label*="end" i], div[aria-label*="end" i][role="button"]');
      if (btn && !btn.disabled) btn.click();
      else {
        var o = { key: "Enter", code: "Enter", keyCode: 13, which: 13, bubbles: true, cancelable: true };
        el.dispatchEvent(new KeyboardEvent("keydown", o));
        el.dispatchEvent(new KeyboardEvent("keypress", o));
        el.dispatchEvent(new KeyboardEvent("keyup", o));
      }
      await sleep(600);
      if (readText(el) === "") report(id, true, "");
      else report(id, false, "TikTok hat die Nachricht nicht angenommen");
    } catch (e) { report(id, false, String(e)); }
  };
})();
"#;

fn tiktok_url() -> url::Url {
    "https://www.tiktok.com".parse().expect("gültige URL")
}

/// Titeländerungen der TikTok-Fenster auswerten.
fn on_title(st: &Shared, title: &str) {
    if let Some(user) = title.strip_prefix("SSUSER:") {
        let user = user.trim().to_string();
        if !user.is_empty() && st.store.get().tiktok.username != user && st.store.get().tiktok.session_id.is_some() {
            st.store.update(|s| s.tiktok.username = user);
        }
        *st.tiktok_detected_user.lock() = Some(title["SSUSER:".len()..].trim().to_string());
    } else if let Some(rest) = title.strip_prefix("SSCHAT:") {
        let mut parts = rest.splitn(3, ':');
        let (id, status, msg) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        if let Some(tx) = st.tiktok_pending.lock().remove(id) {
            let _ = tx.send(if status == "ok" { Ok(()) } else { Err(msg.to_string()) });
        }
    }
}

fn build_window(st: &Shared, label: &str, url: url::Url, visible: bool, silent: bool) -> anyhow::Result<WebviewWindow> {
    let st2 = st.clone();
    let script = format!("window.__SS_SILENT = {silent};\n{PAGE_SCRIPT}");
    let win = WebviewWindowBuilder::new(&st.app, label, WebviewUrl::External(url))
        .title(if visible { "Bei TikTok anmelden – StreamSuite" } else { "TikTok LIVE – StreamSuite" })
        .inner_size(480.0, 780.0)
        .visible(visible)
        .initialization_script(script)
        .on_document_title_changed(move |_w, title| on_title(&st2, &title))
        .build()?;
    Ok(win)
}

fn cookie_header(win: &WebviewWindow) -> (String, Option<String>) {
    let cookies = win.cookies_for_url(tiktok_url()).unwrap_or_default();
    let session = cookies.iter().find(|c| c.name() == "sessionid" && !c.value().is_empty()).map(|c| c.value().to_string());
    let header = cookies.iter().map(|c| format!("{}={}", c.name(), c.value())).collect::<Vec<_>>().join("; ");
    (header, session)
}

/// Benutzername über die TikTok-Web-API ermitteln (mit den Cookies des Fensters).
async fn fetch_username(st: &Shared, cookie: &str) -> Option<String> {
    let res = st
        .http
        .get("https://www.tiktok.com/passport/web/account/info/?aid=1459&app_language=de-DE")
        .header("Cookie", cookie)
        .header("Referer", "https://www.tiktok.com/")
        .send()
        .await
        .ok()?;
    let v: Value = res.json().await.ok()?;
    let name = s(&v, "/data/username");
    if name.is_empty() { None } else { Some(name) }
}

fn auth_result(st: &Shared, ok: bool, msg: &str) {
    st.hub.emit_raw("ss://auth", json!({ "platform": "tiktok", "ok": ok, "message": msg }));
}

/// Öffnet das TikTok-Login-Fenster und wartet im Hintergrund auf die erfolgreiche Anmeldung.
pub fn open_login(st: &Shared) -> anyhow::Result<()> {
    if let Some(w) = st.app.get_webview_window(LOGIN_LABEL) {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    *st.tiktok_detected_user.lock() = None;
    let win = build_window(st, LOGIN_LABEL, "https://www.tiktok.com/login".parse()?, true, false)?;
    let st2 = st.clone();
    tauri::async_runtime::spawn(async move {
        let mut announced = false;
        for _ in 0..600 {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            // Fenster vom Nutzer geschlossen?
            if st2.app.get_webview_window(LOGIN_LABEL).is_none() {
                return;
            }
            let (cookie, session) = cookie_header(&win);
            let Some(session) = session else { continue };
            if !announced {
                announced = true;
                // Nach dem Login zur Startseite, damit die Seite den Nutzer verrät
                let _ = win.eval("if (location.pathname.indexOf('/login') === 0) location.href = 'https://www.tiktok.com/';");
            }
            let mut user = fetch_username(&st2, &cookie).await;
            if user.is_none() {
                user = st2.tiktok_detected_user.lock().clone();
            }
            let Some(user) = user else { continue };
            st2.store.update(|s| {
                s.tiktok.username = user.clone();
                s.tiktok.session_id = Some(session.clone());
                s.tiktok.enabled = true;
            });
            let _ = win.close();
            auth_result(&st2, true, &format!("Bei TikTok angemeldet als @{user}"));
            crate::connections::start(&st2, Platform::Tiktok);
            return;
        }
        auth_result(&st2, false, "TikTok-Anmeldung abgebrochen (Zeitüberschreitung)");
    });
    Ok(())
}

/// Verstecktes Fenster mit der eigenen LIVE-Seite (für das Senden von Chatnachrichten).
pub fn ensure_chat_window(st: &Shared) -> anyhow::Result<WebviewWindow> {
    if let Some(w) = st.app.get_webview_window(CHAT_LABEL) {
        return Ok(w);
    }
    let cfg = st.store.get().tiktok;
    if cfg.session_id.is_none() {
        bail!("Für das Senden im TikTok-Chat bitte bei TikTok anmelden");
    }
    let user = cfg.username.trim().trim_start_matches('@').to_string();
    if user.is_empty() {
        bail!("TikTok-Benutzername unbekannt");
    }
    let url = url::Url::parse(&format!("https://www.tiktok.com/@{user}/live"))?;
    build_window(st, CHAT_LABEL, url, false, true)
}

/// Zeigt das TikTok-LIVE-Fenster (z. B. um ein Captcha zu lösen).
pub fn show_chat_window(st: &Shared) -> anyhow::Result<()> {
    let w = ensure_chat_window(st)?;
    w.show()?;
    w.set_focus()?;
    Ok(())
}

pub fn close_windows(st: &Shared) {
    for label in [LOGIN_LABEL, CHAT_LABEL] {
        if let Some(w) = st.app.get_webview_window(label) {
            let _ = w.close();
        }
    }
}

/// Lädt die LIVE-Seite neu (z. B. wenn der Stream neu gestartet wurde).
pub fn reload_chat_window(st: &Shared) {
    if let Some(w) = st.app.get_webview_window(CHAT_LABEL) {
        let _ = w.eval("location.reload()");
    }
}

pub async fn send_chat(st: &Shared, text: &str) -> anyhow::Result<()> {
    let win = ensure_chat_window(st)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();
    st.tiktok_pending.lock().insert(id.clone(), tx);
    let js = format!(
        "(function(){{var n=0;function go(){{if(window.__ssSend){{window.__ssSend({id},{text});}}else if(n++<40){{setTimeout(go,500);}}else{{document.title='SSCHAT:'+{id}+':err:Seite nicht geladen';}}}}go();}})();",
        id = serde_json::to_string(&id)?,
        text = serde_json::to_string(text)?,
    );
    win.eval(js)?;
    let res = tokio::time::timeout(Duration::from_secs(30), rx).await;
    st.tiktok_pending.lock().remove(&id);
    match res {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(msg))) => Err(anyhow!(msg)),
        _ => bail!("Keine Antwort vom TikTok-Fenster (Zeitüberschreitung)"),
    }
}

/// Abmelden: TikTok-Cookies löschen und Fenster schließen.
pub fn logout(st: &Shared) {
    // Ein (unsichtbares) Fenster wird zum Löschen der Cookies gebraucht
    let win = st
        .app
        .get_webview_window(CHAT_LABEL)
        .or_else(|| st.app.get_webview_window(LOGIN_LABEL))
        .or_else(|| st.app.get_webview_window("main"));
    if let Some(w) = win {
        if let Ok(cookies) = w.cookies() {
            for c in cookies {
                if c.domain().map(|d| d.contains("tiktok")).unwrap_or(false) {
                    let _ = w.delete_cookie(c);
                }
            }
        }
    }
    close_windows(st);
}
