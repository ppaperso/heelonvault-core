use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use secrecy::SecretBox;
use tokio::runtime::Handle;
use tracing::{info, warn};

use super::bootstrap_flow::setup_bootstrap_submit_handler;
use super::events;
use super::feedback;
use super::lock_state;
use super::login_flow;
use super::restore_flow;
use super::types::{AuthenticatedSession, BootstrapServicesContext, LoginAttemptOutcome};
use super::views::{
    LoginDialogWidgets, build_login_view, setup_bootstrap_gates, setup_language_toggle,
};
use super::window_state;
use heelonvault_core::errors::AppError;
use heelonvault_core::services::admin_service::BootstrapResult;
use heelonvault_core::services::auth_policy_service::AuthPolicyService;
use heelonvault_core::services::auth_service::AuthService;
use heelonvault_core::services::totp_service::TotpService;
use heelonvault_core::services::user_service::UserService;

/// Contexte de login pour stocker l'état partagé
struct LoginContext<TAuth, TPolicy, TUser, TTotp>
where
    TAuth: AuthService + Send + Sync + 'static,
    TPolicy: AuthPolicyService + Send + Sync + 'static,
    TUser: UserService + Send + Sync + 'static,
    TTotp: TotpService + Send + Sync + 'static,
{
    window: gtk4::Window,
    widgets: LoginDialogWidgets,
    runtime: Handle,
    auth_service: Arc<TAuth>,
    auth_policy_service: Arc<TPolicy>,
    user_service: Arc<TUser>,
    totp_service: Arc<TTotp>,
    startup_psc_artifact: Option<String>,
    bootstrap_ctx: Option<BootstrapServicesContext>,
    authenticated: Rc<Cell<bool>>,
    lock_active: Rc<Cell<bool>>,
    lock_timer: Rc<RefCell<Option<glib::SourceId>>>,
    on_restore_requested: restore_flow::RestoreHandler,
    on_restore_completed: Rc<dyn Fn()>,
    on_authenticated: Rc<dyn Fn(AuthenticatedSession)>,
    on_cancelled: Rc<dyn Fn()>,
}

