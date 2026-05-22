use std::fs;
use std::path::PathBuf;

use gtk4::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UiWindowState {
    login_size: Option<(i32, i32)>,
    bootstrap_size: Option<(i32, i32)>,
    restore_size: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy)]
enum WindowStateKey {
    Login,
    Bootstrap,
    Restore,
}

const LOGIN_MIN_WIDTH: i32 = 440;
const LOGIN_MIN_HEIGHT: i32 = 580;
const BOOTSTRAP_MIN_WIDTH: i32 = 520;
const BOOTSTRAP_MIN_HEIGHT: i32 = 680;
const RESTORE_MIN_WIDTH: i32 = 480;
const RESTORE_MIN_HEIGHT: i32 = 500;

fn ui_window_state_path() -> PathBuf {
    if let Some(config_dir) = dirs::config_dir() {
        return config_dir.join("heelonvault").join("ui_window_state.json");
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("heelonvault")
            .join("ui_window_state.json");
    }

    PathBuf::from("ui_window_state.json")
}

fn load_ui_window_state() -> UiWindowState {
    let path = ui_window_state_path();
    let Ok(raw) = fs::read_to_string(path) else {
        return UiWindowState::default();
    };

    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_ui_window_size(key: WindowStateKey, width: i32, height: i32) {
    if width < 100 || height < 100 {
        return;
    }

    let mut state = load_ui_window_state();
    match key {
        WindowStateKey::Login => state.login_size = Some((width, height)),
        WindowStateKey::Bootstrap => state.bootstrap_size = Some((width, height)),
        WindowStateKey::Restore => state.restore_size = Some((width, height)),
    }

    let path = ui_window_state_path();
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            warn!(path = %parent.display(), %error, "failed to create UI state directory");
            return;
        }
    }

    let payload = match serde_json::to_string_pretty(&state) {
        Ok(payload) => payload,
        Err(error) => {
            warn!(%error, "failed to serialize UI window state");
            return;
        }
    };

    if let Err(error) = fs::write(&path, payload) {
        warn!(path = %path.display(), %error, "failed to persist UI window state");
    }
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

fn adaptive_size(
    width_ratio: f64,
    height_ratio: f64,
    fallback_width: i32,
    fallback_height: i32,
    min_width: i32,
    min_height: i32,
) -> (i32, i32) {
    if let Some((screen_width, screen_height)) = primary_monitor_size() {
        let target_width = ((screen_width as f64) * width_ratio).round() as i32;
        let target_height = ((screen_height as f64) * height_ratio).round() as i32;
        return clamp_size_to_monitor(target_width, target_height, min_width, min_height);
    }

    (
        fallback_width.max(min_width),
        fallback_height.max(min_height),
    )
}

pub(super) fn resolve_login_window_size(in_bootstrap_mode: bool) -> (i32, i32) {
    let state = load_ui_window_state();
    let saved = if in_bootstrap_mode {
        state.bootstrap_size
    } else {
        state.login_size
    };

    if let Some((width, height)) = saved {
        return if in_bootstrap_mode {
            clamp_size_to_monitor(width, height, BOOTSTRAP_MIN_WIDTH, BOOTSTRAP_MIN_HEIGHT)
        } else {
            clamp_size_to_monitor(width, height, LOGIN_MIN_WIDTH, LOGIN_MIN_HEIGHT)
        };
    }

    if in_bootstrap_mode {
        adaptive_size(
            0.52,
            0.88,
            560,
            760,
            BOOTSTRAP_MIN_WIDTH,
            BOOTSTRAP_MIN_HEIGHT,
        )
    } else {
        adaptive_size(0.42, 0.78, 480, 640, LOGIN_MIN_WIDTH, LOGIN_MIN_HEIGHT)
    }
}

pub(super) fn resolve_restore_window_size() -> (i32, i32) {
    let state = load_ui_window_state();
    if let Some((width, height)) = state.restore_size {
        return clamp_size_to_monitor(width, height, RESTORE_MIN_WIDTH, RESTORE_MIN_HEIGHT);
    }

    adaptive_size(0.50, 0.70, 560, 520, RESTORE_MIN_WIDTH, RESTORE_MIN_HEIGHT)
}

pub(super) fn save_login_or_bootstrap_window_size(
    in_bootstrap_mode: bool,
    width: i32,
    height: i32,
) {
    if in_bootstrap_mode {
        save_ui_window_size(WindowStateKey::Bootstrap, width, height);
    } else {
        save_ui_window_size(WindowStateKey::Login, width, height);
    }
}

pub(super) fn save_restore_window_size(width: i32, height: i32) {
    save_ui_window_size(WindowStateKey::Restore, width, height);
}

pub(super) fn login_min_size() -> (i32, i32) {
    (LOGIN_MIN_WIDTH, LOGIN_MIN_HEIGHT)
}

pub(super) fn restore_min_size() -> (i32, i32) {
    (RESTORE_MIN_WIDTH, RESTORE_MIN_HEIGHT)
}
