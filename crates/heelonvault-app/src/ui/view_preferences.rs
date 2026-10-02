//! Display preferences of the main window that survive restarts (secret list layout).
//!
//! Stored next to the window size state, per installation. A missing or unreadable file
//! silently falls back to the defaults: these are conveniences, never security settings.

use std::fs;

use serde::{Deserialize, Serialize};
use tracing::warn;

use super::window_sizing::ui_state_file_path;

const FILE_NAME: &str = "ui_view_preferences.json";

/// How secrets are laid out in the main list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretViewMode {
    /// Multi-column grid of cards (historical layout).
    #[default]
    Grid,
    /// One compact row per secret, columns aligned.
    List,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ViewPreferences {
    #[serde(default)]
    secret_view_mode: SecretViewMode,
}

fn load() -> ViewPreferences {
    let Ok(raw) = fs::read_to_string(ui_state_file_path(FILE_NAME)) else {
        return ViewPreferences::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn load_secret_view_mode() -> SecretViewMode {
    load().secret_view_mode
}

pub fn persist_secret_view_mode(mode: SecretViewMode) {
    let mut preferences = load();
    preferences.secret_view_mode = mode;

    let path = ui_state_file_path(FILE_NAME);
    if let Some(parent) = path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        warn!(path = %parent.display(), %error, "failed to create UI state directory");
        return;
    }

    let payload = match serde_json::to_string_pretty(&preferences) {
        Ok(payload) => payload,
        Err(error) => {
            warn!(%error, "failed to serialize view preferences");
            return;
        }
    };

    if let Err(error) = fs::write(&path, payload) {
        warn!(path = %path.display(), %error, "failed to persist view preferences");
    }
}