impl super::LoginDialog {
    /// Crée et initialise une nouvelle instance de la boîte de dialogue de connexion.
    pub fn new<TAuth, TPolicy, TUser, TTotp, TFederated>(
        application: &adw::Application,
        parent: &adw::ApplicationWindow,
        runtime_handle: Handle,
        auth_service: Arc<TAuth>,
        auth_policy_service: Arc<TPolicy>,
        user_service: Arc<TUser>,
        totp_service: Arc<TTotp>,
        _federated_auth_service: Arc<TFederated>,
        startup_psc_artifact: Option<String>,
        bootstrap_ctx: Option<BootstrapServicesContext>,
        license_badge_text: String,
        on_restore_requested: impl Fn(
            PathBuf,
            String,
            String,
        ) -> Result<
            heelonvault_core::services::rekey_service::RekeyReport,
            AppError,
        > + Send
        + Sync
        + 'static,
        on_restore_completed: impl Fn() + 'static,
        on_authenticated: impl Fn(AuthenticatedSession) + 'static,
        on_cancelled: impl Fn() + 'static,
    ) -> Self
    where
        TAuth: AuthService + Send + Sync + 'static,
        TPolicy: AuthPolicyService + Send + Sync + 'static,
        TUser: UserService + Send + Sync + 'static,
        TTotp: TotpService + Send + Sync + 'static,
        TFederated: Send + Sync + 'static,
    {
        // 1. Création de la fenêtre
        let in_bootstrap_mode = bootstrap_ctx.is_some();
        let (initial_width, initial_height) =
            window_state::resolve_login_window_size(in_bootstrap_mode);

        let window = gtk4::Window::builder()
            .application(application)
            .transient_for(parent)
            .title(heelonvault_core::tr!("login-window-title").as_str())
            .modal(true)
            .resizable(true)
            .default_width(initial_width)
            .default_height(initial_height)
            .build();

        let (login_min_width, login_min_height) = window_state::login_min_size();
        window.set_size_request(login_min_width, login_min_height);

        // 2. Construction de l'interface graphique
        let widgets = build_login_view(license_badge_text, in_bootstrap_mode);

        // 2b. Configuration du changement de langue (FR/EN)
        setup_language_toggle(&widgets, in_bootstrap_mode);

        // 2c. Convertir on_authenticated en Rc avant de l'utiliser
        let on_authenticated_rc: Rc<dyn Fn(AuthenticatedSession)> = Rc::new(on_authenticated);

        // 2d. Configuration des gates bootstrap si mode initialisation
        if in_bootstrap_mode {
            setup_bootstrap_gates(&widgets);

            // Extraire les callbacks depuis bootstrap_ctx
            let gen_key_fn = bootstrap_ctx
                .as_ref()
                .map(|ctx| Arc::clone(&ctx.generate_recovery_key));
            let do_bootstrap_fn = bootstrap_ctx.as_ref().map(|ctx| {
                let fn_arc: &Arc<
                    dyn Fn(String, Vec<u8>) -> Result<BootstrapResult, AppError> + Send + Sync,
                > = &ctx.do_bootstrap;
                Arc::clone(fn_arc)
            });

            // Configuration du handler submit pour le mode bootstrap
            setup_bootstrap_submit_handler(
                &widgets,
                gen_key_fn,
                do_bootstrap_fn,
                &window,
                Rc::new(Cell::new(false)), // authenticated - sera géré par le handler
                Rc::clone(&on_authenticated_rc),
            );
        }

        // 3. Initialisation de l'état partagé
        let authenticated = Rc::new(Cell::new(false));
        let lock_active = Rc::new(Cell::new(false));
        let lock_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

        // Convertir les impl Fn en dyn Fn pour le stockage
        let on_restore_requested_arc: restore_flow::RestoreHandler = Arc::new(on_restore_requested);
        let on_restore_completed_rc: Rc<dyn Fn()> = Rc::new(on_restore_completed);
        let on_cancelled_rc: Rc<dyn Fn()> = Rc::new(on_cancelled);

        // 4. Connexion des événements de base (Entrée -> clic sur submit)
        let submit_button_for_events = widgets.submit_button.clone();
        events::setup_events(&widgets, move || {
            submit_button_for_events.emit_clicked();
        });

        // 4b. Connexion du bouton Quitter pour fermer la fenêtre
        let back_button_for_close = widgets.back_button.clone();
        let window_for_back = window.clone();
        back_button_for_close.connect_clicked(move |_| {
            window_for_back.close();
        });

        // 4b-bis. Retour depuis l'étape TOTP vers la saisie des identifiants
        {
            let step_stack_for_back = widgets.step_stack.clone();
            let error_label_for_back = widgets.error_label.clone();
            let password_entry_for_back = widgets.password_entry.clone();
            widgets.totp_back_button.connect_clicked(move |_| {
                step_stack_for_back.set_visible_child_name("credentials");
                feedback::clear_feedback(&error_label_for_back);
                password_entry_for_back.grab_focus();
            });
        }

        // 4c. Connexion du bouton Restore pour ouvrir la dialogue de restauration
        // Uniquement en mode NON bootstrap
        if !in_bootstrap_mode {
            let restore_parent = window.clone();
            let restore_request_handler = Arc::clone(&on_restore_requested_arc);
            let restore_complete_handler = Rc::clone(&on_restore_completed_rc);
            let restore_button = widgets.restore_button.clone();
            restore_button.connect_clicked(move |_| {
                restore_flow::present_restore_dialog(
                    &restore_parent,
                    Arc::clone(&restore_request_handler),
                    Rc::clone(&restore_complete_handler),
                );
            });
        }

        // 5. Configuration du handler principal du bouton submit (uniquement si NON en mode bootstrap)
        if !in_bootstrap_mode {
            let context = Rc::new(LoginContext {
                window: window.clone(),
                widgets: widgets.clone(),
                runtime: runtime_handle.clone(),
                auth_service: Arc::clone(&auth_service),
                auth_policy_service: Arc::clone(&auth_policy_service),
                user_service: Arc::clone(&user_service),
                totp_service: Arc::clone(&totp_service),
                startup_psc_artifact: startup_psc_artifact.clone(),
                bootstrap_ctx: bootstrap_ctx.clone(),
                authenticated: Rc::clone(&authenticated),
                lock_active: Rc::clone(&lock_active),
                lock_timer: Rc::clone(&lock_timer),
                on_restore_requested: Arc::clone(&on_restore_requested_arc),
                on_restore_completed: Rc::clone(&on_restore_completed_rc),
                on_authenticated: Rc::clone(&on_authenticated_rc),
                on_cancelled: Rc::clone(&on_cancelled_rc),
            });

            Self::setup_submit_handler(&window, &widgets, Rc::clone(&context));
        }

        // 6. Intégration du conteneur principal dans la fenêtre GTK
        window.set_child(Some(&widgets.container));

        // 7. Configuration du handler close_request pour la fenêtre
        // Ce handler gère la fermeture de la fenêtre (sauvegarde de la taille, nettoyage, etc.)
        if in_bootstrap_mode {
            // En mode bootstrap, on configure un handler simple avec le log marker
            let authenticated_for_close = Rc::clone(&authenticated);
            let on_cancelled_for_close = Rc::clone(&on_cancelled_rc);

            window.connect_close_request(move |win| {
                info!(
                    authenticated = authenticated_for_close.get(),
                    "login window close requested"
                );
                window_state::save_login_or_bootstrap_window_size(
                    true, // in_bootstrap_mode
                    win.width(),
                    win.height(),
                );

                if !authenticated_for_close.get() {
                    warn!("login window closed before authentication, requesting application quit");
                    on_cancelled_for_close();
                } else {
                    info!(
                        "login window closed after authentication, returning to application flow"
                    );
                }
                glib::Propagation::Proceed
            });
        } else {
            // Mode normal (non bootstrap)
            let authenticated_for_close = Rc::clone(&authenticated);
            let on_cancelled_for_close = Rc::clone(&on_cancelled_rc);

            window.connect_close_request(move |win| {
                info!(
                    authenticated = authenticated_for_close.get(),
                    "login window close requested"
                );
                window_state::save_login_or_bootstrap_window_size(
                    false, // in_bootstrap_mode
                    win.width(),
                    win.height(),
                );

                if !authenticated_for_close.get() {
                    warn!("login window closed before authentication, requesting application quit");
                    on_cancelled_for_close();
                } else {
                    info!(
                        "login window closed after authentication, returning to application flow"
                    );
                }
                glib::Propagation::Proceed
            });
        }

        Self {
            window,
            in_bootstrap_mode,
            widgets,
        }
    }

