//! Tray-Icon (Windows/Linux Taskleiste, macOS Menüleiste) für den Hintergrundbetrieb.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

static AVAILABLE: AtomicBool = AtomicBool::new(false);

pub fn available() -> bool {
    AVAILABLE.load(Ordering::Relaxed)
}

/// Unter Linux benötigt das Tray-Icon libayatana-appindicator. Fehlt sie, würde tray-icon abstürzen –
/// daher vorher prüfen.
fn supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        for name in [
            "libayatana-appindicator3.so.1",
            "libayatana-appindicator3.so",
            "libappindicator3.so.1",
            "libappindicator3.so",
        ] {
            // SAFETY: Die Bibliothek wird nur geladen und sofort wieder freigegeben.
            if unsafe { libloading::Library::new(name) }.is_ok() {
                return true;
            }
        }
        false
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    if !supported() {
        log::warn!("Kein AppIndicator gefunden – Tray-Icon deaktiviert");
        return Ok(());
    }
    let open = MenuItem::with_id(app, "open", "StreamSuite öffnen", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("StreamSuite")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    AVAILABLE.store(true, Ordering::Relaxed);
    Ok(())
}
