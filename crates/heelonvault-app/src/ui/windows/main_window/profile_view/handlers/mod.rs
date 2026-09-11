//! Handler wiring for the profile page sections.
//!
//! Handlers live next to the section they drive, with their dependencies passed as an
//! explicit struct rather than captured from a surrounding 2000-line function body.

pub mod data;
pub mod twofa;