    /// Configure le handler du bouton submit avec toute la logique métier.
    fn setup_submit_handler<TAuth, TPolicy, TUser, TTotp>(
        window: &gtk4::Window,
        widgets: &LoginDialogWidgets,
        context: Rc<LoginContext<TAuth, TPolicy, TUser, TTotp>>,
    ) where
        TAuth: AuthService + Send + Sync + 'static,
        TPolicy: AuthPolicyService + Send + Sync + 'static,
        TUser: UserService + Send + Sync + 'static,
        TTotp: TotpService + Send + Sync + 'static,
    {
        let widgets_clone = widgets.clone();
        let context_for_handler = Rc::clone(&context);
        let window_clone = window.clone();

        widgets.submit_button.connect_clicked(move |_| {
            // L'étape TOTP est une page du `step_stack`, pas un bloc masqué : c'est la
            // page visible du stack qui fait foi. Ne PAS tester `totp_step_box.is_visible()`
            // — un widget GTK4 est visible par défaut, donc la page "totp" est `visible`
            // dès la construction alors que le stack affiche encore "credentials", et le
            // tout premier clic partirait vérifier un code TOTP vide.
            let in_totp_step = widgets_clone
                .step_stack
                .visible_child_name()
                .is_some_and(|name| name == "totp");

            if in_totp_step {
                let ctx = context_for_handler.as_ref();
                login_flow::handle_totp_submit(
                    ctx.runtime.clone(),
                    Arc::clone(&ctx.auth_service),
                    Arc::clone(&ctx.auth_policy_service),
                    Arc::clone(&ctx.user_service),
                    Arc::clone(&ctx.totp_service),
                    &widgets_clone.username_entry,
                    &widgets_clone.password_entry,
                    &widgets_clone.totp_entry,
                    &widgets_clone.submit_button,
                    &widgets_clone.submit_spinner,
                    &window_clone,
                    &widgets_clone.error_label,
                    Rc::clone(&ctx.authenticated),
                    Rc::clone(&ctx.on_authenticated),
                    Rc::clone(&ctx.lock_active),
                    Rc::clone(&ctx.lock_timer),
                    1200,
                );
            } else {
                Self::handle_submit(
                    &window_clone,
                    &widgets_clone,
                    Rc::clone(&context_for_handler),
                );
            }
        });
    }

