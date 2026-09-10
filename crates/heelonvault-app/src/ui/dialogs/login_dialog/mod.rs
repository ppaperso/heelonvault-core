#![allow(clippy::too_many_arguments)]
#![allow(dead_code)]

mod bootstrap_flow;
mod core;
mod events;
mod feedback;
mod lock_state;
mod login_flow;
mod restore_flow;
mod types;
mod views;
mod window_state;

use self::types::LoginAttemptOutcome;
pub use self::types::{AuthenticatedSession, BootstrapServicesContext};

/// Structure principale de la boîte de dialogue de connexion.
pub struct LoginDialog {
    window: gtk4::Window,
    // Ajouter ici les champs nécessaires pour l'état partagé
}
