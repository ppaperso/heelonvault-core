use gtk4::{
    Align, InputPurpose, Justification, Orientation, PolicyType, Stack, StackTransitionType,
};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::ui::license_badge::LicenseDisplay;
use crate::ui::widgets::password_strength_bar::PasswordStrengthBar;

/// Structure contenant tous les widgets de la dialogue de connexion.
/// Cette structure est utilisée pour accéder aux widgets depuis core.rs et events.rs.
#[derive(Clone)]
pub struct LoginDialogWidgets {
    // Container principal
    pub container: gtk4::ScrolledWindow,

    // Barre de titre plate : langue à gauche, nom du produit, fermeture native à droite
    pub header_bar: adw::HeaderBar,
    pub language_button: gtk4::MenuButton,
    pub language_popover: gtk4::Popover,
    pub language_fr_button: gtk4::ToggleButton,
    pub language_en_button: gtk4::ToggleButton,

    // Shell et root
    pub shell: gtk4::Box,
    pub root: gtk4::Box,

    // En-tête (style « status page »)
    pub header_box: gtk4::Box,
    pub brand_icon: gtk4::Image,
    pub title_label: gtk4::Label,
    pub subtitle_label: gtk4::Label,
    pub meta_box: gtk4::Box,
    pub version_label: gtk4::Label,
    #[cfg(feature = "premium")]
    pub license_seal: gtk4::Box,
    /// "Certifié par Heelonys" caption of the seal (empty label in Community).
    #[cfg(feature = "premium")]
    pub license_seal_caption: gtk4::Label,
    pub license_badge: gtk4::Label,
    pub license: LicenseDisplay,

    // Formulaire
    pub form_box: gtk4::Box,

    // Teaser CPS (ligne atténuée + badge « À venir »)
    pub cps_row: gtk4::Box,
    pub cps_image: gtk4::Image,
    pub cps_name: gtk4::Label,
    pub cps_sub: gtk4::Label,
    pub cps_badge: gtk4::Label,

    // Champs de connexion
    pub credentials_list: gtk4::ListBox,
    pub username_entry: adw::EntryRow,
    pub password_entry: adw::PasswordEntryRow,
    pub strength_label: gtk4::Label,

    // Bouton de restauration
    pub restore_button: gtk4::Button,

    // Widgets PSC (Premium uniquement)
    #[cfg(feature = "premium")]
    pub psc_start_button: gtk4::Button,
    #[cfg(feature = "premium")]
    pub psc_artifact_entry: gtk4::Entry,
    #[cfg(feature = "premium")]
    pub psc_complete_button: gtk4::Button,

    // Boxes pour les étapes
    pub credentials_step_box: gtk4::Box,
    pub totp_step_box: gtk4::Box,

    // Widgets TOTP
    pub totp_back_button: gtk4::Button,
    pub totp_icon: gtk4::Image,
    pub totp_title: gtk4::Label,
    pub totp_subtitle: gtk4::Label,
    pub totp_entry: gtk4::Entry,

    // Stack pour les étapes
    pub step_stack: Stack,

    // Message d'erreur
    pub error_label: gtk4::Label,

    // Boutons : action principale puis liens secondaires
    pub button_box: gtk4::Box,
    pub links_box: gtk4::Box,
    pub back_button: gtk4::Button,
    pub submit_button: gtk4::Button,
    pub button_content: gtk4::Box,
    pub submit_spinner: gtk4::Spinner,
    pub button_label: gtk4::Label,

    // Clamp
    pub clamp: adw::Clamp,

    // Widgets Bootstrap - Identity
    pub init_identity_box: gtk4::Box,
    pub init_step_label_1: gtk4::Label,
    pub init_identity_list: gtk4::ListBox,
    pub init_username_entry: adw::EntryRow,
    pub init_password_entry: adw::PasswordEntryRow,
    pub init_confirm_entry: adw::PasswordEntryRow,
    pub init_strength_bar: PasswordStrengthBar,
    pub init_password_hint: gtk4::Label,

    // Widgets Bootstrap - Oath
    pub init_oath_box: gtk4::Box,
    pub init_step_label_2: gtk4::Label,
    pub init_warning_frame: gtk4::Frame,
    pub init_warning_label: gtk4::Label,
    pub init_key_title: gtk4::Label,
    pub init_grid_frame: gtk4::Frame,
    pub word_flow: gtk4::FlowBox,
    pub word_labels: Vec<gtk4::Label>,
    pub init_copy_button: gtk4::Button,
    pub init_verify_hint_label: gtk4::Label,
    pub init_verify_a_label: gtk4::Label,
    pub init_verify_a_entry: gtk4::Entry,
    pub init_verify_b_label: gtk4::Label,
    pub init_verify_b_entry: gtk4::Entry,
    pub verify_col_a: gtk4::Box,
    pub verify_col_b: gtk4::Box,
    pub verify_row: gtk4::Box,

    // Widgets Bootstrap - Pending
    pub init_pending_box: gtk4::Box,
    pub init_pending_spinner: gtk4::Spinner,
    pub init_pending_label: gtk4::Label,
}

/// Libellé du sélecteur de langue : la langue active, dans sa propre langue.
fn current_language_name() -> String {
    if heelonvault_core::i18n::current_language()
        .to_ascii_lowercase()
        .starts_with("en")
    {
        heelonvault_core::tr!("login-language-en")
    } else {
        heelonvault_core::tr!("login-language-fr")
    }
}

/// Champ de formulaire libadwaita précédé d'une icône neutre.
fn entry_row(title: &str, icon_name: &str) -> adw::EntryRow {
    let row = adw::EntryRow::builder().title(title).build();
    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.add_css_class("login-row-icon");
    row.add_prefix(&icon);
    row
}

fn password_row(title: &str) -> adw::PasswordEntryRow {
    let row = adw::PasswordEntryRow::builder().title(title).build();
    let icon = gtk4::Image::from_icon_name("hv-lock-symbolic");
    icon.add_css_class("login-row-icon");
    row.add_prefix(&icon);
    row
}

