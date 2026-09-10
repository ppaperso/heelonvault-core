use gtk4::prelude::*;
use gtk4::{
    Align, InputPurpose, Justification, Orientation, PolicyType, Separator, Stack, StackTransitionType,
};
use libadwaita as adw;

use crate::ui::widgets::password_strength_bar::PasswordStrengthBar;

/// Structure contenant tous les widgets de la dialogue de connexion.
/// Cette structure est utilisée pour accéder aux widgets depuis core.rs et events.rs.
#[derive(Clone)]
pub struct LoginDialogWidgets {
    // Container principal
    pub container: gtk4::ScrolledWindow,

    // Shell et root
    pub shell: gtk4::Box,
    pub root: gtk4::Box,

    // Section Hero
    pub hero_frame: gtk4::Frame,
    pub hero_box: gtk4::Box,
    pub hero_top: gtk4::Box,
    pub hero_icon: gtk4::Image,
    pub eyebrow_label: gtk4::Label,
    pub top_plan_badge: gtk4::Label,
    pub title_label: gtk4::Label,
    pub subtitle_label: gtk4::Label,
    pub badges_box: gtk4::Box,
    #[cfg(feature = "premium")]
    pub license_seal: gtk4::Box,
    pub license_badge: gtk4::Label,

    // Section Formulaire
    pub form_card: gtk4::Frame,
    pub form_box: gtk4::Box,

    // Sélecteur de langue
    pub language_row: gtk4::Box,
    pub language_label: gtk4::Label,
    pub language_buttons: gtk4::Box,
    pub language_fr_button: gtk4::ToggleButton,
    pub language_en_button: gtk4::ToggleButton,

    // Teaser CPS
    pub cps_frame: gtk4::Frame,
    pub cps_box: gtk4::Box,
    pub cps_image: gtk4::Image,
    pub cps_info: gtk4::Box,
    pub cps_name: gtk4::Label,
    pub cps_sub: gtk4::Label,
    pub cps_badge: gtk4::Label,

    // Champs de connexion
    pub username_label: gtk4::Label,
    pub username_entry: gtk4::Entry,
    pub password_label: gtk4::Label,
    pub password_entry: gtk4::PasswordEntry,
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
    pub credentials_box: gtk4::Box,
    pub credentials_step_box: gtk4::Box,
    pub totp_step_box: gtk4::Box,

    // Widgets TOTP
    pub totp_back_button: gtk4::Button,
    pub totp_spacer: Separator,
    pub totp_icon: gtk4::Image,
    pub totp_title: gtk4::Label,
    pub totp_subtitle: gtk4::Label,
    pub totp_entry_wrap: gtk4::Box,
    pub totp_entry: gtk4::Entry,

    // Stack pour les étapes
    pub step_stack: Stack,

    // Message d'erreur
    pub error_label: gtk4::Label,

    // Boutons
    pub button_box: gtk4::Box,
    pub back_button: gtk4::Button,
    pub submit_button: gtk4::Button,
    pub button_content: gtk4::Box,
    pub submit_spinner: gtk4::Spinner,
    pub button_label: gtk4::Label,

    // Strip de sécurité
    pub sec_strip: gtk4::Box,
    pub sec_dot: gtk4::Label,
    pub sec_text: gtk4::Label,

    // Clamp
    pub clamp: adw::Clamp,