    /// Gère la soumission du formulaire de connexion.
    fn handle_submit<TAuth, TPolicy, TUser, TTotp>(
        window: &gtk4::Window,
        widgets: &LoginDialogWidgets,
        context: Rc<LoginContext<TAuth, TPolicy, TUser, TTotp>>,
    ) where
        TAuth: AuthService + Send + Sync + 'static,
        TPolicy: AuthPolicyService + Send + Sync + 'static,
        TUser: UserService + Send + Sync + 'static,
        TTotp: TotpService + Send + Sync + 'static,
    {
        // Empêcher les clics multiples
        if context.lock_active.get() {
            return;
        }

        // Effacer les feedbacks précédents
        feedback::clear_feedback(&widgets.error_label);

        // Récupérer les valeurs
        let username = widgets.username_entry.text().trim().to_string();
        let password = widgets.password_entry.text().to_string();

        // Validation basique
        if username.is_empty() {
            feedback::show_feedback(
                &widgets.error_label,
                heelonvault_core::tr!("login-error-username-required").as_str(),
            );
            return;
        }

        if password.is_empty() {
            feedback::show_feedback(
                &widgets.error_label,
                heelonvault_core::tr!("login-error-password-required").as_str(),
            );
            return;
        }

        // Afficher l'état pending
        feedback::set_pending_state(&widgets.submit_button, &widgets.submit_spinner, true);
        context.lock_active.set(true);

        // Cloner les données nécessaires pour le thread
        let auth_service = Arc::clone(&context.auth_service);
        let auth_policy_service = Arc::clone(&context.auth_policy_service);
        let user_service = Arc::clone(&context.user_service);
        let totp_service = Arc::clone(&context.totp_service);
        let username_for_task = username.clone();
        let password_for_task = password.into_bytes();
        let runtime = context.runtime.clone();
        let totp_entry_for_task = widgets.totp_entry.clone();
        let step_stack_for_task = widgets.step_stack.clone();

        // Lancer la tâche d'authentification dans un thread
        let (result_sender, result_receiver) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            let password_bytes = password_for_task;
            let result: Result<LoginAttemptOutcome, AppError> = runtime.block_on(async move {
                // Résoudre le nom d'utilisateur
                let resolved_username = user_service
                    .resolve_username_for_login_identifier(&username_for_task)
                    .await?;

                let canonical_username = match resolved_username {
                    Some(value) => value,
                    None => {
                        return Ok(LoginAttemptOutcome::InvalidCredentials {
                            remaining_lock_secs: 0,
                        });
                    }
                };

                // Vérifier l'état de verrouillage
                let lock_state_result = auth_policy_service
                    .get_state(canonical_username.as_str())
                    .await?;

                if lock_state_result.is_locked() {
                    return Ok(LoginAttemptOutcome::Locked {
                        remaining_lock_secs: lock_state_result.remaining_lock_secs,
                    });
                }

                // Vérifier si le mot de passe est valide
                let password_valid = auth_service
                    .verify_password(
                        canonical_username.as_str(),
                        SecretBox::new(Box::new(password_bytes.clone())),
                    )
                    .await?;

                if !password_valid {
                    let state = auth_policy_service
                        .record_failed_attempt(canonical_username.as_str())
                        .await?;
                    return Ok(LoginAttemptOutcome::InvalidCredentials {
                        remaining_lock_secs: state.remaining_lock_secs,
                    });
                }

                // Vérifier si le TOTP est activé pour cet utilisateur
                let has_totp = totp_service
                    .is_totp_enabled_for_username(canonical_username.as_str())
                    .await?;

                if has_totp {
                    // En mode TOTP, on affiche le champ TOTP et on retourne RequiresTotp
                    return Ok(LoginAttemptOutcome::RequiresTotp);
                }

                // Si pas de TOTP requis, on peut authentifier directement
                let master_key_opt = auth_service
                    .derive_key_if_valid(
                        canonical_username.as_str(),
                        SecretBox::new(Box::new(password_bytes.clone())),
                    )
                    .await?;

                let master_key = match master_key_opt {
                    Some(key) => key,
                    None => {
                        let state = auth_policy_service
                            .record_failed_attempt(canonical_username.as_str())
                            .await?;
                        return Ok(LoginAttemptOutcome::InvalidCredentials {
                            remaining_lock_secs: state.remaining_lock_secs,
                        });
                    }
                };
                let master_key = super::login_flow::upgrade_legacy_credentials(
                    user_service.as_ref(),
                    canonical_username.as_str(),
                    &password_bytes,
                    master_key,
                )
                .await;

                let user_profile = user_service
                    .get_user_profile_by_username(canonical_username.as_str())
                    .await?;

                let identity_label = user_profile
                    .display_name
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| user_profile.username.clone());

                // Réinitialiser les tentatives échouées
                auth_policy_service
                    .reset_failed_attempts(canonical_username.as_str())
                    .await?;

                Ok(LoginAttemptOutcome::Success(AuthenticatedSession {
                    user_id: user_profile.id,
                    username: canonical_username,
                    identity_label,
                    master_key,
                }))
            });