fn boxed_list() -> gtk4::ListBox {
    let list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .build();
    list.add_css_class("boxed-list");
    list.add_css_class("login-boxed-list");
    list
}

/// Construit l'interface utilisateur complète de la dialogue de connexion.
///
/// Cette fonction crée tous les widgets nécessaires pour l'interface de connexion,
/// y compris l'en-tête, le formulaire de connexion, les sections TOTP et bootstrap.
///
/// # Arguments
/// * `license` - Licence affichée (badge Community ou sceau Professionnel)
/// * `in_bootstrap_mode` - Si vrai, active le mode d'initialisation (premier admin)
///
/// # Returns
/// Une structure `LoginDialogWidgets` contenant tous les widgets créés.
pub fn build_login_view(license: &LicenseDisplay, in_bootstrap_mode: bool) -> LoginDialogWidgets {
    // ─── Barre de titre ──────────────────────────────────────────────────────────
    let header_bar = adw::HeaderBar::new();
    header_bar.add_css_class("flat");
    header_bar.add_css_class("login-headerbar");
    // Product name, not translated (same as the window icon caption).
    let header_title = gtk4::Label::new(Some("HeelonVault"));
    header_title.add_css_class("heading");
    header_bar.set_title_widget(Some(&header_title));

    let language_fr_button =
        gtk4::ToggleButton::with_label(heelonvault_core::tr!("login-language-fr").as_str());
    language_fr_button.add_css_class("flat");
    language_fr_button.add_css_class("login-language-choice");
    let language_en_button =
        gtk4::ToggleButton::with_label(heelonvault_core::tr!("login-language-en").as_str());
    language_en_button.add_css_class("flat");
    language_en_button.add_css_class("login-language-choice");
    language_en_button.set_group(Some(&language_fr_button));

    // Définir la langue active initiale
    let current_lang = heelonvault_core::i18n::current_language();
    if current_lang.to_ascii_lowercase().starts_with("en") {
        language_en_button.set_active(true);
    } else {
        language_fr_button.set_active(true);
    }

    let language_choices = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .build();
    language_choices.append(&language_fr_button);
    language_choices.append(&language_en_button);
    let language_popover = gtk4::Popover::new();
    language_popover.set_child(Some(&language_choices));

    let language_button = gtk4::MenuButton::builder()
        .label(current_language_name().as_str())
        .tooltip_text(heelonvault_core::tr!("login-language-label").as_str())
        .popover(&language_popover)
        .build();
    language_button.add_css_class("flat");
    language_button.add_css_class("login-language-button");
    header_bar.pack_start(&language_button);

    // ─── Section Shell ────────────────────────────────────────────────────────────
    let shell = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .margin_top(12)
        .margin_bottom(32)
        .margin_start(16)
        .margin_end(16)
        .build();
    shell.add_css_class("login-shell");
    shell.set_vexpand(true);

    let root = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(24)
        .halign(Align::Fill)
        .valign(if in_bootstrap_mode {
            Align::Start
        } else {
            Align::Center
        })
        .vexpand(true)
        .build();
    root.add_css_class("login-panel");

    // ─── En-tête ─────────────────────────────────────────────────────────────────
    let header_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .halign(Align::Fill)
        .build();
    header_box.add_css_class("login-header");

    let brand_icon = gtk4::Image::from_resource(
        "/com/heelonvault/rust/icons/hicolor/128x128/apps/heelonvault.png",
    );
    brand_icon.set_pixel_size(if in_bootstrap_mode { 64 } else { 88 });
    brand_icon.set_halign(Align::Center);
    brand_icon.set_margin_bottom(8);
    brand_icon.add_css_class("login-brand-icon");

    let title_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-hero-title").as_str()));
    title_label.add_css_class("title-1");
    title_label.add_css_class("login-title");
    title_label.set_wrap(true);
    title_label.set_justify(Justification::Center);
    title_label.set_halign(Align::Center);

    let subtitle_label =
        gtk4::Label::new(Some(heelonvault_core::tr!("login-hero-subtitle").as_str()));
    subtitle_label.add_css_class("login-subtitle");
    subtitle_label.set_wrap(true);
    subtitle_label.set_justify(Justification::Center);
    subtitle_label.set_halign(Align::Center);

    // Mode bootstrap: adapter les textes de l'en-tête
    if in_bootstrap_mode {
        title_label.set_text(heelonvault_core::tr!("init-hero-title").as_str());
        subtitle_label.set_text(heelonvault_core::tr!("init-hero-subtitle").as_str());
    }

    // Licence et version : une ligne discrète sous le sous-titre.
    let meta_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .halign(Align::Center)
        .margin_top(4)
        .build();

    let version_label = gtk4::Label::new(Some(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    version_label.add_css_class("login-version");
    version_label.set_valign(Align::Center);

    // License seal (Premium) ou badge simple
    #[cfg(feature = "premium")]
    let (license_seal, license_seal_caption, license_badge) =
        if let Some(customer_name) = license.customer_name() {
            let seal = gtk4::Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(8)
                .halign(Align::Start)
                .valign(Align::Center)
                .build();
            seal.add_css_class("heelonys-seal");
            seal.add_css_class("heelonys-seal-login");

            let icon_box = gtk4::Box::builder()
                .orientation(Orientation::Horizontal)
                .spacing(2)
                .valign(Align::Center)
                .build();
            icon_box.add_css_class("heelonys-seal-emblem");

            let shield_icon = gtk4::Image::from_icon_name("hv-shield-symbolic");
            shield_icon.add_css_class("heelonys-seal-shield");
            let check_dot = gtk4::Label::new(Some("•"));
            check_dot.add_css_class("heelonys-seal-dot");
            icon_box.append(&shield_icon);
            icon_box.append(&check_dot);

            let divider = gtk4::Separator::new(Orientation::Vertical);
            divider.add_css_class("heelonys-seal-divider");

            let text_box = gtk4::Box::builder()
                .orientation(Orientation::Vertical)
                .spacing(1)
                .valign(Align::Center)
                .build();

            let cert_label = gtk4::Label::new(Some(
                heelonvault_core::tr!("license-seal-certified-by").as_str(),
            ));
            cert_label.set_halign(Align::Start);
            cert_label.add_css_class("heelonys-seal-cert");
            let customer_label = gtk4::Label::new(Some(customer_name.as_str()));
            customer_label.set_halign(Align::Start);
            customer_label.add_css_class("heelonys-seal-customer");
            text_box.append(&cert_label);
            text_box.append(&customer_label);

            seal.append(&icon_box);
            seal.append(&divider);
            seal.append(&text_box);
            meta_box.append(&seal);

            // Retourner un dummy license_badge pour satisfaire la structure
            let dummy_badge = gtk4::Label::new(None);
            (seal, cert_label, dummy_badge)
        } else {
            let lb = gtk4::Label::new(Some(license.badge_text().as_str()));
            lb.add_css_class("login-license-badge");
            lb.set_valign(Align::Center);
            meta_box.append(&lb);
            // Retourner un dummy license_seal
            let dummy_seal = gtk4::Box::builder().build();
            (dummy_seal, gtk4::Label::new(None), lb)
        };

    #[cfg(not(feature = "premium"))]
    let license_badge = {
        let lb = gtk4::Label::new(Some(license.badge_text().as_str()));
        lb.add_css_class("login-license-badge");
        lb.set_valign(Align::Center);
        meta_box.append(&lb);
        lb
    };

    meta_box.append(&version_label);

    header_box.append(&brand_icon);
    header_box.append(&title_label);
    header_box.append(&subtitle_label);
    header_box.append(&meta_box);

    // ─── Section Formulaire ───────────────────────────────────────────────────────
    let form_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .build();
    form_box.add_css_class("login-form");

    // Teaser CPS : ligne atténuée, non interactive
    let cps_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .build();
    cps_row.add_css_class("login-cps-row");
    cps_row.set_sensitive(false);

    let cps_image = gtk4::Image::from_resource("/com/heelonvault/rust/images/cps_card.png");
    cps_image.add_css_class("login-cps-image");
    cps_image.set_pixel_size(32);
    cps_image.set_size_request(48, 32);
    cps_image.set_valign(Align::Center);

    let cps_info = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .build();

    let cps_name = gtk4::Label::new(Some(heelonvault_core::tr!("login-cps-name").as_str()));
    cps_name.add_css_class("login-cps-title");
    cps_name.set_halign(Align::Start);

    let cps_sub = gtk4::Label::new(Some(heelonvault_core::tr!("login-cps-subtitle").as_str()));
    cps_sub.add_css_class("login-cps-copy");
    cps_sub.set_halign(Align::Start);

    let cps_badge = gtk4::Label::new(Some(heelonvault_core::tr!("login-cps-badge").as_str()));
    cps_badge.add_css_class("login-cps-badge");
    cps_badge.set_valign(Align::Center);

    cps_info.append(&cps_name);
    cps_info.append(&cps_sub);
    cps_row.append(&cps_image);
    cps_row.append(&cps_info);
    cps_row.append(&cps_badge);

    // Champs de connexion : une liste encadrée libadwaita
    let username_entry = entry_row(
        heelonvault_core::tr!("login-username-label").as_str(),
        "hv-user-symbolic",
    );
    username_entry.set_tooltip_text(Some(heelonvault_core::tr!("login-username-hint").as_str()));

    let password_entry = password_row(heelonvault_core::tr!("login-password-label").as_str());

    let credentials_list = boxed_list();
    credentials_list.append(&username_entry);
    credentials_list.append(&password_entry);

    let strength_label = gtk4::Label::new(None);
    strength_label.add_css_class("login-strength");
    strength_label.set_halign(Align::Start);
    strength_label.set_visible(false);

    let restore_button =
        gtk4::Button::with_label(heelonvault_core::tr!("login-restore-button").as_str());
    restore_button.add_css_class("flat");
    restore_button.add_css_class("login-link-accent");

    // Widgets PSC (Premium uniquement)
    #[cfg(feature = "premium")]
    let psc_start_button =
        gtk4::Button::with_label(heelonvault_core::tr!("psc-start-button").as_str());
    #[cfg(feature = "premium")]
    {
        psc_start_button.add_css_class("suggested-action");
        psc_start_button.set_halign(Align::Fill);
    }

    #[cfg(feature = "premium")]
    let psc_artifact_entry = gtk4::Entry::builder()
        .placeholder_text(heelonvault_core::tr!("psc-artifact-placeholder").as_str())
        .hexpand(true)
        .build();

    #[cfg(feature = "premium")]
    let psc_complete_button =
        gtk4::Button::with_label(heelonvault_core::tr!("psc-complete-button").as_str());
    #[cfg(feature = "premium")]
    {
        psc_complete_button.add_css_class("secondary-pill");
    }

    // STEP 1: Credentials form
    let credentials_step_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .build();

    // PSC (Pro Santé Connect) désactivé côté UI : ni HEELONVAULT_PSC_CLIENT_ID/SECRET
    // ne sont configurés en usage courant (cf. le WARN "PSC configuration not
    // available; using non-operational premium placeholder" au démarrage), ni
    // psc_start_button / psc_complete_button ne sont câblés à un handler
    // (handle_psc_start / handle_psc_artifact_completion existent dans
    // login_flow.rs mais ne sont appelés nulle part). Afficher ce bloc n'offre
    // donc qu'un bouton mort. Les widgets restent construits (litige compile
    // sous `premium`) mais ne sont plus insérés dans la vue tant que PSC n'est
    // pas réellement opérationnel.
    #[cfg(feature = "premium")]
    {
        let _ = (
            &cps_row,
            &psc_start_button,
            &psc_artifact_entry,
            &psc_complete_button,
        );
        cps_row.set_visible(false);
    }
    #[cfg(not(feature = "premium"))]
    {
        cps_row.set_visible(false);
    }

    credentials_step_box.append(&credentials_list);
    credentials_step_box.append(&strength_label);

    // STEP 2: TOTP view
    let totp_step_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .build();
    totp_step_box.add_css_class("login-totp-block");

    let totp_back_button =
        gtk4::Button::with_label(heelonvault_core::tr!("login-totp-back").as_str());
    totp_back_button.add_css_class("flat");
    totp_back_button.set_halign(Align::Start);

    let totp_icon = gtk4::Image::from_icon_name("hv-shield-symbolic");
    totp_icon.set_pixel_size(40);
    totp_icon.set_halign(Align::Center);
    totp_icon.add_css_class("login-totp-icon");

    let totp_title = gtk4::Label::new(Some(heelonvault_core::tr!("login-totp-title").as_str()));
    totp_title.add_css_class("title-3");
    totp_title.set_halign(Align::Center);

    let totp_subtitle =
        gtk4::Label::new(Some(heelonvault_core::tr!("login-totp-subtitle").as_str()));
    totp_subtitle.add_css_class("login-subtitle");
    totp_subtitle.set_wrap(true);
    totp_subtitle.set_justify(Justification::Center);
    totp_subtitle.set_halign(Align::Fill);

    let totp_entry = gtk4::Entry::builder()
        .placeholder_text("000000")
        .max_length(6)
        .input_purpose(InputPurpose::Digits)
        .build();
    totp_entry.add_css_class("login-totp-entry");
    totp_entry.set_halign(Align::Center);
    totp_entry.set_width_chars(8);
    totp_entry.set_margin_top(6);
    gtk4::prelude::EntryExt::set_alignment(&totp_entry, 0.5);

    totp_step_box.append(&totp_back_button);
    totp_step_box.append(&totp_icon);
    totp_step_box.append(&totp_title);
    totp_step_box.append(&totp_subtitle);
    totp_step_box.append(&totp_entry);

    // Stack pour basculer entre les étapes
    let step_stack = Stack::builder()
        .transition_type(StackTransitionType::Crossfade)
        .vhomogeneous(false)
        .interpolate_size(true)
        .build();

    step_stack.add_named(&credentials_step_box, Some("credentials"));
    step_stack.add_named(&totp_step_box, Some("totp"));
    step_stack.set_visible_child_name("credentials");

    // Message d'erreur
    let error_label = gtk4::Label::new(None);
    error_label.add_css_class("login-error");
    error_label.set_wrap(true);
    error_label.set_justify(Justification::Center);
    error_label.set_halign(Align::Center);
    error_label.set_visible(false);

    // Boutons : l'action principale en pilule, puis des liens plats
    let button_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .build();

    let submit_button = gtk4::Button::builder()
        .halign(Align::Center)
        .width_request(220)
        .build();
    submit_button.add_css_class("suggested-action");
    submit_button.add_css_class("pill");
    submit_button.add_css_class("login-submit");

    // Mode bootstrap: bouton désactivé au début (sera activé quand les champs sont valides)
    if in_bootstrap_mode {
        submit_button.set_sensitive(false);
    }

    let button_content = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .halign(Align::Center)
        .build();

    let submit_spinner = gtk4::Spinner::new();
    submit_spinner.set_visible(false);

    let button_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-button").as_str()));

    // Mode bootstrap: adapter le texte du bouton
    if in_bootstrap_mode {
        button_label.set_text(heelonvault_core::tr!("init-next-button").as_str());
    }

    button_content.append(&submit_spinner);
    button_content.append(&button_label);
    submit_button.set_child(Some(&button_content));

    let back_button = gtk4::Button::with_label(heelonvault_core::tr!("login-back-button").as_str());
    back_button.add_css_class("flat");
    back_button.add_css_class("login-link");

    let links_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(4)
        .halign(Align::Center)
        .build();
    // Le bouton de restauration n'est proposé qu'à la saisie des identifiants (pas en
    // bootstrap, pas pendant l'étape TOTP).
    if !in_bootstrap_mode {
        links_box.append(&restore_button);
        let restore_for_step = restore_button.clone();
        step_stack.connect_visible_child_name_notify(move |stack| {
            restore_for_step.set_visible(
                stack
                    .visible_child_name()
                    .is_some_and(|n| n == "credentials"),
            );
        });
    }
    links_box.append(&back_button);

    button_box.append(&submit_button);
    button_box.append(&links_box);

    // Assemblage du form_box
    form_box.append(&step_stack);
    form_box.append(&error_label);
    form_box.append(&button_box);
    form_box.append(&cps_row);

    // ─── Bootstrap Widgets - Identity Step ────────────────────────────────────────
    let init_identity_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .build();

    let init_step_label_1 = gtk4::Label::new(Some(
        heelonvault_core::tr!("init-step-label-identity").as_str(),
    ));
    init_step_label_1.add_css_class("login-step-label");
    init_step_label_1.set_halign(Align::Start);

    let init_username_entry = entry_row(
        heelonvault_core::tr!("init-username-label").as_str(),
        "hv-user-symbolic",
    );
    let init_password_entry = password_row(heelonvault_core::tr!("init-password-label").as_str());
    let init_confirm_entry = password_row(heelonvault_core::tr!("init-confirm-label").as_str());

    let init_identity_list = boxed_list();
    init_identity_list.append(&init_username_entry);
    init_identity_list.append(&init_password_entry);
    init_identity_list.append(&init_confirm_entry);

    let init_strength_bar = PasswordStrengthBar::new();

    let init_password_hint =
        gtk4::Label::new(Some(heelonvault_core::tr!("init-password-hint").as_str()));
    init_password_hint.add_css_class("login-hint");
    init_password_hint.set_wrap(true);
    init_password_hint.set_halign(Align::Start);

    init_identity_box.append(&init_step_label_1);
    init_identity_box.append(&init_identity_list);
    init_identity_box.append(init_strength_bar.root());
    init_identity_box.append(&init_password_hint);

    // ─── Bootstrap Widgets - Oath Step ────────────────────────────────────────────
    let init_oath_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .build();

    let init_step_label_2 =
        gtk4::Label::new(Some(heelonvault_core::tr!("init-step-label-oath").as_str()));
    init_step_label_2.add_css_class("login-step-label");
    init_step_label_2.set_halign(Align::Start);

    let init_warning_frame = gtk4::Frame::new(None);
    init_warning_frame.add_css_class("init-warning-banner");

    let init_warning_label =
        gtk4::Label::new(Some(heelonvault_core::tr!("init-oath-warning").as_str()));
    init_warning_label.set_wrap(true);
    init_warning_label.set_margin_top(10);
    init_warning_label.set_margin_bottom(10);
    init_warning_label.set_margin_start(12);
    init_warning_label.set_margin_end(12);
    init_warning_label.set_halign(Align::Start);
    init_warning_frame.set_child(Some(&init_warning_label));

    let init_key_title = gtk4::Label::new(Some(
        heelonvault_core::tr!("init-recovery-key-title").as_str(),
    ));
    init_key_title.add_css_class("heading");
    init_key_title.set_halign(Align::Start);

    let init_grid_frame = gtk4::Frame::new(None);
    init_grid_frame.add_css_class("init-recovery-grid-frame");

    let word_flow = gtk4::FlowBox::builder()
        .max_children_per_line(6)
        .min_children_per_line(6)
        .selection_mode(gtk4::SelectionMode::None)
        .homogeneous(true)
        .column_spacing(4)
        .row_spacing(4)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(10)
        .build();

    let mut word_labels: Vec<gtk4::Label> = Vec::with_capacity(24);
    for i in 0..24_usize {
        let item_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(2)
            .halign(Align::Center)
            .build();
        item_box.add_css_class("init-recovery-badge");

        let num_label = gtk4::Label::new(Some(&format!("{:02}", i + 1)));
        num_label.add_css_class("init-badge-num");
        num_label.set_halign(Align::Center);

        let word_label = gtk4::Label::new(Some("•••"));
        word_label.add_css_class("init-badge-word");
        word_label.set_halign(Align::Center);
        word_label.set_selectable(false);

        item_box.append(&num_label);
        item_box.append(&word_label);
        word_labels.push(word_label);
        word_flow.insert(&item_box, -1);
    }

    init_grid_frame.set_child(Some(&word_flow));

    let init_copy_button =
        gtk4::Button::with_label(heelonvault_core::tr!("init-copy-button").as_str());
    init_copy_button.add_css_class("flat");
    init_copy_button.add_css_class("login-link-accent");
    init_copy_button.set_halign(Align::End);

    let init_verify_hint_label = gtk4::Label::new(None);
    init_verify_hint_label.add_css_class("login-hint");
    init_verify_hint_label.set_wrap(true);
    init_verify_hint_label.set_halign(Align::Start);

    let init_verify_a_label = gtk4::Label::new(None);
    init_verify_a_label.add_css_class("login-field-caption");
    init_verify_a_label.set_halign(Align::Start);

    let init_verify_a_entry = gtk4::Entry::builder()
        .placeholder_text(heelonvault_core::tr!("init-verify-placeholder").as_str())
        .hexpand(true)
        .build();

    let init_verify_b_label = gtk4::Label::new(None);
    init_verify_b_label.add_css_class("login-field-caption");
    init_verify_b_label.set_halign(Align::Start);

    let init_verify_b_entry = gtk4::Entry::builder()
        .placeholder_text(heelonvault_core::tr!("init-verify-placeholder").as_str())
        .hexpand(true)
        .build();

    let verify_col_a = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .build();
    verify_col_a.append(&init_verify_a_label);
    verify_col_a.append(&init_verify_a_entry);

    let verify_col_b = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .build();
    verify_col_b.append(&init_verify_b_label);
    verify_col_b.append(&init_verify_b_entry);

    let verify_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .build();
    verify_row.append(&verify_col_a);
    verify_row.append(&verify_col_b);

    init_oath_box.append(&init_step_label_2);
    init_oath_box.append(&init_warning_frame);
    init_oath_box.append(&init_key_title);
    init_oath_box.append(&init_grid_frame);
    init_oath_box.append(&init_copy_button);
    init_oath_box.append(&init_verify_hint_label);
    init_oath_box.append(&verify_row);

    // ─── Bootstrap Widgets - Pending Step ──────────────────────────────────────────
    let init_pending_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .halign(Align::Fill)
        .valign(Align::Center)
        .build();

    let init_pending_spinner = gtk4::Spinner::new();
    init_pending_spinner.set_halign(Align::Center);
    init_pending_spinner.set_size_request(32, 32);

    let init_pending_label =
        gtk4::Label::new(Some(heelonvault_core::tr!("init-progress-label").as_str()));
    init_pending_label.add_css_class("login-subtitle");
    init_pending_label.set_halign(Align::Center);

    init_pending_box.append(&init_pending_spinner);
    init_pending_box.append(&init_pending_label);

    // Mode bootstrap: ajouter les étapes d'initialisation au step_stack
    if in_bootstrap_mode {
        step_stack.add_named(&init_identity_box, Some("init-identity"));
        step_stack.add_named(&init_oath_box, Some("init-oath"));
        step_stack.add_named(&init_pending_box, Some("init-pending"));
        step_stack.set_visible_child_name("init-identity");
    }

    // ─── Assemblage final ────────────────────────────────────────────────────────
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(if in_bootstrap_mode { 600 } else { 420 });
    clamp.set_tightening_threshold(360);

    root.append(&header_box);
    root.append(&form_box);

    clamp.set_child(Some(&root));
    shell.append(&clamp);

    let container = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .min_content_width(360)
        .build();
    container.add_css_class("login-surface");
    container.set_child(Some(&shell));

    // ─── Return structure ────────────────────────────────────────────────────────
    LoginDialogWidgets {
        container,
        header_bar,
        language_button,
        language_popover,
        language_fr_button,
        language_en_button,
        shell,
        root,
        header_box,
        brand_icon,
        title_label,
        subtitle_label,
        meta_box,
        version_label,
        #[cfg(feature = "premium")]
        license_seal,
        #[cfg(feature = "premium")]
        license_seal_caption,
        license_badge,
        license: license.clone(),
        form_box,
        cps_row,
        cps_image,
        cps_name,
        cps_sub,
        cps_badge,
        credentials_list,
        username_entry,
        password_entry,
        strength_label,
        restore_button,
        #[cfg(feature = "premium")]
        psc_start_button,
        #[cfg(feature = "premium")]
        psc_artifact_entry,
        #[cfg(feature = "premium")]
        psc_complete_button,
        credentials_step_box,
        totp_step_box,
        totp_back_button,
        totp_icon,
        totp_title,
        totp_subtitle,
        totp_entry,
        step_stack,
        error_label,
        button_box,
        links_box,
        back_button,
        submit_button,
        button_content,
        submit_spinner,
        button_label,
        clamp,
        init_identity_box,
        init_step_label_1,
        init_identity_list,
        init_username_entry,
        init_password_entry,
        init_confirm_entry,
        init_strength_bar,
        init_password_hint,
        init_oath_box,
        init_step_label_2,
        init_warning_frame,
        init_warning_label,
        init_key_title,
        init_grid_frame,
        word_flow,
        word_labels,
        init_copy_button,
        init_verify_hint_label,
        init_verify_a_label,
        init_verify_a_entry,
        init_verify_b_label,
        init_verify_b_entry,
        verify_col_a,
        verify_col_b,
        verify_row,
        init_pending_box,
        init_pending_spinner,
        init_pending_label,
    }
}

/// Configure le basculement de langue (FR/EN) pour tous les widgets.
///
/// Cette fonction connecte les boutons de langue et met à jour dynamiquement
/// tous les textes de l'interface lorsque la langue change.
///
/// # Arguments
/// * `widgets` - Référence vers tous les widgets de la dialogue
/// * `in_bootstrap_mode` - Si vrai, adapte les textes pour le mode bootstrap
pub fn setup_language_toggle(widgets: &LoginDialogWidgets, in_bootstrap_mode: bool) {
    use std::cell::Cell;
    use std::rc::Rc;

    // Guard pour éviter les boucles lors du changement de langue
    let language_toggle_guard = Rc::new(Cell::new(false));

    // Fonction pour appliquer la traduction à tous les widgets
    let apply_login_i18n = {
        let widgets_clone = widgets.clone();
        let guard_clone = Rc::clone(&language_toggle_guard);
        let in_bootstrap = in_bootstrap_mode;

        Rc::new(move || {
            guard_clone.set(true);

            // Mettre à jour les textes en fonction du mode bootstrap
            if in_bootstrap {
                widgets_clone
                    .title_label
                    .set_text(heelonvault_core::tr!("init-hero-title").as_str());
                widgets_clone
                    .subtitle_label
                    .set_text(heelonvault_core::tr!("init-hero-subtitle").as_str());
                widgets_clone
                    .button_label
                    .set_text(heelonvault_core::tr!("init-next-button").as_str());
            } else {
                widgets_clone
                    .title_label
                    .set_text(heelonvault_core::tr!("login-hero-title").as_str());
                widgets_clone
                    .subtitle_label
                    .set_text(heelonvault_core::tr!("login-hero-subtitle").as_str());
                widgets_clone
                    .button_label
                    .set_text(heelonvault_core::tr!("login-button").as_str());
            }

            // Mettre à jour les textes communs
            widgets_clone
                .cps_name
                .set_text(heelonvault_core::tr!("login-cps-name").as_str());
            widgets_clone
                .cps_sub
                .set_text(heelonvault_core::tr!("login-cps-subtitle").as_str());
            widgets_clone
                .cps_badge
                .set_text(heelonvault_core::tr!("login-cps-badge").as_str());
            widgets_clone
                .username_entry
                .set_title(heelonvault_core::tr!("login-username-label").as_str());
            widgets_clone
                .username_entry
                .set_tooltip_text(Some(heelonvault_core::tr!("login-username-hint").as_str()));
            widgets_clone
                .password_entry
                .set_title(heelonvault_core::tr!("login-password-label").as_str());
            widgets_clone
                .restore_button
                .set_label(heelonvault_core::tr!("login-restore-button").as_str());
            widgets_clone
                .totp_back_button
                .set_label(heelonvault_core::tr!("login-totp-back").as_str());
            widgets_clone
                .totp_title
                .set_text(heelonvault_core::tr!("login-totp-title").as_str());
            widgets_clone
                .totp_subtitle
                .set_text(heelonvault_core::tr!("login-totp-subtitle").as_str());
            widgets_clone
                .back_button
                .set_label(heelonvault_core::tr!("login-back-button").as_str());
            widgets_clone
                .language_button
                .set_label(current_language_name().as_str());
            widgets_clone
                .language_button
                .set_tooltip_text(Some(heelonvault_core::tr!("login-language-label").as_str()));
            widgets_clone
                .language_fr_button
                .set_label(heelonvault_core::tr!("login-language-fr").as_str());
            widgets_clone
                .language_en_button
                .set_label(heelonvault_core::tr!("login-language-en").as_str());

            // Bootstrap widgets (même s'ils ne sont pas visibles, on met à jour pour quand ils le seront)
            widgets_clone
                .init_step_label_1
                .set_text(heelonvault_core::tr!("init-step-label-identity").as_str());
            widgets_clone
                .init_username_entry
                .set_title(heelonvault_core::tr!("init-username-label").as_str());
            widgets_clone
                .init_password_entry
                .set_title(heelonvault_core::tr!("init-password-label").as_str());
            widgets_clone
                .init_confirm_entry
                .set_title(heelonvault_core::tr!("init-confirm-label").as_str());
            widgets_clone
                .init_password_hint
                .set_text(heelonvault_core::tr!("init-password-hint").as_str());
            widgets_clone
                .init_step_label_2
                .set_text(heelonvault_core::tr!("init-step-label-oath").as_str());
            widgets_clone
                .init_warning_label
                .set_text(heelonvault_core::tr!("init-oath-warning").as_str());
            widgets_clone
                .init_key_title
                .set_text(heelonvault_core::tr!("init-recovery-key-title").as_str());
            widgets_clone
                .init_copy_button
                .set_label(heelonvault_core::tr!("init-copy-button").as_str());
            widgets_clone
                .init_verify_hint_label
                .set_text(heelonvault_core::tr!("init-oath-warning").as_str());
            widgets_clone.init_verify_a_entry.set_placeholder_text(Some(
                heelonvault_core::tr!("init-verify-placeholder").as_str(),
            ));
            widgets_clone.init_verify_b_entry.set_placeholder_text(Some(
                heelonvault_core::tr!("init-verify-placeholder").as_str(),
            ));
            widgets_clone
                .init_pending_label
                .set_text(heelonvault_core::tr!("init-progress-label").as_str());

            // Définir la langue active initiale sur les boutons
            let current_lang = heelonvault_core::i18n::current_language();
            let active_is_en = current_lang.to_ascii_lowercase().starts_with("en");
            if active_is_en {
                widgets_clone.language_en_button.set_active(true);
            } else {
                widgets_clone.language_fr_button.set_active(true);
            }

            widgets_clone
                .license_badge
                .set_text(widgets_clone.license.badge_text().as_str());
            #[cfg(feature = "premium")]
            {
                widgets_clone
                    .license_seal_caption
                    .set_text(heelonvault_core::tr!("license-seal-certified-by").as_str());
                widgets_clone
                    .psc_start_button
                    .set_label(heelonvault_core::tr!("psc-start-button").as_str());
                widgets_clone.psc_artifact_entry.set_placeholder_text(Some(
                    heelonvault_core::tr!("psc-artifact-placeholder").as_str(),
                ));
                widgets_clone
                    .psc_complete_button
                    .set_label(heelonvault_core::tr!("psc-complete-button").as_str());
            }

            // Mettre à jour le texte du bouton en fonction de l'étape courante
            let step_stack = widgets_clone.step_stack.clone();
            let button_label_clone = widgets_clone.button_label.clone();
            let step = step_stack
                .visible_child_name()
                .as_ref()
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| "credentials".to_string());

            match step.as_str() {
                "totp" => {
                    button_label_clone
                        .set_text(heelonvault_core::tr!("login-button-verify").as_str());
                }
                "init-identity" => {
                    button_label_clone.set_text(heelonvault_core::tr!("init-next-button").as_str());
                }
                "init-oath" => {
                    button_label_clone
                        .set_text(heelonvault_core::tr!("init-confirm-button").as_str());
                }
                "init-pending" => {}
                _ => {
                    if in_bootstrap {
                        button_label_clone
                            .set_text(heelonvault_core::tr!("init-next-button").as_str());
                    } else {
                        button_label_clone.set_text(heelonvault_core::tr!("login-button").as_str());
                    }
                }
            }

            guard_clone.set(false);
        })
    };

    // Connexion du signal toggled pour le bouton FR
    {
        let apply_i18n = Rc::clone(&apply_login_i18n);
        let guard = Rc::clone(&language_toggle_guard);
        let fr_button = widgets.language_fr_button.clone();
        let popover = widgets.language_popover.clone();
        fr_button.connect_toggled(move |button| {
            if guard.get() || !button.is_active() {
                return;
            }
            popover.popdown();
            if !heelonvault_core::i18n::current_language()
                .to_ascii_lowercase()
                .starts_with("fr")
            {
                let _ = heelonvault_core::i18n::set_language("fr");
                apply_i18n();
            }
        });
    }

    // Connexion du signal toggled pour le bouton EN
    {
        let apply_i18n = Rc::clone(&apply_login_i18n);
        let guard = Rc::clone(&language_toggle_guard);
        let en_button = widgets.language_en_button.clone();
        let popover = widgets.language_popover.clone();
        en_button.connect_toggled(move |button| {
            if guard.get() || !button.is_active() {
                return;
            }
            popover.popdown();
            if !heelonvault_core::i18n::current_language()
                .to_ascii_lowercase()
                .starts_with("en")
            {
                let _ = heelonvault_core::i18n::set_language("en");
                apply_i18n();
            }
        });
    }

    // Appliquer une première fois pour initialiser les textes
    apply_login_i18n();
}

/// Extrait l'indice numérique d'un label de vérification (ex: "Mot n°5" -> 4)
///
/// Les labels de vérification ont le format "Mot n°X" ou "Word #X" où X est le numéro du mot (1-based).
/// Cette fonction retourne l'indice 0-based pour accéder au tableau des mots.
/// Gère aussi les caractères de formattage invisibles (U+2066..U+2069, etc.).
fn parse_index_from_label(label_text: &str) -> Option<usize> {
    // Nettoyer le texte des caractères de formattage invisibles (bidirectionnel)
    // U+2066..U+2069 = Left-to-Right/Right-to-Left Isolate characters
    // Ces caractères sont utilisés pour le formattage bidi et peuvent apparaître dans les traductions
    let cleaned_text: String = label_text
        .chars()
        .filter(|c| {
            // Garder les caractères imprimables normaux et les chiffres
            // Filtrer les caractères de contrôle et les isolate characters (U+2066..U+2069)
            let code = *c as u32;
            !c.is_control() && !(0x2066..=0x2069).contains(&code)
        })
        .collect();

    let text = cleaned_text.to_lowercase();

    // Essayer de trouver un nombre dans le texte
    // Utiliser .len() pour gérer correctement les caractères UTF-8 multi-octets
    if let Some(start) = text.find("n°") {
        let num_part = &text[start + "n°".len()..];
        if let Ok(num) = num_part.trim().parse::<usize>() {
            // Convertir de 1-based à 0-based
            return Some(num.saturating_sub(1));
        }
    }

    // Alternative: chercher "#" suivi d'un nombre
    if let Some(start) = text.find('#') {
        let num_part = &text[start + "#".len()..];
        if let Ok(num) = num_part.trim().parse::<usize>() {
            return Some(num.saturating_sub(1));
        }
    }

    // Alternative: chercher simplement le premier nombre dans le texte
    // (pour gérer des formats inattendus)
    if let Ok(num) = text
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse::<usize>()
    {
        return Some(num.saturating_sub(1));
    }

    None
}

/// Configure les "gates" (portails de validation) pour le mode bootstrap.
///
/// Ces gates activent/désactivent le bouton submit en fonction de la validité des champs :
/// - **Identity step** : username non vide, password non vide, confirmation == password, strength >= 3
/// - **Oath step** : les mots de vérification correspondent aux mots générés
///
/// # Arguments
/// * `widgets` - Référence vers tous les widgets de la dialogue
pub fn setup_bootstrap_gates(widgets: &LoginDialogWidgets) {
    use std::rc::Rc;

    // Connecter la strength bar à l'entry password pour qu'elle mette à jour son score
    widgets
        .init_strength_bar
        .connect_to_password_entry_row(&widgets.init_password_entry);

    // ─── Gate 1: Identity step ───────────────────────────────────────────────────
    // Le bouton est activé si : username non vide + password non vide + confirm == password + strength >= 3
    let check_init_identity_gate = {
        let username_entry = widgets.init_username_entry.clone();
        let password_entry = widgets.init_password_entry.clone();
        let confirm_entry = widgets.init_confirm_entry.clone();
        let strength_bar = widgets.init_strength_bar.clone();
        let submit_button = widgets.submit_button.clone();

        Rc::new(move || {
            let username_ok = !username_entry.text().trim().is_empty();
            let password_ok = !password_entry.text().is_empty();
            let confirm_ok = password_entry.text() == confirm_entry.text();
            let strength_ok = strength_bar.last_score() >= 3;

            let all_ok = username_ok && password_ok && confirm_ok && strength_ok;
            submit_button.set_sensitive(all_ok);
        })
    };

    // Connecter la gate identity aux champs
    {
        let gate = Rc::clone(&check_init_identity_gate);
        let username_entry = widgets.init_username_entry.clone();
        username_entry.connect_changed(move |_| gate());
    }
    {
        let gate = Rc::clone(&check_init_identity_gate);
        let password_entry = widgets.init_password_entry.clone();
        password_entry.connect_changed(move |_| gate());
    }
    {
        let gate = Rc::clone(&check_init_identity_gate);
        let confirm_entry = widgets.init_confirm_entry.clone();
        confirm_entry.connect_changed(move |_| gate());
    }

    // Connecter aussi à la strength bar (qui met à jour son score interne)
    // La strength bar écoute déjà les changements de l'entry, donc on déclenche la gate
    // quand son score change via un signal personnalisé ou on rely sur le fait que
    // le connect_to_password_entry est déjà appelé (à faire dans core.rs)

    // ─── Gate 2: Oath step ─────────────────────────────────────────────────────
    // Le bouton est activé si : les mots saisis correspondent aux mots aux indices sélectionnés
    let word_labels_for_oath = widgets.word_labels.clone();
    let init_verify_a_label_for_indices = widgets.init_verify_a_label.clone();
    let init_verify_b_label_for_indices = widgets.init_verify_b_label.clone();

    let check_init_oath_gate = {
        let verify_a_entry = widgets.init_verify_a_entry.clone();
        let verify_b_entry = widgets.init_verify_b_entry.clone();
        let word_labels = word_labels_for_oath.clone();
        let submit_button = widgets.submit_button.clone();

        Rc::new(move || {
            // Extraire les mots depuis les word_labels
            let words: Vec<String> = word_labels
                .iter()
                .map(|label| label.text().to_string())
                .collect();

            if words.len() != 24 {
                submit_button.set_sensitive(false);
                return;
            }

            // Extraire les indices depuis les labels (format : "Mot n°X")
            let ia_text = init_verify_a_label_for_indices.text();
            let ib_text = init_verify_b_label_for_indices.text();
            let ia_str = ia_text.as_str();
            let ib_str = ib_text.as_str();

            let (ia, ib) = match (
                parse_index_from_label(ia_str),
                parse_index_from_label(ib_str),
            ) {
                (Some(a), Some(b)) => (a, b),
                _ => {
                    submit_button.set_sensitive(false);
                    return;
                }
            };

            let a_ok = verify_a_entry.text().trim().to_lowercase() == words[ia].to_lowercase();
            let b_ok = verify_b_entry.text().trim().to_lowercase() == words[ib].to_lowercase();
            submit_button.set_sensitive(a_ok && b_ok);

            // Feedback visuel : appliquer des classes CSS pour indiquer le statut
            // Supprimer les classes précédentes
            verify_a_entry.remove_css_class("error");
            verify_a_entry.remove_css_class("success");
            verify_b_entry.remove_css_class("error");
            verify_b_entry.remove_css_class("success");

            if a_ok {
                verify_a_entry.add_css_class("success");
            } else if !verify_a_entry.text().trim().is_empty() {
                verify_a_entry.add_css_class("error");
            }

            if b_ok {
                verify_b_entry.add_css_class("success");
            } else if !verify_b_entry.text().trim().is_empty() {
                verify_b_entry.add_css_class("error");
            }
        })
    };

    // Connecter la gate oath aux champs de vérification
    {
        let gate = Rc::clone(&check_init_oath_gate);
        let verify_a_entry = widgets.init_verify_a_entry.clone();
        verify_a_entry.connect_changed(move |_| gate());
    }
    {
        let gate = Rc::clone(&check_init_oath_gate);
        let verify_b_entry = widgets.init_verify_b_entry.clone();
        verify_b_entry.connect_changed(move |_| gate());
    }

    // ─── Bouton Copy : copier la phrase vers le presse-papier ───────────────────────
    // Effacement automatique après 60 s, et dès la validation de l'étape (bootstrap_flow).
    {
        let word_labels_for_copy = widgets.word_labels.clone();
        let copy_button = widgets.init_copy_button.clone();

        copy_button.connect_clicked(move |_| {
            // Extraire la phrase depuis les word_labels (24 mots)
            let words: Vec<String> = word_labels_for_copy
                .iter()
                .map(|label| label.text().to_string())
                .collect();

            let phrase = words.join(" ");
            if phrase.is_empty() || words.len() != 24 {
                return;
            }

            crate::ui::sensitive_clipboard::copy_sensitive(
                &phrase,
                crate::ui::sensitive_clipboard::SensitiveKind::RecoveryPhrase,
                crate::ui::sensitive_clipboard::RECOVERY_PHRASE_CLEAR_DELAY,
            );
        });
    }

    // ─── Exécuter la vérification initiale des gates ───────────────────────────────
    // Déclencher la gate identity une première fois pour initialiser l'état du bouton
    check_init_identity_gate();
}
