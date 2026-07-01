//! Unified frontend+backend debug logging via `tauri-plugin-log`. The plugin has no
//! post-init `set_max_level`, so the "Enable debug logs" Settings toggle is implemented as a
//! runtime `.filter` closure backed by this process-global flag: warn/error always pass,
//! everything else (info/debug/trace) only passes while the flag is on.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

pub static DEBUG_LOGGING: AtomicBool = AtomicBool::new(false);

pub fn set_debug(on: bool) {
    DEBUG_LOGGING.store(on, Ordering::Relaxed);
}

pub fn is_debug() -> bool {
    DEBUG_LOGGING.load(Ordering::Relaxed)
}

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .level(log::LevelFilter::Trace)
        .level_for("hyper", log::LevelFilter::Warn)
        .level_for("reqwest", log::LevelFilter::Info)
        .max_file_size(5_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(3))
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some("lord-of-the-files".into()),
            }),
            Target::new(TargetKind::Webview),
        ])
        .filter(|meta| meta.level() <= log::Level::Warn || is_debug())
        .build()
}
