// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK stürzt auf manchen Wayland-Setups (v. a. NVIDIA) mit
    // "Error 71 (Protocol error)" ab. Der DMABUF-Renderer ist die Ursache – deaktivieren,
    // sofern der Nutzer nichts anderes gesetzt hat.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: wird vor dem Start jeglicher Threads aufgerufen.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    streamsuite_lib::run()
}
