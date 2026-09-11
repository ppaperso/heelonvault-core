//! Window module - Main application window construction and management
//!
//! This module provides the modular architecture for the main window,
//! separating concerns into:
//! - `types.rs`: Data structures (MainWindowWidgets, MainWindowState)
//! - `views.rs`: UI construction only (no event handlers)
//! - `events.rs`: Event handlers and callbacks
//! - `core.rs`: Orchestration - combines views and events

pub mod core;
pub mod events;
pub mod types;
pub mod views;
