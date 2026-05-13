use std::fs;
use std::path::PathBuf;

use gtk4::prelude::{Cast, DisplayExt, ListModelExt, MonitorExt};
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone, Copy)]
pub struct MainWindowLaunch {
    pub width: i32,
    pub height: i32,
    pub fullscreen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MainWindowState {
    width: i32,
    height: i32,
    fullscreen: bool,
}

fn state_file_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir
            .join("heelonvault")
            .join("ui_main_window_state.json");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("heelonvault")
            .join("ui_main_window_state.json");
    }

    PathBuf::from("ui_main_window_state.json")
}

fn primary_monitor_size() -> Option<(i32, i32)> {
    let display = gtk4::gdk::Display::default()?;
    let monitors = display.monitors();
    let monitor_obj = monitors.item(0)?;
    let monitor = monitor_obj.downcast::<gtk4::gdk::Monitor>().ok()?;
    let geometry = monitor.geometry();
    Some((geometry.width(), geometry.height()))
}

fn clamp_size_to_monitor(width: i32, height: i32, min_width: i32, min_height: i32) -> (i32, i32) {
    if let Some((screen_width, screen_height)) = primary_monitor_size() {
        let max_width = (((screen_width as f64) * 0.95).round() as i32).max(min_width);
        let max_height = (((screen_height as f64) * 0.95).round() as i32).max(min_height);
        return (
            width.clamp(min_width, max_width),
            height.clamp(min_height, max_height),
        );
    }

    (width.max(min_width), height.max(min_height))
}

fn adaptive_main_size(
    fallback_width: i32,
    fallback_height: i32,
    min_width: i32,
    min_height: i32,
) -> (i32, i32) {
    if let Some((screen_width, screen_height)) = primary_monitor_size() {
        let target_width = ((screen_width as f64) * 0.70).round() as i32;
        let target_height = ((screen_height as f64) * 0.70).round() as i32;
        return clamp_size_to_monitor(target_width, target_height, min_width, min_height);
    }

    (
        fallback_width.max(min_width),
        fallback_height.max(min_height),
    )
}

fn load_main_state() -> Option<MainWindowState> {
    let path = state_file_path();
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn resolve_main_window_launch(
    fallback_width: i32,
    fallback_height: i32,
    min_width: i32,
    min_height: i32,
) -> MainWindowLaunch {
    if let Some(saved) = load_main_state() {
        let (width, height) =
            clamp_size_to_monitor(saved.width, saved.height, min_width, min_height);
        return MainWindowLaunch {
            width,
            height,
            fullscreen: saved.fullscreen,
        };
    }

    let (width, height) =
        adaptive_main_size(fallback_width, fallback_height, min_width, min_height);
    MainWindowLaunch {
        width,
        height,
        fullscreen: true,
    }
}

pub fn persist_main_window_state(width: i32, height: i32, fullscreen: bool) {
    if width < 100 || height < 100 {
        return;
    }

    let state = MainWindowState {
        width,
        height,
        fullscreen,
    };

    let path = state_file_path();
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            warn!(path = %parent.display(), %error, "failed to create UI state directory");
            return;
        }
    }

    let payload = match serde_json::to_string_pretty(&state) {
        Ok(payload) => payload,
        Err(error) => {
            warn!(%error, "failed to serialize main window state");
            return;
        }
    };

    if let Err(error) = fs::write(&path, payload) {
        warn!(path = %path.display(), %error, "failed to persist main window state");
    }
}