    // Widgets Bootstrap - Identity
    pub init_identity_box: gtk4::Box,
    pub init_step_label_1: gtk4::Label,
    pub init_sep_1: Separator,
    pub init_username_label: gtk4::Label,
    pub init_username_entry: gtk4::Entry,
    pub init_password_label: gtk4::Label,
    pub init_password_entry: gtk4::PasswordEntry,
    pub init_strength_bar: PasswordStrengthBar,
    pub init_confirm_label: gtk4::Label,
    pub init_confirm_entry: gtk4::PasswordEntry,

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

/// Construit l'interface utilisateur complète de la dialogue de connexion.
/// 
/// Cette fonction crée tous les widgets nécessaires pour l'interface de connexion,
/// y compris la section hero, le formulaire de connexion, les sections TOTP et bootstrap.
/// 
/// # Arguments
/// * `license_badge_text` - Texte pour le badge de licence
/// * `in_bootstrap_mode` - Si vrai, active le mode d'initialisation (premier admin)
/// 
/// # Returns
/// Une structure `LoginDialogWidgets` contenant tous les widgets créés.
pub fn build_login_view(license_badge_text: String, in_bootstrap_mode: bool) -> LoginDialogWidgets {
    // ─── Section Shell ────────────────────────────────────────────────────────────
    let shell = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .build();
    shell.add_css_class("login-shell");
    shell.set_vexpand(true);

    let root = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(20)
        .halign(Align::Fill)
        .valign(if in_bootstrap_mode {
            Align::Start
        } else {
            Align::Center
        })
        .build();
    root.add_css_class("login-panel");

    // ─── Section Hero ────────────────────────────────────────────────────────────
    let hero_frame = gtk4::Frame::new(None);
    hero_frame.add_css_class("login-hero");

    let hero_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(24)
        .margin_bottom(28)
        .margin_start(24)
        .margin_end(24)
        .build();

    let hero_top = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .valign(Align::Center)
        .build();

    let hero_icon = gtk4::Image::from_resource(
        "/com/heelonvault/rust/icons/hicolor/128x128/apps/heelonvault.png",
    );
    hero_icon.set_pixel_size(44);
    hero_icon.set_halign(Align::Center);
    hero_icon.set_valign(Align::Center);
    hero_icon.add_css_class("login-hero-icon");

    let eyebrow_label = gtk4::Label::new(Some("HeelonVault"));
    eyebrow_label.add_css_class("login-badge");
    eyebrow_label.set_halign(Align::Start);

    let top_plan_badge = gtk4::Label::new(Some(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    top_plan_badge.add_css_class("beta-badge");
    top_plan_badge.add_css_class("login-beta-badge");
    top_plan_badge.set_halign(Align::Start);

    let title_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-hero-title").as_str()));
    title_label.add_css_class("title-1");
    title_label.add_css_class("login-hero-title");
    title_label.set_halign(Align::Start);

    let subtitle_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-hero-subtitle").as_str()));
    subtitle_label.add_css_class("login-hero-copy");
    subtitle_label.set_wrap(true);
    subtitle_label.set_halign(Align::Start);

    // Mode bootstrap: adapter les textes du hero
    if in_bootstrap_mode {
        title_label.set_text(heelonvault_core::tr!("init-hero-title").as_str());
        subtitle_label.set_text(heelonvault_core::tr!("init-hero-subtitle").as_str());
    }

    let badges_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .halign(Align::Start)
        .build();

    // Badges de fonctionnalités
    for text in ["AES-256-GCM".to_string(), "2FA TOTP".to_string()] {
        let badge = gtk4::Label::new(Some(text.as_str()));
        badge.add_css_class("login-hero-badge");
        badges_box.append(&badge);
    }

    // License seal (Premium) ou badge simple
    #[cfg(feature = "premium")]
    let (license_seal, license_badge) = if let Some(customer_name) = professional_customer_name(license_badge_text.as_str()) {
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

        let shield_icon = gtk4::Image::from_icon_name("security-high-symbolic");
        shield_icon.add_css_class("heelonys-seal-shield");
        let check_dot = gtk4::Label::new(Some("•"));
        check_dot.add_css_class("heelonys-seal-dot");
        icon_box.append(&shield_icon);
        icon_box.append(&check_dot);

        let divider = Separator::new(Orientation::Vertical);
        divider.add_css_class("heelonys-seal-divider");

        let text_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(1)
            .valign(Align::Center)
            .build();

        let cert_label = gtk4::Label::new(Some("CERTIFIE PAR HEELONYS"));
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
        badges_box.append(&seal);

        // Retourner un dummy license_badge pour satisfaire la structure
        let dummy_badge = gtk4::Label::new(None);
        (seal, dummy_badge)
    } else {
        let lb = gtk4::Label::new(Some(license_badge_text.as_str()));
        lb.add_css_class("login-hero-badge");
        lb.add_css_class("login-license-badge");
        badges_box.append(&lb);
        // Retourner un dummy license_seal
        let dummy_seal = gtk4::Box::builder().build();
        (dummy_seal, lb)
    };

    #[cfg(not(feature = "premium"))]
    let license_badge = {
        let lb = gtk4::Label::new(Some(license_badge_text.as_str()));
        lb.add_css_class("login-hero-badge");
        lb.add_css_class("login-license-badge");
        badges_box.append(&lb);
        lb
    };

    // Assemblage de la section hero
    hero_top.append(&hero_icon);
    hero_top.append(&eyebrow_label);
    hero_top.append(&top_plan_badge);
    hero_box.append(&hero_top);
    hero_box.append(&title_label);
    hero_box.append(&subtitle_label);
    hero_box.append(&badges_box);
    hero_frame.set_child(Some(&hero_box));

    // ─── Section Formulaire ───────────────────────────────────────────────────────
    let form_card = gtk4::Frame::new(None);
    form_card.add_css_class("login-card");

    let form_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(14)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();

    // Sélecteur de langue
    let language_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .halign(Align::Fill)
        .build();

    let language_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-language-label").as_str()));
    language_label.add_css_class("login-field-label");
    language_label.set_halign(Align::Start);
    language_label.set_hexpand(true);

    let language_buttons = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .halign(Align::End)
        .build();

    let language_fr_button = gtk4::ToggleButton::with_label("🇫🇷");
    language_fr_button.add_css_class("login-lang-flag");
    language_fr_button.set_tooltip_text(Some(heelonvault_core::tr!("login-language-fr").as_str()));

    let language_en_button = gtk4::ToggleButton::with_label("🇬🇧");
    language_en_button.add_css_class("login-lang-flag");
    language_en_button.set_tooltip_text(Some(heelonvault_core::tr!("login-language-en").as_str()));
    language_en_button.set_group(Some(&language_fr_button));

    language_buttons.append(&language_fr_button);
    language_buttons.append(&language_en_button);

    // Définir la langue active initiale
    let current_lang = heelonvault_core::i18n::current_language();
    if current_lang.to_ascii_lowercase().starts_with("en") {
        language_en_button.set_active(true);
    } else {
        language_fr_button.set_active(true);
    }

    language_row.append(&language_label);
    language_row.append(&language_buttons);

    // Teaser CPS
    let cps_frame = gtk4::Frame::new(None);
    cps_frame.add_css_class("login-cps-teaser");
    cps_frame.set_sensitive(false);

    let cps_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();

    let cps_image = gtk4::Image::from_resource("/com/heelonvault/rust/images/cps_card.png");
    cps_image.add_css_class("login-cps-image");
    cps_image.set_pixel_size(40);
    cps_image.set_size_request(64, 40);
    cps_image.set_halign(Align::Center);
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
    cps_badge.set_halign(Align::End);
    cps_badge.set_valign(Align::Center);

    cps_info.append(&cps_name);
    cps_info.append(&cps_sub);
    cps_box.append(&cps_image);
    cps_box.append(&cps_info);
    cps_box.append(&cps_badge);
    cps_frame.set_child(Some(&cps_box));

    // Champs de connexion
    let username_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-username-label").as_str()));
    username_label.add_css_class("login-field-label");
    username_label.add_css_class("login-field-label-caps");
    username_label.set_halign(Align::Start);

    let username_entry = gtk4::Entry::builder()
        .placeholder_text(heelonvault_core::tr!("login-username-placeholder").as_str())
        .hexpand(true)
        .build();
    username_entry.add_css_class("login-entry");
    username_entry.set_activates_default(true);

    let password_label = gtk4::Label::new(Some(heelonvault_core::tr!("login-password-label").as_str()));
    password_label.add_css_class("login-field-label");
    password_label.add_css_class("login-field-label-caps");
    password_label.set_halign(Align::Start);

    let password_entry = gtk4::PasswordEntry::builder()
        .placeholder_text(heelonvault_core::tr!("login-password-placeholder").as_str())
        .hexpand(true)
        .show_peek_icon(true)
        .build();
    password_entry.add_css_class("login-entry");
    password_entry.set_activates_default(true);

    let strength_label = gtk4::Label::new(None);
    strength_label.add_css_class("login-strength");
    strength_label.set_halign(Align::Start);
    strength_label.set_visible(false);

    let restore_button = gtk4::Button::with_label(heelonvault_core::tr!("login-restore-button").as_str());
    restore_button.add_css_class("flat");
    restore_button.set_halign(Align::End);

    // Widgets PSC (Premium uniquement)
    #[cfg(feature = "premium")]
    let psc_start_button = gtk4::Button::with_label("Se connecter avec Pro Sante Connect");
    #[cfg(feature = "premium")]
    {
        psc_start_button.add_css_class("suggested-action");
        psc_start_button.set_halign(Align::Fill);
    }

    #[cfg(feature = "premium")]
    let psc_artifact_entry = gtk4::Entry::builder()
        .placeholder_text("Artefact callback PSC")
        .hexpand(true)
        .build();
    #[cfg(feature = "premium")]
    {
        psc_artifact_entry.add_css_class("login-entry");
    }

    #[cfg(feature = "premium")]
    let psc_complete_button = gtk4::Button::with_label("Valider artefact PSC");
    #[cfg(feature = "premium")]
    {
        psc_complete_button.add_css_class("secondary-pill");
    }

    // STEP 1: Credentials form
    let credentials_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(14)
        .build();

    credentials_box.append(&username_label);
    credentials_box.append(&username_entry);
    credentials_box.append(&password_label);
    credentials_box.append(&password_entry);
    credentials_box.append(&strength_label);

    let credentials_step_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(14)
        .build();

    #[cfg(feature = "premium")]
    {
        credentials_step_box.append(&cps_frame);
        credentials_step_box.append(&psc_start_button);
        credentials_step_box.append(&psc_artifact_entry);
        credentials_step_box.append(&psc_complete_button);
    }
    #[cfg(not(feature = "premium"))]
    {
        cps_frame.set_visible(false);
    }

    credentials_step_box.append(&credentials_box);
    credentials_step_box.append(&restore_button);

    // STEP 2: TOTP view
    let totp_step_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    totp_step_box.add_css_class("login-totp-block");

    let totp_back_button = gtk4::Button::with_label(heelonvault_core::tr!("login-totp-back").as_str());
    totp_back_button.add_css_class("flat");
    totp_back_button.set_halign(Align::Start);

    let totp_spacer = Separator::new(Orientation::Horizontal);
    totp_spacer.set_margin_top(8);
    totp_spacer.set_margin_bottom(8);

    let totp_icon = gtk4::Image::from_icon_name("auth-2fa-symbolic");
    totp_icon.set_pixel_size(48);
    totp_icon.set_halign(Align::Center);
    totp_icon.add_css_class("login-totp-icon");

    let totp_title = gtk4::Label::new(Some(heelonvault_core::tr!("login-totp-title").as_str()));
    totp_title.add_css_class("login-field-label");
    totp_title.set_halign(Align::Center);

    let totp_subtitle = gtk4::Label::new(Some(heelonvault_core::tr!("login-totp-subtitle").as_str()));
    totp_subtitle.add_css_class("login-support-copy");
    totp_subtitle.set_wrap(true);
    totp_subtitle.set_justify(Justification::Center);
    totp_subtitle.set_halign(Align::Fill);

    let totp_entry_wrap = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .halign(Align::Center)
        .build();

    let totp_entry = gtk4::Entry::builder()
        .placeholder_text("000000")
        .max_length(6)
        .input_purpose(InputPurpose::Digits)
        .build();
    totp_entry.add_css_class("login-entry");
    totp_entry.add_css_class("login-totp-entry");
    totp_entry.set_halign(Align::Center);
    totp_entry.set_width_chars(8);
    gtk4::prelude::EntryExt::set_alignment(&totp_entry, 0.5);
    totp_entry_wrap.append(&totp_entry);

    totp_step_box.append(&totp_back_button);
    totp_step_box.append(&totp_spacer);
    totp_step_box.append(&totp_icon);
    totp_step_box.append(&totp_title);
    totp_step_box.append(&totp_subtitle);
    totp_step_box.append(&totp_entry_wrap);

    // Stack pour basculer entre les étapes
    let step_stack = Stack::builder()
        .transition_type(StackTransitionType::SlideLeft)
        .build();

    step_stack.add_named(&credentials_step_box, Some("credentials"));
    step_stack.add_named(&totp_step_box, Some("totp"));
    step_stack.set_visible_child_name("credentials");

    // Message d'erreur
    let error_label = gtk4::Label::new(None);
    error_label.add_css_class("login-error");
    error_label.set_wrap(true);
    error_label.set_halign(Align::Start);
    error_label.set_visible(false);

    // Boutons
    let button_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    let back_button = gtk4::Button::with_label(heelonvault_core::tr!("login-back-button").as_str());
    back_button.add_css_class("secondary-pill");
    back_button.set_hexpand(false);

    let submit_button = gtk4::Button::builder()
        .hexpand(true)
        .halign(Align::Fill)
        .build();
    submit_button.add_css_class("primary-pill");

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
    button_label.add_css_class("heading");

    // Mode bootstrap: adapter le texte du bouton
    if in_bootstrap_mode {
        button_label.set_text(heelonvault_core::tr!("init-next-button").as_str());
    }

    button_content.append(&submit_spinner);
    button_content.append(&button_label);
    submit_button.set_child(Some(&button_content));
    button_box.append(&back_button);
    button_box.append(&submit_button);

    // Strip de sécurité
    let sec_strip = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .halign(Align::Center)
        .margin_top(4)
        .build();

    let sec_dot = gtk4::Label::new(Some("·"));
    sec_dot.add_css_class("login-sec-dot");

    let sec_text = gtk4::Label::new(Some(heelonvault_core::tr!("login-security-note").as_str()));
    sec_text.add_css_class("login-support-copy");
    sec_text.set_halign(Align::Center);

    sec_strip.append(&sec_dot);
    sec_strip.append(&sec_text);

    // Assemblage du form_box
    form_box.append(&step_stack);
    form_box.append(&error_label);
    form_box.append(&button_box);
    form_box.append(&sec_strip);
    form_box.prepend(&language_row);

    form_card.set_child(Some(&form_box));

    // ─── Bootstrap Widgets - Identity Step ────────────────────────────────────────
    let init_identity_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(14)
        .build();

    let init_step_label_1 = gtk4::Label::new(Some(heelonvault_core::tr!("init-step-label-identity").as_str()));
    init_step_label_1.add_css_class("dim-label");
    init_step_label_1.set_halign(Align::End);

    let init_sep_1 = Separator::new(Orientation::Horizontal);

    let init_username_label = gtk4::Label::new(Some(heelonvault_core::tr!("init-username-label").as_str()));
    init_username_label.add_css_class("login-field-label");
    init_username_label.add_css_class("login-field-label-caps");
    init_username_label.set_halign(Align::Start);

    let init_username_entry = gtk4::Entry::builder()
        .placeholder_text(heelonvault_core::tr!("init-username-placeholder").as_str())
        .hexpand(true)
        .build();
    init_username_entry.add_css_class("login-entry");

    let init_password_label = gtk4::Label::new(Some(heelonvault_core::tr!("init-password-label").as_str()));
    init_password_label.add_css_class("login-field-label");
    init_password_label.add_css_class("login-field-label-caps");
    init_password_label.set_halign(Align::Start);

    let init_password_entry = gtk4::PasswordEntry::builder()
        .placeholder_text(heelonvault_core::tr!("init-password-placeholder").as_str())
        .hexpand(true)
        .show_peek_icon(true)
        .build();
    init_password_entry.add_css_class("login-entry");

    let init_strength_bar = PasswordStrengthBar::new();

    let init_confirm_label = gtk4::Label::new(Some(heelonvault_core::tr!("init-confirm-label").as_str()));
    init_confirm_label.add_css_class("login-field-label");
    init_confirm_label.add_css_class("login-field-label-caps");
    init_confirm_label.set_halign(Align::Start);

    let init_confirm_entry = gtk4::PasswordEntry::builder()
        .placeholder_text(heelonvault_core::tr!("init-confirm-placeholder").as_str())
        .hexpand(true)
        .show_peek_icon(true)
        .build();
    init_confirm_entry.add_css_class("login-entry");

    init_identity_box.append(&init_step_label_1);
    init_identity_box.append(&init_sep_1);
    init_identity_box.append(&init_username_label);
    init_identity_box.append(&init_username_entry);
    init_identity_box.append(&init_password_label);
    init_identity_box.append(&init_password_entry);
    init_identity_box.append(init_strength_bar.root());
    init_identity_box.append(&init_confirm_label);
    init_identity_box.append(&init_confirm_entry);

    // ─── Bootstrap Widgets - Oath Step ────────────────────────────────────────────
    let init_oath_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .build();

    let init_step_label_2 = gtk4::Label::new(Some(heelonvault_core::tr!("init-step-label-oath").as_str()));
    init_step_label_2.add_css_class("dim-label");
    init_step_label_2.set_halign(Align::End);

    let init_warning_frame = gtk4::Frame::new(None);
    init_warning_frame.add_css_class("init-warning-banner");

    let init_warning_label = gtk4::Label::new(Some(heelonvault_core::tr!("init-oath-warning").as_str()));
    init_warning_label.set_wrap(true);
    init_warning_label.add_css_class("caption");
    init_warning_label.set_margin_top(8);
    init_warning_label.set_margin_bottom(8);
    init_warning_label.set_margin_start(10);
    init_warning_label.set_margin_end(10);
    init_warning_label.set_halign(Align::Start);
    init_warning_frame.set_child(Some(&init_warning_label));

    let init_key_title = gtk4::Label::new(Some(heelonvault_core::tr!("init-recovery-key-title").as_str()));
    init_key_title.add_css_class("login-field-label");
    init_key_title.add_css_class("login-field-label-caps");
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
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
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

    let init_copy_button = gtk4::Button::with_label(heelonvault_core::tr!("init-copy-button").as_str());
    init_copy_button.add_css_class("flat");
    init_copy_button.set_halign(Align::End);

    let init_verify_hint_label = gtk4::Label::new(None);
    init_verify_hint_label.add_css_class("login-support-copy");
    init_verify_hint_label.set_wrap(true);
    init_verify_hint_label.set_halign(Align::Start);

    let init_verify_a_label = gtk4::Label::new(None);
    init_verify_a_label.add_css_class("login-field-label");
    init_verify_a_label.set_halign(Align::Start);

    let init_verify_a_entry = gtk4::Entry::builder()
        .placeholder_text(heelonvault_core::tr!("init-verify-placeholder").as_str())
        .hexpand(true)
        .build();
    init_verify_a_entry.add_css_class("login-entry");

    let init_verify_b_label = gtk4::Label::new(None);
    init_verify_b_label.add_css_class("login-field-label");
    init_verify_b_label.set_halign(Align::Start);

    let init_verify_b_entry = gtk4::Entry::builder()
        .placeholder_text(heelonvault_core::tr!("init-verify-placeholder").as_str())
        .hexpand(true)
        .build();
    init_verify_b_entry.add_css_class("login-entry");

    let verify_col_a = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .build();
    verify_col_a.append(&init_verify_a_label);
    verify_col_a.append(&init_verify_a_entry);

    let verify_col_b = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .build();
    verify_col_b.append(&init_verify_b_label);
    verify_col_b.append(&init_verify_b_entry);

    let verify_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
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

    let init_pending_label = gtk4::Label::new(Some(heelonvault_core::tr!("init-progress-label").as_str()));
    init_pending_label.add_css_class("login-support-copy");
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
    clamp.set_maximum_size(560);
    clamp.set_tightening_threshold(420);

    root.append(&hero_frame);
    root.append(&form_card);

    clamp.set_child(Some(&root));
    shell.append(&clamp);

    let container = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .min_content_width(360)
        .build();
    container.set_child(Some(&shell));

    // ─── Return structure ────────────────────────────────────────────────────────
    LoginDialogWidgets {
        container,
        shell,
        root,
        hero_frame,
        hero_box,
        hero_top,
        hero_icon,
        eyebrow_label,
        top_plan_badge,
        title_label,
        subtitle_label,
        badges_box,
        #[cfg(feature = "premium")]
        license_seal,
        license_badge,
        form_card,
        form_box,
        language_row,
        language_label,
        language_buttons,
        language_fr_button,
        language_en_button,
        cps_frame,
        cps_box,
        cps_image,
        cps_info,
        cps_name,
        cps_sub,
        cps_badge,
        username_label,
        username_entry,
        password_label,
        password_entry,
        strength_label,
        restore_button,
        #[cfg(feature = "premium")]
        psc_start_button,
        #[cfg(feature = "premium")]
        psc_artifact_entry,
        #[cfg(feature = "premium")]
        psc_complete_button,
        credentials_box,
        credentials_step_box,
        totp_step_box,
        totp_back_button,
        totp_spacer,
        totp_icon,
        totp_title,
        totp_subtitle,
        totp_entry_wrap,
        totp_entry,
        step_stack,
        error_label,
        button_box,
        back_button,
        submit_button,
        button_content,
        submit_spinner,
        button_label,
        sec_strip,
        sec_dot,
        sec_text,
        clamp,
        init_identity_box,
        init_step_label_1,
        init_sep_1,
        init_username_label,
        init_username_entry,
        init_password_label,
        init_password_entry,
        init_strength_bar,
        init_confirm_label,
        init_confirm_entry,
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

/// Extraire le nom du client professionnel du texte du badge de licence.
/// Format attendu: "Licence pro - NomClient" ou similaire.
#[cfg(feature = "premium")]
fn professional_customer_name(license_badge_text: &str) -> Option<String> {
    let text = license_badge_text.trim();
    if text.starts_with("Licence pro") && text.contains('-') {
        let parts: Vec<&str> = text.splitn(2, '-').collect();
        if parts.len() == 2 {
            let customer = parts[1].trim();
            if !customer.is_empty() {
                return Some(customer.to_string());
            }
        }
    }
    None
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
                widgets_clone.title_label.set_text(heelonvault_core::tr!("init-hero-title").as_str());
                widgets_clone.subtitle_label.set_text(heelonvault_core::tr!("init-hero-subtitle").as_str());
                widgets_clone.button_label.set_text(heelonvault_core::tr!("init-next-button").as_str());
            } else {
                widgets_clone.title_label.set_text(heelonvault_core::tr!("login-hero-title").as_str());
                widgets_clone.subtitle_label.set_text(heelonvault_core::tr!("login-hero-subtitle").as_str());
                widgets_clone.button_label.set_text(heelonvault_core::tr!("login-button").as_str());
            }

            // Mettre à jour les textes communs
            widgets_clone.cps_name.set_text(heelonvault_core::tr!("login-cps-name").as_str());
            widgets_clone.cps_sub.set_text(heelonvault_core::tr!("login-cps-subtitle").as_str());
            widgets_clone.cps_badge.set_text(heelonvault_core::tr!("login-cps-badge").as_str());
            widgets_clone.username_label.set_text(heelonvault_core::tr!("login-username-label").as_str());
            widgets_clone.username_entry.set_placeholder_text(Some(heelonvault_core::tr!("login-username-placeholder").as_str()));
            widgets_clone.password_label.set_text(heelonvault_core::tr!("login-password-label").as_str());
            widgets_clone.password_entry.set_placeholder_text(Some(heelonvault_core::tr!("login-password-placeholder").as_str()));
            widgets_clone.restore_button.set_label(heelonvault_core::tr!("login-restore-button").as_str());
            widgets_clone.totp_back_button.set_label(heelonvault_core::tr!("login-totp-back").as_str());
            widgets_clone.totp_title.set_text(heelonvault_core::tr!("login-totp-title").as_str());
            widgets_clone.totp_subtitle.set_text(heelonvault_core::tr!("login-totp-subtitle").as_str());
            widgets_clone.back_button.set_label(heelonvault_core::tr!("login-back-button").as_str());
            widgets_clone.sec_text.set_text(heelonvault_core::tr!("login-security-note").as_str());
            widgets_clone.language_label.set_text(heelonvault_core::tr!("login-language-label").as_str());
            widgets_clone.language_fr_button.set_tooltip_text(Some(heelonvault_core::tr!("login-language-fr").as_str()));
            widgets_clone.language_en_button.set_tooltip_text(Some(heelonvault_core::tr!("login-language-en").as_str()));

            // Bootstrap widgets (même s'ils ne sont pas visibles, on met à jour pour quand ils le seront)
            widgets_clone.init_step_label_1.set_text(heelonvault_core::tr!("init-step-label-identity").as_str());
            widgets_clone.init_username_label.set_text(heelonvault_core::tr!("init-username-label").as_str());
            widgets_clone.init_username_entry.set_placeholder_text(Some(heelonvault_core::tr!("init-username-placeholder").as_str()));
            widgets_clone.init_password_label.set_text(heelonvault_core::tr!("init-password-label").as_str());
            widgets_clone.init_password_entry.set_placeholder_text(Some(heelonvault_core::tr!("init-password-placeholder").as_str()));
            widgets_clone.init_confirm_label.set_text(heelonvault_core::tr!("init-confirm-label").as_str());
            widgets_clone.init_confirm_entry.set_placeholder_text(Some(heelonvault_core::tr!("init-confirm-placeholder").as_str()));
            widgets_clone.init_step_label_2.set_text(heelonvault_core::tr!("init-step-label-oath").as_str());
            widgets_clone.init_warning_label.set_text(heelonvault_core::tr!("init-oath-warning").as_str());
            widgets_clone.init_key_title.set_text(heelonvault_core::tr!("init-recovery-key-title").as_str());
            widgets_clone.init_copy_button.set_label(heelonvault_core::tr!("init-copy-button").as_str());
            widgets_clone.init_verify_hint_label.set_text(heelonvault_core::tr!("init-oath-warning").as_str());
            widgets_clone.init_verify_a_entry.set_placeholder_text(Some(heelonvault_core::tr!("init-verify-placeholder").as_str()));
            widgets_clone.init_verify_b_entry.set_placeholder_text(Some(heelonvault_core::tr!("init-verify-placeholder").as_str()));
            widgets_clone.init_pending_label.set_text(heelonvault_core::tr!("init-progress-label").as_str());

            // Définir la langue active initiale sur les boutons
            let current_lang = heelonvault_core::i18n::current_language();
            let active_is_en = current_lang.to_ascii_lowercase().starts_with("en");
            if active_is_en {
                widgets_clone.language_en_button.set_active(true);
            } else {
                widgets_clone.language_fr_button.set_active(true);
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
                    button_label_clone.set_text(heelonvault_core::tr!("login-button-verify").as_str());
                }
                "init-identity" => {
                    button_label_clone.set_text(heelonvault_core::tr!("init-next-button").as_str());
                }
                "init-oath" => {
                    button_label_clone.set_text(heelonvault_core::tr!("init-confirm-button").as_str());
                }
                "init-pending" => {}
                _ => {
                    if in_bootstrap {
                        button_label_clone.set_text(heelonvault_core::tr!("init-next-button").as_str());
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
        fr_button.connect_toggled(move |button| {
            if guard.get() || !button.is_active() {
                return;
            }
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
        en_button.connect_toggled(move |button| {
            if guard.get() || !button.is_active() {
                return;
            }
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
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use gtk4::glib;

    // Connecter la strength bar à l'entry password pour qu'elle mette à jour son score
    widgets.init_strength_bar.connect_to_password_entry(&widgets.init_password_entry);

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
            
            let (ia, ib) = match (parse_index_from_label(ia_str), parse_index_from_label(ib_str)) {
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
    // État pour suivre si le presse-papier contient la phrase (nettoyage auto après 60s)
    let init_clipboard_dirty: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let init_clipboard_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    
    {
        let word_labels_for_copy = widgets.word_labels.clone();
        let dirty_for_copy = Rc::clone(&init_clipboard_dirty);
        let timer_for_copy = Rc::clone(&init_clipboard_timer);
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
            
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&phrase);
                dirty_for_copy.set(true);
                
                // Nettoyer le presse-papier après 60 secondes
                if let Some(id) = timer_for_copy.borrow_mut().take() {
                    id.remove();
                }
                
                let dirty_for_timer = Rc::clone(&dirty_for_copy);
                let id = glib::timeout_add_seconds_local(60, move || {
                    if let Some(disp) = gtk4::gdk::Display::default() {
                        disp.clipboard().set_text("");
                    }
                    dirty_for_timer.set(false);
                    glib::ControlFlow::Break
                });
                *timer_for_copy.borrow_mut() = Some(id);
            }
        });
    }

    // ─── Exécuter la vérification initiale des gates ───────────────────────────────
    // Déclencher la gate identity une première fois pour initialiser l'état du bouton
    check_init_identity_gate();
}
