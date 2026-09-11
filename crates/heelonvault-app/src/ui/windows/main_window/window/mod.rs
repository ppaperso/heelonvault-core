//! Main application window construction and management.
//!
//! Responsibilities are split so that `core.rs` stays an orchestrator:
//! - `types.rs`: data structures shared by the window modules
//! - `views.rs`: pure UI construction (no event handlers)
//! - `events.rs`: event handlers and callbacks
//! - `editor.rs`: the inline secret editor page
//! - `refresh.rs`: secret list reload
//! - `vault_list.rs`: vault sidebar population, selection, creation, deletion
//! - `navigation.rs`: profile / users / teams stack pages
//! - `pin_badge.rs`: header PIN badge state and countdown
//! - `i18n_refresh.rs`: live re-translation of the whole window
//! - `core.rs`: orchestration — wires all of the above together

pub mod core;
pub mod editor;
pub mod events;
pub mod i18n_refresh;
pub mod navigation;
pub mod pin_badge;
pub mod refresh;
pub mod types;
pub mod vault_list;
pub mod views;