            let _ = result_sender.send(result);
        });

        // Traiter le résultat dans le main thread GTK
        let window_for_result = window.clone();
        let authenticated_for_result = Rc::clone(&context.authenticated);
        let on_authenticated_for_result = Rc::clone(&context.on_authenticated);
        let lock_active_for_result = Rc::clone(&context.lock_active);
        let lock_timer_for_result = Rc::clone(&context.lock_timer);
        let error_label_for_result = widgets.error_label.clone();
        let submit_button_for_result = widgets.submit_button.clone();
        let submit_spinner_for_result = widgets.submit_spinner.clone();

        glib::MainContext::default().spawn_local(async move {
            match result_receiver.await {
                Ok(Ok(LoginAttemptOutcome::Success(session))) => {
                    feedback::set_pending_state(
                        &submit_button_for_result,
                        &submit_spinner_for_result,
                        false,
                    );
                    lock_active_for_result.set(false);
                    authenticated_for_result.set(true);
                    on_authenticated_for_result(session);
                    window_for_result.close();
                }
                Ok(Ok(LoginAttemptOutcome::RequiresTotp)) => {
                    // Basculer le stack sur l'étape TOTP. `totp_step_box.set_visible(true)`
                    // ne suffit pas : la page est déjà `visible`, c'est le stack qui affiche
                    // encore "credentials".
                    step_stack_for_task.set_visible_child_name("totp");
                    totp_entry_for_task.set_text("");
                    totp_entry_for_task.grab_focus();
                    feedback::set_pending_state(
                        &submit_button_for_result,
                        &submit_spinner_for_result,
                        false,
                    );
                    lock_active_for_result.set(false);
                }
                Ok(Ok(LoginAttemptOutcome::InvalidCredentials {
                    remaining_lock_secs,
                })) => {
                    lock_active_for_result.set(false);
                    if remaining_lock_secs > 0 {
                        feedback::set_pending_state(
                            &submit_button_for_result,
                            &submit_spinner_for_result,
                            false,
                        );
                        lock_state::start_lock_countdown(
                            &submit_button_for_result,
                            &submit_spinner_for_result,
                            &error_label_for_result,
                            remaining_lock_secs,
                            Rc::clone(&lock_active_for_result),
                            Rc::clone(&lock_timer_for_result),
                            feedback::set_pending_state,
                            feedback::show_feedback,
                        );
                    } else {
                        feedback::set_pending_state(
                            &submit_button_for_result,
                            &submit_spinner_for_result,
                            false,
                        );
                        feedback::show_feedback(
                            &error_label_for_result,
                            heelonvault_core::tr!("login-error-invalid-credentials").as_str(),
                        );
                    }
                }
                Ok(Ok(LoginAttemptOutcome::InvalidTotp {
                    remaining_lock_secs,
                })) => {
                    lock_active_for_result.set(false);
                    totp_entry_for_task.grab_focus();
                    if remaining_lock_secs > 0 {
                        feedback::set_pending_state(
                            &submit_button_for_result,
                            &submit_spinner_for_result,
                            false,
                        );
                        lock_state::start_lock_countdown(
                            &submit_button_for_result,
                            &submit_spinner_for_result,
                            &error_label_for_result,
                            remaining_lock_secs,
                            Rc::clone(&lock_active_for_result),
                            Rc::clone(&lock_timer_for_result),
                            feedback::set_pending_state,
                            feedback::show_feedback,
                        );
                    } else {
                        feedback::show_feedback(
                            &error_label_for_result,
                            heelonvault_core::tr!("login-error-invalid-credentials").as_str(),
                        );
                        let button_after_delay = submit_button_for_result.clone();
                        let spinner_after_delay = submit_spinner_for_result.clone();
                        glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                            feedback::set_pending_state(
                                &button_after_delay,
                                &spinner_after_delay,
                                false,
                            );
                        });
                    }
                }
                Ok(Ok(LoginAttemptOutcome::Locked {
                    remaining_lock_secs,
                })) => {
                    lock_active_for_result.set(false);
                    feedback::set_pending_state(
                        &submit_button_for_result,
                        &submit_spinner_for_result,
                        false,
                    );
                    lock_state::start_lock_countdown(
                        &submit_button_for_result,
                        &submit_spinner_for_result,
                        &error_label_for_result,
                        remaining_lock_secs,
                        Rc::clone(&lock_active_for_result),
                        Rc::clone(&lock_timer_for_result),
                        feedback::set_pending_state,
                        feedback::show_feedback,
                    );
                }
                Ok(Err(error)) => {
                    lock_active_for_result.set(false);
                    feedback::set_pending_state(
                        &submit_button_for_result,
                        &submit_spinner_for_result,
                        false,
                    );
                    feedback::show_feedback(
                        &error_label_for_result,
                        format!("Erreur: {}", error).as_str(),
                    );
                    let button_after_delay = submit_button_for_result.clone();
                    let spinner_after_delay = submit_spinner_for_result.clone();
                    glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                        feedback::set_pending_state(
                            &button_after_delay,
                            &spinner_after_delay,
                            false,
                        );
                    });
                }
                Err(_) => {
                    lock_active_for_result.set(false);
                    feedback::show_feedback(
                        &error_label_for_result,
                        heelonvault_core::tr!("login-error-interrupted").as_str(),
                    );
                    let button_after_delay = submit_button_for_result.clone();
                    let spinner_after_delay = submit_spinner_for_result.clone();
                    glib::timeout_add_local_once(Duration::from_millis(1200), move || {
                        feedback::set_pending_state(
                            &button_after_delay,
                            &spinner_after_delay,
                            false,
                        );
                    });
                }
            }
        });
    }

    /// Expose la fenêtre principale si nécessaire pour l'affichage
    pub fn window(&self) -> &gtk4::Window {
        &self.window
    }

    /// Présente la fenêtre de dialogue et met le focus sur le champ approprié
    pub fn present(&self) {
        self.window.present();

        // Mettre le focus sur le champ approprié selon le mode
        if self.in_bootstrap_mode {
            // Mode bootstrap : focus sur le champ init_username_entry
            self.widgets.init_username_entry.grab_focus();
        } else {
            // Mode login normal : focus sur le champ username_entry
            self.widgets.username_entry.grab_focus();
        }
    }
}
