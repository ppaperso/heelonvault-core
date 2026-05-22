#![allow(clippy::too_many_arguments)]

mod bootstrap_flow;
mod core;
mod feedback;
mod lock_state;
mod login_flow;
mod restore_flow;
mod types;
mod window_state;

use self::types::LoginAttemptOutcome;
pub use self::types::{AuthenticatedSession, BootstrapServicesContext};

pub struct LoginDialog {
    window: gtk4::Window,
}
