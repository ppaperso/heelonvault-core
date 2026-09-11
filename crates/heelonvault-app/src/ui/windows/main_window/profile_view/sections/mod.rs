//! Profile page sections.
//!
//! Each section builds its own widgets, owns the handlers that only touch them, and knows
//! how to re-apply its translations.

pub mod admin;
pub mod data;
pub mod identity;
pub mod password;
pub mod security_prefs;
pub mod twofa;
