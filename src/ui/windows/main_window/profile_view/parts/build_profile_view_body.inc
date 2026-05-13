{
    let container = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    content.add_css_class("profile-view-content");

    let header = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    header.add_css_class("profile-view-header");
    let back_button = gtk4::Button::with_label(crate::tr!("profile-view-back").as_str());
    back_button.add_css_class("flat");
    back_button.set_halign(Align::Start);
    let title = gtk4::Label::new(Some(crate::tr!("profile-view-title").as_str()));
    title.add_css_class("title-3");
    title.add_css_class("heading");
    title.set_hexpand(true);
    title.set_halign(Align::Center);
    header.append(&back_button);
    header.append(&title);
    content.append(&header);

    let profile_intro = gtk4::Label::new(Some(crate::tr!("profile-view-intro").as_str()));
    profile_intro.set_halign(Align::Start);
    profile_intro.set_wrap(true);
    profile_intro.add_css_class("dim-label");
    content.append(&profile_intro);

    let sections_columns = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(18)
        .hexpand(true)
        .homogeneous(true)
        .build();
    sections_columns.add_css_class("profile-sections-columns");

    let sections_left = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .hexpand(true)
        .build();
    sections_left.add_css_class("profile-sections-column");

    let sections_right = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .hexpand(true)
        .build();
    sections_right.add_css_class("profile-sections-column");

    sections_columns.append(&sections_left);
    sections_columns.append(&sections_right);
    content.append(&sections_columns);

    let info_frame = gtk4::Frame::builder()
        .label(crate::tr!("profile-section-info").as_str())
        .build();
    info_frame.add_css_class("profile-section-frame");
    let info_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let info_subtitle =
        gtk4::Label::new(Some(crate::tr!("profile-section-info-subtitle").as_str()));
    info_subtitle.set_halign(Align::Start);
    info_subtitle.set_wrap(true);
    info_subtitle.add_css_class("profile-section-subtitle");
    info_subtitle.add_css_class("dim-label");
    info_box.append(&info_subtitle);
    let username_label = gtk4::Label::new(Some(crate::tr!("profile-field-username").as_str()));
    username_label.set_halign(Align::Start);
    username_label.add_css_class("profile-field-label");
    let username_entry = gtk4::Entry::new();
    username_entry.set_sensitive(false);
    username_entry.set_hexpand(true);
    username_entry.add_css_class("profile-field-entry");
    let display_label =
        gtk4::Label::new(Some(crate::tr!("profile-field-display-name").as_str()));
    display_label.set_halign(Align::Start);
    display_label.add_css_class("profile-field-label");
    let display_entry = gtk4::Entry::new();
    display_entry.set_hexpand(true);
    display_entry.add_css_class("profile-field-entry");
    let email_label = gtk4::Label::new(Some(crate::tr!("profile-field-email").as_str()));
    email_label.set_halign(Align::Start);
    email_label.add_css_class("profile-field-label");
    let email_entry = gtk4::Entry::new();
    email_entry.set_hexpand(true);
    email_entry.add_css_class("profile-field-entry");
    let language_label = gtk4::Label::new(Some(crate::tr!("profile-language-title").as_str()));
    language_label.set_halign(Align::Start);
    language_label.add_css_class("profile-field-label");
    let language_hint =
        gtk4::Label::new(Some(crate::tr!("profile-language-subtitle").as_str()));
    language_hint.set_halign(Align::Start);
    language_hint.set_wrap(true);
    language_hint.add_css_class("dim-label");
    language_hint.add_css_class("profile-section-subtitle");
    let language_items = gtk4::StringList::new(&[
        crate::tr!("language-option-fr").as_str(),
        crate::tr!("language-option-en").as_str(),
    ]);
    let language_dropdown =
        gtk4::DropDown::new(Some(language_items.clone()), None::<gtk4::Expression>);
    language_dropdown.add_css_class("profile-field-entry");
    language_dropdown.set_selected(0);
    let current_email_pw_label = gtk4::Label::new(Some(
        crate::tr!("profile-field-current-password-email-change").as_str(),
    ));
    current_email_pw_label.set_halign(Align::Start);
    current_email_pw_label.add_css_class("profile-field-label");
    let current_email_pw_entry = gtk4::PasswordEntry::new();
    current_email_pw_entry.set_hexpand(true);
    current_email_pw_entry.add_css_class("profile-field-entry");
    let profile_status_label = gtk4::Label::new(None);
    profile_status_label.set_halign(Align::Start);
    profile_status_label.set_wrap(true);
    profile_status_label.add_css_class("inline-status");
    profile_status_label.add_css_class("profile-inline-status");
    profile_status_label.set_visible(false);
    let save_profile_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .hexpand(true)
        .halign(Align::End)
        .build();
    save_profile_row.add_css_class("profile-actions-row");
    let save_profile_button = gtk4::Button::with_label(crate::tr!("profile-save").as_str());
    save_profile_button.add_css_class("suggested-action");
    save_profile_button.add_css_class("profile-action-btn");
    save_profile_row.append(&save_profile_button);

    for widget in [
        username_label.upcast_ref::<gtk4::Widget>(),
        username_entry.upcast_ref::<gtk4::Widget>(),
        display_label.upcast_ref::<gtk4::Widget>(),
        display_entry.upcast_ref::<gtk4::Widget>(),
        email_label.upcast_ref::<gtk4::Widget>(),
        email_entry.upcast_ref::<gtk4::Widget>(),
        language_label.upcast_ref::<gtk4::Widget>(),
        language_dropdown.upcast_ref::<gtk4::Widget>(),
        language_hint.upcast_ref::<gtk4::Widget>(),
        current_email_pw_label.upcast_ref::<gtk4::Widget>(),
        current_email_pw_entry.upcast_ref::<gtk4::Widget>(),
        profile_status_label.upcast_ref::<gtk4::Widget>(),
        save_profile_row.upcast_ref::<gtk4::Widget>(),
    ] {
        info_box.append(widget);
    }
    info_frame.set_child(Some(&info_box));
    info_frame.set_hexpand(true);
    sections_left.append(&info_frame);

    let password_change_frame = gtk4::Frame::builder()
        .label(crate::tr!("profile-section-password-change").as_str())
        .build();
    password_change_frame.add_css_class("profile-section-frame");
    let password_change_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let password_change_subtitle = gtk4::Label::new(Some(
        crate::tr!("profile-section-password-change-subtitle").as_str(),
    ));
    password_change_subtitle.set_halign(Align::Start);
    password_change_subtitle.set_wrap(true);
    password_change_subtitle.add_css_class("profile-section-subtitle");
    password_change_subtitle.add_css_class("dim-label");
    password_change_box.append(&password_change_subtitle);

    let current_pw_label =
        gtk4::Label::new(Some(crate::tr!("profile-field-current-password").as_str()));
    current_pw_label.set_halign(Align::Start);
    current_pw_label.add_css_class("profile-field-label");
    let current_pw_entry = gtk4::PasswordEntry::new();
    current_pw_entry.set_hexpand(true);
    current_pw_entry.add_css_class("profile-field-entry");

    let new_pw_label =
        gtk4::Label::new(Some(crate::tr!("profile-field-new-password").as_str()));
    new_pw_label.set_halign(Align::Start);
    new_pw_label.add_css_class("profile-field-label");
    let new_pw_entry = gtk4::PasswordEntry::new();
    new_pw_entry.set_hexpand(true);
    new_pw_entry.add_css_class("profile-field-entry");

    let confirm_pw_label = gtk4::Label::new(Some(
        crate::tr!("profile-field-confirm-new-password").as_str(),
    ));
    confirm_pw_label.set_halign(Align::Start);
    confirm_pw_label.add_css_class("profile-field-label");
    let confirm_pw_entry = gtk4::PasswordEntry::new();
    confirm_pw_entry.set_hexpand(true);
    confirm_pw_entry.add_css_class("profile-field-entry");

    let password_change_status_label = gtk4::Label::new(None);
    password_change_status_label.set_halign(Align::Start);
    password_change_status_label.set_wrap(true);
    password_change_status_label.add_css_class("inline-status");
    password_change_status_label.add_css_class("profile-inline-status");
    password_change_status_label.set_visible(false);

    let security_actions = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .hexpand(true)
        .halign(Align::End)
        .build();
    security_actions.add_css_class("profile-actions-row");
    let change_pw_button =
        gtk4::Button::with_label(crate::tr!("profile-change-button").as_str());
    change_pw_button.add_css_class("suggested-action");
    change_pw_button.add_css_class("profile-action-btn");
    change_pw_button
        .set_tooltip_text(Some(crate::tr!("profile-change-button-tooltip").as_str()));
    let rotate_master_key_button =
        gtk4::Button::with_label(crate::tr!("profile-rotate-button").as_str());
    rotate_master_key_button.add_css_class("suggested-action");
    rotate_master_key_button.add_css_class("profile-action-btn");
    rotate_master_key_button
        .set_tooltip_text(Some(crate::tr!("profile-rotate-button-tooltip").as_str()));
    security_actions.append(&change_pw_button);
    security_actions.append(&rotate_master_key_button);

    for widget in [
        current_pw_label.upcast_ref::<gtk4::Widget>(),
        current_pw_entry.upcast_ref::<gtk4::Widget>(),
        new_pw_label.upcast_ref::<gtk4::Widget>(),
        new_pw_entry.upcast_ref::<gtk4::Widget>(),
        confirm_pw_label.upcast_ref::<gtk4::Widget>(),
        confirm_pw_entry.upcast_ref::<gtk4::Widget>(),
        password_change_status_label.upcast_ref::<gtk4::Widget>(),
        security_actions.upcast_ref::<gtk4::Widget>(),
    ] {
        password_change_box.append(widget);
    }
    password_change_frame.set_child(Some(&password_change_box));
    password_change_frame.set_hexpand(true);
    sections_right.append(&password_change_frame);

    let security_prefs_frame = gtk4::Frame::builder()
        .label(crate::tr!("profile-section-security-prefs").as_str())
        .build();
    security_prefs_frame.add_css_class("profile-section-frame");
    let security_prefs_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let auto_lock_label =
        gtk4::Label::new(Some(crate::tr!("profile-auto-lock-title").as_str()));
    auto_lock_label.set_halign(Align::Start);
    auto_lock_label.add_css_class("profile-field-label");
    let auto_lock_items = gtk4::StringList::new(&[
        crate::tr!("profile-auto-lock-1").as_str(),
        crate::tr!("profile-auto-lock-5").as_str(),
        crate::tr!("profile-auto-lock-15").as_str(),
        crate::tr!("profile-auto-lock-30").as_str(),
        crate::tr!("profile-auto-lock-never").as_str(),
    ]);
    let auto_lock_dropdown =
        gtk4::DropDown::new(Some(auto_lock_items.clone()), None::<gtk4::Expression>);
    auto_lock_dropdown.add_css_class("profile-field-entry");

    let show_edit_passwords_label = gtk4::Label::new(Some(
        crate::tr!("profile-show-edit-passwords-title").as_str(),
    ));
    show_edit_passwords_label.set_halign(Align::Start);
    show_edit_passwords_label.add_css_class("profile-field-label");
    let show_edit_passwords_hint = gtk4::Label::new(Some(
        crate::tr!("profile-show-edit-passwords-hint").as_str(),
    ));
    show_edit_passwords_hint.set_halign(Align::Start);
    show_edit_passwords_hint.set_wrap(true);
    show_edit_passwords_hint.add_css_class("dim-label");
    show_edit_passwords_hint.add_css_class("profile-section-subtitle");
    let show_edit_passwords_switch = gtk4::Switch::new();
    show_edit_passwords_switch.set_halign(Align::Start);

    let security_prefs_status_label = gtk4::Label::new(None);
    security_prefs_status_label.set_halign(Align::Start);
    security_prefs_status_label.set_wrap(true);
    security_prefs_status_label.add_css_class("inline-status");
    security_prefs_status_label.add_css_class("profile-inline-status");
    security_prefs_status_label.set_visible(false);

    for widget in [
        auto_lock_label.upcast_ref::<gtk4::Widget>(),
        auto_lock_dropdown.upcast_ref::<gtk4::Widget>(),
        show_edit_passwords_label.upcast_ref::<gtk4::Widget>(),
        show_edit_passwords_switch.upcast_ref::<gtk4::Widget>(),
        show_edit_passwords_hint.upcast_ref::<gtk4::Widget>(),
        security_prefs_status_label.upcast_ref::<gtk4::Widget>(),
    ] {
        security_prefs_box.append(widget);
    }
    security_prefs_frame.set_child(Some(&security_prefs_box));
    security_prefs_frame.set_hexpand(true);
    sections_right.append(&security_prefs_frame);

    let twofa_frame = gtk4::Frame::new(None);
    twofa_frame.add_css_class("profile-section-frame");
    let twofa_header = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();
    let twofa_title = gtk4::Label::new(Some(crate::tr!("profile-section-2fa-title").as_str()));
    twofa_title.add_css_class("profile-field-label");
    twofa_title.set_halign(Align::Start);
    let twofa_badge_text = messages::twofa_badge_disabled();
    let twofa_state_badge = gtk4::Label::new(Some(twofa_badge_text.as_str()));
    twofa_state_badge.add_css_class("status-role-pill");
    twofa_state_badge.add_css_class("status-role-user");
    twofa_state_badge.set_halign(Align::Start);
    twofa_header.append(&twofa_title);
    twofa_header.append(&twofa_state_badge);
    twofa_frame.set_label_widget(Some(&twofa_header));
    MainWindow::set_twofa_badge_state(&twofa_state_badge, false);
    let twofa_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let twofa_stack = gtk4::Stack::builder()
        .hexpand(true)
        .transition_type(gtk4::StackTransitionType::Crossfade)
        .build();

    let twofa_disabled_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    let twofa_disabled_copy =
        gtk4::Label::new(Some(crate::tr!("profile-2fa-disabled-copy").as_str()));
    twofa_disabled_copy.set_halign(Align::Start);
    twofa_disabled_copy.set_wrap(true);
    twofa_disabled_copy.add_css_class("dim-label");
    twofa_disabled_copy.add_css_class("profile-section-subtitle");
    let twofa_activate_button =
        gtk4::Button::with_label(crate::tr!("profile-2fa-activate").as_str());
    twofa_activate_button.add_css_class("suggested-action");
    twofa_activate_button.add_css_class("profile-action-btn");
    twofa_activate_button.set_halign(Align::Start);
    twofa_disabled_box.append(&twofa_disabled_copy);
    twofa_disabled_box.append(&twofa_activate_button);

    let twofa_setup_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    let twofa_setup_copy =
        gtk4::Label::new(Some(crate::tr!("profile-2fa-setup-copy").as_str()));
    twofa_setup_copy.set_halign(Align::Start);
    twofa_setup_copy.set_wrap(true);
    twofa_setup_copy.add_css_class("dim-label");
    twofa_setup_copy.add_css_class("profile-section-subtitle");
    let twofa_qr_picture = gtk4::Picture::new();
    twofa_qr_picture.set_size_request(200, 200);
    twofa_qr_picture.add_css_class("totp-qr-container");
    let twofa_secret_label =
        gtk4::Label::new(Some(crate::tr!("profile-2fa-secret-base32").as_str()));
    twofa_secret_label.set_halign(Align::Start);
    twofa_secret_label.add_css_class("profile-field-label");
    let twofa_secret_entry = gtk4::Entry::new();
    twofa_secret_entry.set_editable(false);
    twofa_secret_entry.add_css_class("profile-field-entry");
    let twofa_code_label = gtk4::Label::new(Some(crate::tr!("profile-2fa-code").as_str()));
    twofa_code_label.set_halign(Align::Start);
    twofa_code_label.add_css_class("profile-field-label");
    let twofa_code_entry = gtk4::Entry::new();
    twofa_code_entry.set_input_purpose(gtk4::InputPurpose::Digits);
    twofa_code_entry.set_max_length(6);
    twofa_code_entry.add_css_class("profile-field-entry");
    let twofa_setup_actions = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();
    twofa_setup_actions.add_css_class("profile-actions-row");
    let twofa_confirm_button =
        gtk4::Button::with_label(crate::tr!("profile-2fa-confirm").as_str());
    twofa_confirm_button.add_css_class("suggested-action");
    twofa_confirm_button.add_css_class("profile-action-btn");
    let twofa_cancel_setup_button =
        gtk4::Button::with_label(crate::tr!("profile-2fa-cancel").as_str());
    twofa_cancel_setup_button.add_css_class("flat");
    twofa_cancel_setup_button.add_css_class("profile-action-btn");
    twofa_setup_actions.append(&twofa_confirm_button);
    twofa_setup_actions.append(&twofa_cancel_setup_button);
    for widget in [
        twofa_setup_copy.upcast_ref::<gtk4::Widget>(),
        twofa_qr_picture.upcast_ref::<gtk4::Widget>(),
        twofa_secret_label.upcast_ref::<gtk4::Widget>(),
        twofa_secret_entry.upcast_ref::<gtk4::Widget>(),
        twofa_code_label.upcast_ref::<gtk4::Widget>(),
        twofa_code_entry.upcast_ref::<gtk4::Widget>(),
        twofa_setup_actions.upcast_ref::<gtk4::Widget>(),
    ] {
        twofa_setup_box.append(widget);
    }

    let twofa_enabled_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .build();
    let twofa_enabled_copy =
        gtk4::Label::new(Some(crate::tr!("profile-2fa-enabled-copy").as_str()));
    twofa_enabled_copy.set_halign(Align::Start);
    twofa_enabled_copy.set_wrap(true);
    twofa_enabled_copy.add_css_class("dim-label");
    twofa_enabled_copy.add_css_class("profile-section-subtitle");
    let twofa_disable_toggle_button =
        gtk4::Button::with_label(crate::tr!("profile-2fa-disable").as_str());
    twofa_disable_toggle_button.add_css_class("flat");
    twofa_disable_toggle_button.add_css_class("profile-action-btn");
    twofa_disable_toggle_button.set_halign(Align::Start);
    let twofa_disable_confirm_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .visible(false)
        .build();
    twofa_disable_confirm_row.add_css_class("profile-actions-row");
    let twofa_disable_confirm_button =
        gtk4::Button::with_label(crate::tr!("profile-2fa-disable-confirm").as_str());
    twofa_disable_confirm_button.add_css_class("suggested-action");
    twofa_disable_confirm_button.add_css_class("profile-action-btn");
    let twofa_disable_cancel_button =
        gtk4::Button::with_label(crate::tr!("profile-2fa-cancel").as_str());
    twofa_disable_cancel_button.add_css_class("flat");
    twofa_disable_cancel_button.add_css_class("profile-action-btn");
    twofa_disable_confirm_row.append(&twofa_disable_confirm_button);
    twofa_disable_confirm_row.append(&twofa_disable_cancel_button);
    twofa_enabled_box.append(&twofa_enabled_copy);
    twofa_enabled_box.append(&twofa_disable_toggle_button);
    twofa_enabled_box.append(&twofa_disable_confirm_row);

    twofa_stack.add_titled(
        &twofa_disabled_box,
        Some("disabled"),
        crate::tr!("profile-2fa-stack-disabled").as_str(),
    );
    twofa_stack.add_titled(
        &twofa_setup_box,
        Some("setup"),
        crate::tr!("profile-2fa-stack-setup").as_str(),
    );
    twofa_stack.add_titled(
        &twofa_enabled_box,
        Some("enabled"),
        crate::tr!("profile-2fa-stack-enabled").as_str(),
    );
    twofa_stack.set_visible_child_name("disabled");

    let twofa_status_label = gtk4::Label::new(None);
    twofa_status_label.set_halign(Align::Start);
    twofa_status_label.set_wrap(true);
    twofa_status_label.add_css_class("inline-status");
    twofa_status_label.add_css_class("profile-inline-status");
    twofa_status_label.set_visible(false);

    twofa_box.append(&twofa_stack);
    twofa_box.append(&twofa_status_label);
    twofa_frame.set_child(Some(&twofa_box));
    twofa_frame.set_hexpand(true);
    sections_right.append(&twofa_frame);

    let data_frame = gtk4::Frame::builder()
        .label(crate::tr!("profile-section-data").as_str())
        .build();
    data_frame.add_css_class("profile-section-frame");
    let data_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    let export_title = gtk4::Label::new(Some(crate::tr!("profile-export-title").as_str()));
    export_title.set_halign(Align::Start);
    export_title.add_css_class("heading");
    export_title.add_css_class("profile-field-label");
    let export_subtitle =
        gtk4::Label::new(Some(crate::tr!("profile-export-subtitle").as_str()));
    export_subtitle.set_halign(Align::Start);
    export_subtitle.add_css_class("dim-label");
    export_subtitle.add_css_class("profile-section-subtitle");
    export_subtitle.set_wrap(true);
    let export_button = gtk4::Button::with_label(crate::tr!("profile-export-button").as_str());
    export_button.add_css_class("suggested-action");
    export_button.add_css_class("profile-action-btn");
    export_button.set_halign(Align::End);

    let import_title = gtk4::Label::new(Some(crate::tr!("profile-import-title").as_str()));
    import_title.set_halign(Align::Start);
    import_title.add_css_class("heading");
    import_title.add_css_class("profile-field-label");
    let import_subtitle =
        gtk4::Label::new(Some(crate::tr!("profile-import-subtitle").as_str()));
    import_subtitle.set_halign(Align::Start);
    import_subtitle.add_css_class("dim-label");
    import_subtitle.add_css_class("profile-section-subtitle");
    import_subtitle.set_wrap(true);
    let import_button = gtk4::Button::with_label(crate::tr!("profile-import-button").as_str());
    import_button.add_css_class("suggested-action");
    import_button.add_css_class("profile-action-btn");
    import_button.set_halign(Align::End);

    for widget in [
        export_title.upcast_ref::<gtk4::Widget>(),
        export_subtitle.upcast_ref::<gtk4::Widget>(),
        export_button.upcast_ref::<gtk4::Widget>(),
        import_title.upcast_ref::<gtk4::Widget>(),
        import_subtitle.upcast_ref::<gtk4::Widget>(),
        import_button.upcast_ref::<gtk4::Widget>(),
    ] {
        data_box.append(widget);
    }
    data_frame.set_child(Some(&data_box));
    data_frame.set_hexpand(true);
    sections_left.append(&data_frame);

    container.set_child(Some(&content));

    let back_button_for_i18n = back_button.clone();
    let title_for_i18n = title.clone();
    let intro_for_i18n = profile_intro.clone();
    let info_frame_for_i18n = info_frame.clone();
    let info_subtitle_for_i18n = info_subtitle.clone();
    let username_label_for_i18n = username_label.clone();
    let display_label_for_i18n = display_label.clone();
    let email_label_for_i18n = email_label.clone();
    let language_label_for_i18n = language_label.clone();
    let language_hint_for_i18n = language_hint.clone();
    let language_dropdown_for_i18n = language_dropdown.clone();
    let language_items_for_i18n = language_items.clone();
    let current_email_pw_label_for_i18n = current_email_pw_label.clone();
    let save_profile_button_for_i18n = save_profile_button.clone();

    let password_change_frame_for_i18n = password_change_frame.clone();
    let password_change_subtitle_for_i18n = password_change_subtitle.clone();
    let current_pw_label_for_i18n = current_pw_label.clone();
    let new_pw_label_for_i18n = new_pw_label.clone();
    let confirm_pw_label_for_i18n = confirm_pw_label.clone();
    let change_pw_button_for_i18n = change_pw_button.clone();
    let rotate_master_key_button_for_i18n = rotate_master_key_button.clone();

    let security_prefs_frame_for_i18n = security_prefs_frame.clone();
    let auto_lock_label_for_i18n = auto_lock_label.clone();
    let auto_lock_dropdown_for_i18n = auto_lock_dropdown.clone();
    let auto_lock_items_for_i18n = auto_lock_items.clone();
    let show_edit_passwords_label_for_i18n = show_edit_passwords_label.clone();
    let show_edit_passwords_hint_for_i18n = show_edit_passwords_hint.clone();

    let twofa_title_for_i18n = twofa_title.clone();
    let twofa_disabled_copy_for_i18n = twofa_disabled_copy.clone();
    let twofa_activate_button_for_i18n = twofa_activate_button.clone();
    let twofa_setup_copy_for_i18n = twofa_setup_copy.clone();
    let twofa_secret_label_for_i18n = twofa_secret_label.clone();
    let twofa_code_label_for_i18n = twofa_code_label.clone();
    let twofa_confirm_button_for_i18n = twofa_confirm_button.clone();
    let twofa_cancel_setup_button_for_i18n = twofa_cancel_setup_button.clone();
    let twofa_enabled_copy_for_i18n = twofa_enabled_copy.clone();
    let twofa_disable_toggle_button_for_i18n = twofa_disable_toggle_button.clone();
    let twofa_disable_confirm_button_for_i18n = twofa_disable_confirm_button.clone();
    let twofa_disable_cancel_button_for_i18n = twofa_disable_cancel_button.clone();
    let twofa_stack_for_i18n = twofa_stack.clone();
    let twofa_disabled_box_for_i18n = twofa_disabled_box.clone();
    let twofa_setup_box_for_i18n = twofa_setup_box.clone();
    let twofa_enabled_box_for_i18n = twofa_enabled_box.clone();
    let twofa_badge_for_i18n = twofa_state_badge.clone();

    let data_frame_for_i18n = data_frame.clone();
    let export_title_for_i18n = export_title.clone();
    let export_subtitle_for_i18n = export_subtitle.clone();
    let export_button_for_i18n = export_button.clone();
    let import_title_for_i18n = import_title.clone();
    let import_subtitle_for_i18n = import_subtitle.clone();
    let import_button_for_i18n = import_button.clone();

    let on_language_changed_for_i18n = Rc::clone(&on_language_changed);
    let apply_profile_i18n: Rc<dyn Fn()> = Rc::new(move || {
        back_button_for_i18n.set_label(crate::tr!("profile-view-back").as_str());
        title_for_i18n.set_text(crate::tr!("profile-view-title").as_str());
        intro_for_i18n.set_text(crate::tr!("profile-view-intro").as_str());

        info_frame_for_i18n.set_label(Some(crate::tr!("profile-section-info").as_str()));
        info_subtitle_for_i18n.set_text(crate::tr!("profile-section-info-subtitle").as_str());
        username_label_for_i18n.set_text(crate::tr!("profile-field-username").as_str());
        display_label_for_i18n.set_text(crate::tr!("profile-field-display-name").as_str());
        email_label_for_i18n.set_text(crate::tr!("profile-field-email").as_str());
        language_label_for_i18n.set_text(crate::tr!("profile-language-title").as_str());
        language_hint_for_i18n.set_text(crate::tr!("profile-language-subtitle").as_str());
        current_email_pw_label_for_i18n
            .set_text(crate::tr!("profile-field-current-password-email-change").as_str());
        save_profile_button_for_i18n.set_label(crate::tr!("profile-save").as_str());

        let lang_selected = language_dropdown_for_i18n.selected();
        language_items_for_i18n.splice(
            0,
            language_items_for_i18n.n_items(),
            &[
                crate::tr!("language-option-fr").as_str(),
                crate::tr!("language-option-en").as_str(),
            ],
        );
        language_dropdown_for_i18n.set_selected(lang_selected.min(1));

        password_change_frame_for_i18n
            .set_label(Some(crate::tr!("profile-section-password-change").as_str()));
        password_change_subtitle_for_i18n
            .set_text(crate::tr!("profile-section-password-change-subtitle").as_str());
        current_pw_label_for_i18n
            .set_text(crate::tr!("profile-field-current-password").as_str());
        new_pw_label_for_i18n.set_text(crate::tr!("profile-field-new-password").as_str());
        confirm_pw_label_for_i18n
            .set_text(crate::tr!("profile-field-confirm-new-password").as_str());
        change_pw_button_for_i18n.set_label(crate::tr!("profile-change-button").as_str());
        change_pw_button_for_i18n
            .set_tooltip_text(Some(crate::tr!("profile-change-button-tooltip").as_str()));
        rotate_master_key_button_for_i18n
            .set_label(crate::tr!("profile-rotate-button").as_str());
        rotate_master_key_button_for_i18n
            .set_tooltip_text(Some(crate::tr!("profile-rotate-button-tooltip").as_str()));

        security_prefs_frame_for_i18n
            .set_label(Some(crate::tr!("profile-section-security-prefs").as_str()));
        auto_lock_label_for_i18n.set_text(crate::tr!("profile-auto-lock-title").as_str());
        let auto_lock_selected = auto_lock_dropdown_for_i18n.selected();
        auto_lock_items_for_i18n.splice(
            0,
            auto_lock_items_for_i18n.n_items(),
            &[
                crate::tr!("profile-auto-lock-1").as_str(),
                crate::tr!("profile-auto-lock-5").as_str(),
                crate::tr!("profile-auto-lock-15").as_str(),
                crate::tr!("profile-auto-lock-30").as_str(),
                crate::tr!("profile-auto-lock-never").as_str(),
            ],
        );
        auto_lock_dropdown_for_i18n.set_selected(auto_lock_selected.min(4));
        show_edit_passwords_label_for_i18n
            .set_text(crate::tr!("profile-show-edit-passwords-title").as_str());
        show_edit_passwords_hint_for_i18n
            .set_text(crate::tr!("profile-show-edit-passwords-hint").as_str());

        twofa_title_for_i18n.set_text(crate::tr!("profile-section-2fa-title").as_str());
        twofa_disabled_copy_for_i18n.set_text(crate::tr!("profile-2fa-disabled-copy").as_str());
        twofa_activate_button_for_i18n.set_label(crate::tr!("profile-2fa-activate").as_str());
        twofa_setup_copy_for_i18n.set_text(crate::tr!("profile-2fa-setup-copy").as_str());
        twofa_secret_label_for_i18n.set_text(crate::tr!("profile-2fa-secret-base32").as_str());
        twofa_code_label_for_i18n.set_text(crate::tr!("profile-2fa-code").as_str());
        twofa_confirm_button_for_i18n.set_label(crate::tr!("profile-2fa-confirm").as_str());
        twofa_cancel_setup_button_for_i18n.set_label(crate::tr!("profile-2fa-cancel").as_str());
        twofa_enabled_copy_for_i18n.set_text(crate::tr!("profile-2fa-enabled-copy").as_str());
        twofa_disable_toggle_button_for_i18n
            .set_label(crate::tr!("profile-2fa-disable").as_str());
        twofa_disable_confirm_button_for_i18n
            .set_label(crate::tr!("profile-2fa-disable-confirm").as_str());
        twofa_disable_cancel_button_for_i18n
            .set_label(crate::tr!("profile-2fa-cancel").as_str());
        twofa_stack_for_i18n
            .page(&twofa_disabled_box_for_i18n)
            .set_title(crate::tr!("profile-2fa-stack-disabled").as_str());
        twofa_stack_for_i18n
            .page(&twofa_setup_box_for_i18n)
            .set_title(crate::tr!("profile-2fa-stack-setup").as_str());
        twofa_stack_for_i18n
            .page(&twofa_enabled_box_for_i18n)
            .set_title(crate::tr!("profile-2fa-stack-enabled").as_str());
        let is_twofa_enabled = twofa_stack_for_i18n
            .visible_child_name()
            .as_ref()
            .is_some_and(|name| name == "enabled");
        MainWindow::set_twofa_badge_state(&twofa_badge_for_i18n, is_twofa_enabled);

        data_frame_for_i18n.set_label(Some(crate::tr!("profile-section-data").as_str()));
        export_title_for_i18n.set_text(crate::tr!("profile-export-title").as_str());
        export_subtitle_for_i18n.set_text(crate::tr!("profile-export-subtitle").as_str());
        export_button_for_i18n.set_label(crate::tr!("profile-export-button").as_str());
        import_title_for_i18n.set_text(crate::tr!("profile-import-title").as_str());
        import_subtitle_for_i18n.set_text(crate::tr!("profile-import-subtitle").as_str());
        import_button_for_i18n.set_label(crate::tr!("profile-import-button").as_str());

        on_language_changed_for_i18n();
    });
    apply_profile_i18n();

    let content_for_compact = content.clone();
    let sections_columns_for_compact = sections_columns.clone();
    let save_row_for_compact = save_profile_row.clone();
    let security_actions_for_compact = security_actions.clone();
    let save_btn_for_compact = save_profile_button.clone();
    let change_btn_for_compact = change_pw_button.clone();
    let rotate_btn_for_compact = rotate_master_key_button.clone();
    let export_btn_for_compact = export_button.clone();
    let import_btn_for_compact = import_button.clone();
    container.add_tick_callback(move |widget, _clock| {
        if widget.allocated_width() < 760 {
            content_for_compact.add_css_class("profile-compact");
            sections_columns_for_compact.set_orientation(Orientation::Vertical);
            sections_columns_for_compact.set_homogeneous(false);
            save_row_for_compact.set_orientation(Orientation::Vertical);
            save_row_for_compact.set_halign(Align::Fill);
            security_actions_for_compact.set_orientation(Orientation::Vertical);
            security_actions_for_compact.set_halign(Align::Fill);

            for button in [
                save_btn_for_compact.clone(),
                change_btn_for_compact.clone(),
                rotate_btn_for_compact.clone(),
                export_btn_for_compact.clone(),
                import_btn_for_compact.clone(),
            ] {
                button.set_hexpand(true);
                button.set_halign(Align::Fill);
            }
        } else {
            content_for_compact.remove_css_class("profile-compact");
            sections_columns_for_compact.set_orientation(Orientation::Horizontal);
            sections_columns_for_compact.set_homogeneous(true);
            save_row_for_compact.set_orientation(Orientation::Horizontal);
            save_row_for_compact.set_halign(Align::End);
            security_actions_for_compact.set_orientation(Orientation::Horizontal);
            security_actions_for_compact.set_halign(Align::End);

            for button in [
                save_btn_for_compact.clone(),
                change_btn_for_compact.clone(),
                rotate_btn_for_compact.clone(),
                export_btn_for_compact.clone(),
                import_btn_for_compact.clone(),
            ] {
                button.set_hexpand(false);
                button.set_halign(Align::End);
            }
        }
        glib::ControlFlow::Continue
    });

    let begin_critical_operation: Rc<dyn Fn()> = {
        let counter = Rc::clone(&critical_ops_in_flight);
        let export_btn = export_button.clone();
        let import_btn = import_button.clone();
        Rc::new(move || {
            let next = counter.get().saturating_add(1);
            counter.set(next);
            export_btn.set_sensitive(false);
            import_btn.set_sensitive(false);
        })
    };

    let end_critical_operation: Rc<dyn Fn()> = {
        let counter = Rc::clone(&critical_ops_in_flight);
        let export_btn = export_button.clone();
        let import_btn = import_button.clone();
        Rc::new(move || {
            let next = counter.get().saturating_sub(1);
            counter.set(next);
            if next == 0 {
                export_btn.set_sensitive(true);
                import_btn.set_sensitive(true);
            }
        })
    };

    let loading_lock = Rc::new(Cell::new(true));
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let service_for_load = Arc::clone(&user_service);
    let totp_for_load = Arc::clone(&totp_service);
    let policy_for_load = Arc::clone(&auth_policy_service);
    let runtime_for_load = runtime_handle.clone();
    std::thread::spawn(move || {
        let result = runtime_for_load.block_on(async move {
            let user = service_for_load.get_user_profile(user_id).await?;
            let delay = policy_for_load
                .get_auto_lock_delay(user.username.as_str())
                .await?;
            let totp_enabled = totp_for_load.is_totp_enabled_for_user_id(user_id).await?;
            Ok::<_, crate::errors::AppError>((user, delay, totp_enabled))
        });
        let _ = sender.send(result);
    });

    let username_entry_for_load = username_entry.clone();
    let display_entry_for_load = display_entry.clone();
    let email_entry_for_load = email_entry.clone();
    let auto_lock_for_load = auto_lock_dropdown.clone();
    let language_dropdown_for_load = language_dropdown.clone();
    let show_edit_passwords_for_load = show_edit_passwords_switch.clone();
    let show_passwords_pref_for_load = Rc::clone(&show_passwords_in_edit_pref);
    let twofa_stack_for_load = twofa_stack.clone();
    let twofa_disable_confirm_row_for_load = twofa_disable_confirm_row.clone();
    let twofa_badge_for_load = twofa_state_badge.clone();
    let loading_lock_for_load = Rc::clone(&loading_lock);
    let profile_status_for_load = profile_status_label.clone();
    let apply_profile_i18n_for_load = Rc::clone(&apply_profile_i18n);
    glib::MainContext::default().spawn_local(async move {
        match receiver.await {
            Ok(Ok((user, delay, totp_enabled))) => {
                let language = user.preferred_language.trim().to_ascii_lowercase();
                let _ = crate::i18n::set_language(language.as_str());
                apply_profile_i18n_for_load();
                username_entry_for_load.set_text(user.username.as_str());
                display_entry_for_load
                    .set_text(user.display_name.as_deref().unwrap_or_default());
                email_entry_for_load.set_text(user.email.as_deref().unwrap_or_default());
                language_dropdown_for_load.set_selected(if language.starts_with("en") {
                    1
                } else {
                    0
                });
                let selected = match delay {
                    1 => 0,
                    5 => 1,
                    15 => 2,
                    30 => 3,
                    0 => 4,
                    _ => 1,
                };
                auto_lock_for_load.set_selected(selected);
                show_edit_passwords_for_load.set_active(user.show_passwords_in_edit);
                show_passwords_pref_for_load.set(user.show_passwords_in_edit);
                twofa_stack_for_load.set_visible_child_name(if totp_enabled {
                    "enabled"
                } else {
                    "disabled"
                });
                MainWindow::set_twofa_badge_state(&twofa_badge_for_load, totp_enabled);
                twofa_disable_confirm_row_for_load.set_visible(false);
                loading_lock_for_load.set(false);
            }
            _ => {
                loading_lock_for_load.set(false);
                MainWindow::set_inline_status(
                    &profile_status_for_load,
                    crate::tr!("profile-status-load-failed").as_str(),
                    "error",
                );
            }
        }
    });

    let policy_for_delay = Arc::clone(&auth_policy_service);
    let runtime_for_delay = runtime_handle.clone();
    let username_for_delay = username_entry.clone();
    let loading_lock_for_delay = Rc::clone(&loading_lock);
    let window_for_delay = window.clone();
    let security_status_for_delay = security_prefs_status_label.clone();
    let session_for_delay = Rc::clone(&session_master_key);
    auto_lock_dropdown.connect_selected_notify(move |dropdown| {
        if loading_lock_for_delay.get() {
            return;
        }

        let username = username_for_delay.text().trim().to_string();
        if username.is_empty() {
            return;
        }

        let mins = match dropdown.selected() {
            0 => 1,
            1 => 5,
            2 => 15,
            3 => 30,
            4 => 0,
            _ => 5,
        };
        MainWindow::set_inline_status(
            &security_status_for_delay,
            crate::tr!("profile-status-lock-delay-updating").as_str(),
            "loading",
        );

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = runtime_for_delay.clone();
        let policy_for_task = Arc::clone(&policy_for_delay);
        std::thread::spawn(move || {
            let result = runtime_for_task.block_on(async move {
                policy_for_task
                    .update_auto_lock_delay(username.as_str(), mins)
                    .await
            });
            let _ = sender.send((mins, result));
        });

        let security_status_for_result = security_status_for_delay.clone();
        let auto_lock_timeout_for_result = Rc::clone(&auto_lock_timeout_secs);
        let auto_lock_source_for_result = Rc::clone(&auto_lock_source);
        let auto_lock_armed_for_result = Rc::clone(&auto_lock_armed);
        let on_auto_lock_for_result = Rc::clone(&on_auto_lock);
        let session_for_result = Rc::clone(&session_for_delay);
        let window_for_result = window_for_delay.clone();
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok((updated_mins, Ok(()))) => {
                    auto_lock_timeout_for_result.set((updated_mins as u64).saturating_mul(60));
                    if updated_mins == 0 {
                        auto_lock_armed_for_result.set(false);
                        if let Some(source_id) = auto_lock_source_for_result.borrow_mut().take()
                        {
                            source_id.remove();
                        }
                    } else if auto_lock_armed_for_result.get() {
                        auto_lock::reset_auto_lock_timer(
                            &window_for_result,
                            &auto_lock_source_for_result,
                            &auto_lock_armed_for_result,
                            auto_lock_timeout_for_result.get(),
                            &on_auto_lock_for_result,
                            &session_for_result,
                        );
                    }
                    MainWindow::set_inline_status(
                        &security_status_for_result,
                        crate::tr!("profile-status-lock-delay-updated").as_str(),
                        "success",
                    );
                }
                _ => {
                    MainWindow::set_inline_status(
                        &security_status_for_result,
                        crate::tr!("profile-status-lock-delay-failed").as_str(),
                        "error",
                    );
                }
            }
        });
    });

    let service_for_profile_save = Arc::clone(&user_service);
    let runtime_for_profile_save = runtime_handle.clone();
    let display_for_save = display_entry.clone();
    let email_for_save = email_entry.clone();
    let language_dropdown_for_save = language_dropdown.clone();
    let current_email_pw_for_save = current_email_pw_entry.clone();
    let show_edit_passwords_for_save = show_edit_passwords_switch.clone();
    let show_passwords_pref_for_save = Rc::clone(&show_passwords_in_edit_pref);
    let profile_badge_for_save = profile_badge.clone();
    let profile_status_for_save = profile_status_label.clone();
    let apply_profile_i18n_for_save = Rc::clone(&apply_profile_i18n);
    let service_for_toggle = Arc::clone(&user_service);
    let runtime_for_toggle = runtime_handle.clone();
    let profile_status_for_toggle = security_prefs_status_label.clone();
    let show_passwords_pref_for_toggle = Rc::clone(&show_passwords_in_edit_pref);
    show_edit_passwords_switch.connect_active_notify(move |switch_widget| {
        let enabled = switch_widget.is_active();
        MainWindow::set_inline_status(
            &profile_status_for_toggle,
            crate::tr!("profile-status-show-passwords-updating").as_str(),
            "loading",
        );

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = runtime_for_toggle.clone();
        let service_for_task = Arc::clone(&service_for_toggle);
        std::thread::spawn(move || {
            let result = runtime_for_task.block_on(async move {
                service_for_task
                    .update_show_passwords_in_edit(user_id, enabled)
                    .await
            });
            let _ = sender.send((enabled, result));
        });

        let profile_status_for_result = profile_status_for_toggle.clone();
        let show_passwords_pref_for_result = Rc::clone(&show_passwords_pref_for_toggle);
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok((_, Ok(user))) => {
                    show_passwords_pref_for_result.set(user.show_passwords_in_edit);
                    MainWindow::set_inline_status(
                        &profile_status_for_result,
                        crate::tr!("profile-status-show-passwords-updated").as_str(),
                        "success",
                    );
                }
                _ => {
                    MainWindow::set_inline_status(
                        &profile_status_for_result,
                        crate::tr!("profile-status-show-passwords-failed").as_str(),
                        "error",
                    );
                }
            }
        });
    });
    save_profile_button.connect_clicked(move |_| {
        MainWindow::set_inline_status(
            &profile_status_for_save,
            crate::tr!("profile-status-saving").as_str(),
            "loading",
        );
        let payload = crate::services::user_service::UserProfileUpdate {
            email: {
                let value = email_for_save.text().trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            },
            display_name: {
                let value = display_for_save.text().trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            },
            preferred_language: Some(if language_dropdown_for_save.selected() == 1 {
                "en".to_string()
            } else {
                "fr".to_string()
            }),
            show_passwords_in_edit: Some(show_edit_passwords_for_save.is_active()),
            current_password: {
                let value = current_email_pw_for_save.text().trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(SecretBox::new(Box::new(value.into_bytes())))
                }
            },
        };

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = runtime_for_profile_save.clone();
        let service_for_task = Arc::clone(&service_for_profile_save);
        std::thread::spawn(move || {
            let result = runtime_for_task.block_on(async move {
                service_for_task.update_user_profile(user_id, payload).await
            });
            let _ = sender.send(result);
        });

        let badge_for_result = profile_badge_for_save.clone();
        let profile_status_for_result = profile_status_for_save.clone();
        let show_passwords_pref_for_result = Rc::clone(&show_passwords_pref_for_save);
        let apply_profile_i18n_for_result = Rc::clone(&apply_profile_i18n_for_save);
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(user)) => {
                    let _ = crate::i18n::set_language(user.preferred_language.as_str());
                    apply_profile_i18n_for_result();
                    let display = user
                        .display_name
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or(user.username.clone());
                    show_passwords_pref_for_result.set(user.show_passwords_in_edit);
                    badge_for_result.set_label(
                        crate::i18n::tr_args(
                            "main-connected-label",
                            &[("name", crate::i18n::I18nArg::Str(display.as_str()))],
                        )
                        .as_str(),
                    );
                    MainWindow::set_inline_status(
                        &profile_status_for_result,
                        crate::tr!("profile-status-saved").as_str(),
                        "success",
                    );
                }
                _ => {
                    MainWindow::set_inline_status(
                        &profile_status_for_result,
                        crate::tr!("profile-status-save-failed").as_str(),
                        "error",
                    );
                }
            }
        });
    });

    let service_for_pw_change = Arc::clone(&user_service);
    let runtime_for_pw_change = runtime_handle.clone();
    let current_pw_for_change = current_pw_entry.clone();
    let security_status_for_pw_change = password_change_status_label.clone();
    change_pw_button.connect_clicked(move |_| {
        MainWindow::set_inline_status(
            &security_status_for_pw_change,
            crate::tr!("profile-status-password-change-ready").as_str(),
            "success",
        );
        current_pw_for_change.grab_focus();
    });

    let service_for_rotate = Arc::clone(&service_for_pw_change);
    let runtime_for_rotate = runtime_for_pw_change.clone();
    let current_pw_for_rotate = current_pw_entry.clone();
    let new_pw_for_rotate = new_pw_entry.clone();
    let confirm_pw_for_rotate = confirm_pw_entry.clone();
    let security_status_for_rotate = password_change_status_label.clone();
    rotate_master_key_button.connect_clicked(move |_| {
        let current_raw = current_pw_for_rotate.text().trim().to_string();
        let new_raw = new_pw_for_rotate.text().trim().to_string();
        let confirm_raw = confirm_pw_for_rotate.text().trim().to_string();
        if current_raw.is_empty() || new_raw.is_empty() || confirm_raw.is_empty() {
            MainWindow::set_inline_status(
                &security_status_for_rotate,
                crate::tr!("profile-status-password-fields-required").as_str(),
                "error",
            );
            return;
        }
        if new_raw != confirm_raw {
            MainWindow::set_inline_status(
                &security_status_for_rotate,
                crate::tr!("profile-status-password-confirm-mismatch").as_str(),
                "error",
            );
            return;
        }

        MainWindow::set_inline_status(
            &security_status_for_rotate,
            crate::tr!("profile-status-password-updating").as_str(),
            "loading",
        );

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = runtime_for_rotate.clone();
        let service_for_task = Arc::clone(&service_for_rotate);
        std::thread::spawn(move || {
            let result = runtime_for_task.block_on(async move {
                service_for_task
                    .change_master_password(
                        user_id,
                        SecretBox::new(Box::new(current_raw.into_bytes())),
                        SecretBox::new(Box::new(new_raw.into_bytes())),
                    )
                    .await
            });
            let _ = sender.send(result);
        });

        let current_for_result = current_pw_for_rotate.clone();
        let new_for_result = new_pw_for_rotate.clone();
        let confirm_for_result = confirm_pw_for_rotate.clone();
        let security_status_for_result = security_status_for_rotate.clone();
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(())) => {
                    current_for_result.set_text("");
                    new_for_result.set_text("");
                    confirm_for_result.set_text("");
                    MainWindow::set_inline_status(
                        &security_status_for_result,
                        crate::tr!("profile-status-password-updated").as_str(),
                        "success",
                    );
                }
                _ => {
                    MainWindow::set_inline_status(
                        &security_status_for_result,
                        crate::tr!("profile-status-password-failed").as_str(),
                        "error",
                    );
                }
            }
        });
    });

    let pending_totp_secret: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let username_for_twofa_activate = username_entry.clone();
    let twofa_stack_for_activate = twofa_stack.clone();
    let twofa_qr_for_activate = twofa_qr_picture.clone();
    let twofa_secret_for_activate = twofa_secret_entry.clone();
    let twofa_code_for_activate = twofa_code_entry.clone();
    let twofa_status_for_activate = twofa_status_label.clone();
    let pending_secret_for_activate = Rc::clone(&pending_totp_secret);
    let totp_for_activate = Arc::clone(&totp_service);
    twofa_activate_button.connect_clicked(move |_| {
        let username = username_for_twofa_activate.text().trim().to_string();
        if username.is_empty() {
            MainWindow::set_inline_status(
                &twofa_status_for_activate,
                crate::tr!("profile-status-twofa-profile-missing-start").as_str(),
                "error",
            );
            return;
        }

        match totp_for_activate.create_setup_payload(username.as_str()) {
            Ok(payload) => {
                *pending_secret_for_activate.borrow_mut() = Some(payload.base32_secret.clone());
                let loader = gtk4::gdk_pixbuf::PixbufLoader::new();
                if loader.write(&payload.qr_png).is_ok() && loader.close().is_ok() {
                    if let Some(pixbuf) = loader.pixbuf() {
                        let texture = gtk4::gdk::Texture::for_pixbuf(&pixbuf);
                        twofa_qr_for_activate.set_paintable(Some(&texture));
                    }
                }
                twofa_secret_for_activate.set_text(payload.base32_secret.as_str());
                twofa_code_for_activate.set_text("");
                twofa_stack_for_activate.set_visible_child_name("setup");
                MainWindow::set_inline_status(
                    &twofa_status_for_activate,
                    crate::tr!("profile-status-twofa-ready").as_str(),
                    "success",
                );
            }
            Err(error) => {
                MainWindow::set_inline_status(
                    &twofa_status_for_activate,
                    MainWindow::map_twofa_error(
                        &error,
                        crate::tr!("profile-status-twofa-prepare-failed").as_str(),
                    )
                    .as_str(),
                    "error",
                );
            }
        }
    });

    let pending_secret_for_cancel = Rc::clone(&pending_totp_secret);
    let twofa_stack_for_cancel = twofa_stack.clone();
    let twofa_status_for_cancel = twofa_status_label.clone();
    twofa_cancel_setup_button.connect_clicked(move |_| {
        *pending_secret_for_cancel.borrow_mut() = None;
        twofa_stack_for_cancel.set_visible_child_name("disabled");
        MainWindow::set_inline_status(
            &twofa_status_for_cancel,
            crate::tr!("profile-status-twofa-cancelled").as_str(),
            "success",
        );
    });

    let username_for_twofa_confirm = username_entry.clone();
    let twofa_code_for_confirm = twofa_code_entry.clone();
    let twofa_stack_for_confirm = twofa_stack.clone();
    let twofa_status_for_confirm = twofa_status_label.clone();
    let twofa_badge_for_confirm = twofa_state_badge.clone();
    let pending_secret_for_confirm = Rc::clone(&pending_totp_secret);
    let totp_for_confirm = Arc::clone(&totp_service);
    let runtime_for_twofa_confirm = runtime_handle.clone();
    twofa_confirm_button.connect_clicked(move |_| {
        let username = username_for_twofa_confirm.text().trim().to_string();
        let code = twofa_code_for_confirm.text().trim().to_string();

        if username.is_empty() {
            MainWindow::set_inline_status(
                &twofa_status_for_confirm,
                crate::tr!("profile-status-twofa-profile-missing-finish").as_str(),
                "error",
            );
            return;
        }

        let Some(secret) = pending_secret_for_confirm.borrow().clone() else {
            MainWindow::set_inline_status(
                &twofa_status_for_confirm,
                crate::tr!("profile-status-twofa-none-pending").as_str(),
                "error",
            );
            return;
        };

        if let Some(validation_message) = messages::validate_totp_code_format(code.as_str()) {
            MainWindow::set_inline_status(
                &twofa_status_for_confirm,
                validation_message.as_str(),
                "error",
            );
            return;
        }

        match totp_for_confirm.verify_setup_code(
            username.as_str(),
            secret.as_str(),
            code.as_str(),
        ) {
            Ok(true) => {
                MainWindow::set_inline_status(
                    &twofa_status_for_confirm,
                    crate::tr!("profile-status-twofa-enabling").as_str(),
                    "loading",
                );

                let (sender, receiver) = tokio::sync::oneshot::channel();
                let runtime_for_task = runtime_for_twofa_confirm.clone();
                let totp_for_task = Arc::clone(&totp_for_confirm);
                std::thread::spawn(move || {
                    let result = runtime_for_task.block_on(async move {
                        totp_for_task
                            .enable_totp(
                                user_id,
                                username.as_str(),
                                secret.as_str(),
                                code.as_str(),
                            )
                            .await
                    });
                    let _ = sender.send(result);
                });

                let twofa_stack_for_result = twofa_stack_for_confirm.clone();
                let twofa_status_for_result = twofa_status_for_confirm.clone();
                let twofa_badge_for_result = twofa_badge_for_confirm.clone();
                let twofa_code_for_result = twofa_code_for_confirm.clone();
                let pending_secret_for_result = Rc::clone(&pending_secret_for_confirm);
                glib::MainContext::default().spawn_local(async move {
                    match receiver.await {
                        Ok(Ok(())) => {
                            *pending_secret_for_result.borrow_mut() = None;
                            twofa_code_for_result.set_text("");
                            twofa_stack_for_result.set_visible_child_name("enabled");
                            MainWindow::set_twofa_badge_state(&twofa_badge_for_result, true);
                            MainWindow::set_inline_status(
                                &twofa_status_for_result,
                                crate::tr!("profile-status-twofa-enabled").as_str(),
                                "success",
                            );
                        }
                        Ok(Err(error)) => {
                            MainWindow::set_inline_status(
                                &twofa_status_for_result,
                                MainWindow::map_twofa_error(
                                    &error,
                                    crate::tr!("profile-status-twofa-enable-failed").as_str(),
                                )
                                .as_str(),
                                "error",
                            );
                        }
                        Err(_) => {
                            MainWindow::set_inline_status(
                                &twofa_status_for_result,
                                crate::tr!("profile-status-twofa-interrupted").as_str(),
                                "error",
                            );
                        }
                    }
                });
            }
            Ok(false) => {
                MainWindow::set_inline_status(
                    &twofa_status_for_confirm,
                    messages::profile_totp_code_invalid_error().as_str(),
                    "error",
                );
            }
            Err(error) => {
                MainWindow::set_inline_status(
                    &twofa_status_for_confirm,
                    MainWindow::map_twofa_error(
                        &error,
                        crate::tr!("profile-status-twofa-verify-failed").as_str(),
                    )
                    .as_str(),
                    "error",
                );
            }
        }
    });

    let twofa_confirm_row_for_toggle = twofa_disable_confirm_row.clone();
    twofa_disable_toggle_button.connect_clicked(move |_| {
        twofa_confirm_row_for_toggle.set_visible(true);
    });

    let twofa_confirm_row_for_cancel = twofa_disable_confirm_row.clone();
    twofa_disable_cancel_button.connect_clicked(move |_| {
        twofa_confirm_row_for_cancel.set_visible(false);
    });

    let totp_for_disable = Arc::clone(&totp_service);
    let runtime_for_disable = runtime_handle.clone();
    let twofa_stack_for_disable = twofa_stack.clone();
    let twofa_status_for_disable = twofa_status_label.clone();
    let twofa_badge_for_disable = twofa_state_badge.clone();
    let twofa_confirm_row_for_disable = twofa_disable_confirm_row.clone();
    twofa_disable_confirm_button.connect_clicked(move |_| {
        MainWindow::set_inline_status(
            &twofa_status_for_disable,
            crate::tr!("profile-status-twofa-disabling").as_str(),
            "loading",
        );

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let runtime_for_task = runtime_for_disable.clone();
        let totp_for_task = Arc::clone(&totp_for_disable);
        std::thread::spawn(move || {
            let result = runtime_for_task
                .block_on(async move { totp_for_task.disable_totp(user_id).await });
            let _ = sender.send(result);
        });

        let twofa_stack_for_result = twofa_stack_for_disable.clone();
        let twofa_status_for_result = twofa_status_for_disable.clone();
        let twofa_badge_for_result = twofa_badge_for_disable.clone();
        let twofa_confirm_row_for_result = twofa_confirm_row_for_disable.clone();
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(())) => {
                    twofa_confirm_row_for_result.set_visible(false);
                    twofa_stack_for_result.set_visible_child_name("disabled");
                    MainWindow::set_twofa_badge_state(&twofa_badge_for_result, false);
                    MainWindow::set_inline_status(
                        &twofa_status_for_result,
                        crate::tr!("profile-status-twofa-disabled").as_str(),
                        "success",
                    );
                }
                Ok(Err(error)) => {
                    MainWindow::set_inline_status(
                        &twofa_status_for_result,
                        MainWindow::map_twofa_error(
                            &error,
                            crate::tr!("profile-status-twofa-disable-failed").as_str(),
                        )
                        .as_str(),
                        "error",
                    );
                }
                Err(_) => {
                    MainWindow::set_inline_status(
                        &twofa_status_for_result,
                        crate::tr!("profile-status-twofa-interrupted").as_str(),
                        "error",
                    );
                }
            }
        });
    });

    let window_for_export = window.clone();
    let backup_app_for_export: Arc<TBackupApp> = Arc::clone(&backup_app_service);
    let backup_for_export = Arc::clone(&backup_service);
    let database_path_for_export = database_path.clone();
    let actor_user_id_for_export: Uuid = user_id;
    let begin_critical_for_export = Rc::clone(&begin_critical_operation);
    let end_critical_for_export = Rc::clone(&end_critical_operation);
    export_button.connect_clicked(move |_| {
        let window_for_dialog = window_for_export.clone();
        let backup_for_dialog = Arc::clone(&backup_for_export);
        let backup_app_for_dialog = Arc::clone(&backup_app_for_export);
        let db_path_for_dialog = database_path_for_export.clone();
        let actor_id_for_dialog = actor_user_id_for_export;
        let begin_critical_for_dialog = Rc::clone(&begin_critical_for_export);
        let end_critical_for_dialog = Rc::clone(&end_critical_for_export);

        let window_for_feedback = window_for_dialog.clone();
        let on_feedback: Rc<dyn Fn(&str, &str)> = Rc::new(move |title, body| {
            MainWindow::show_feedback_dialog(&window_for_feedback, title, body);
        });

        let run_export: ExportRunner = Rc::new(move |backup_path: PathBuf, recovery_phrase| {
            let backup_app_for_task = Arc::clone(&backup_app_for_dialog);
            let db_for_task = db_path_for_dialog.clone();
            Box::pin(async move {
                backup_app_for_task
                    .export_backup_secured(
                        actor_id_for_dialog,
                        db_for_task.as_path(),
                        backup_path.as_path(),
                        &recovery_phrase,
                    )
                    .await
                    .map(|_| ())
            })
        });

        RecoveryKeyExportDialog::show(RecoveryKeyExportDialogDeps {
            parent_window: window_for_dialog.upcast::<gtk4::Window>(),
            backup_service: backup_for_dialog,
            cancel_label_key: "trash-dialog-cancel",
            on_feedback,
            on_begin_critical: Some(begin_critical_for_dialog),
            on_end_critical: Some(end_critical_for_dialog),
            run_export,
        });
    });

    let window_for_import = window.clone();
    let import_for_profile = Arc::clone(&import_service);
    let secret_for_import = Arc::clone(&secret_service);
    let vault_for_import = Arc::clone(&vault_service);
    let runtime_for_import = runtime_handle.clone();
    let session_for_import = Rc::clone(&session_master_key);
    let refresh_for_import = Rc::clone(&on_import_completed_refresh);
    let begin_critical_for_import = Rc::clone(&begin_critical_operation);
    let end_critical_for_import = Rc::clone(&end_critical_operation);
    import_button.connect_clicked(move |_| {
        let chooser = gtk4::FileChooserNative::builder()
            .title(crate::tr!("profile-import-chooser-title").as_str())
            .transient_for(&window_for_import)
            .accept_label(crate::tr!("profile-import-accept").as_str())
            .cancel_label(crate::tr!("trash-dialog-cancel").as_str())
            .action(gtk4::FileChooserAction::Open)
            .build();

        let window_for_response = window_for_import.clone();
        let import_for_response = Arc::clone(&import_for_profile);
        let secret_for_response = Arc::clone(&secret_for_import);
        let vault_for_response = Arc::clone(&vault_for_import);
        let runtime_for_response = runtime_for_import.clone();
        let session_for_response = Rc::clone(&session_for_import);
        let refresh_for_response = Rc::clone(&refresh_for_import);
        let begin_critical_for_response = Rc::clone(&begin_critical_for_import);
        let end_critical_for_response = Rc::clone(&end_critical_for_import);
        chooser.connect_response(move |dialog, response| {
            if response != gtk4::ResponseType::Accept {
                dialog.destroy();
                return;
            }

            let selected = dialog.file();
            dialog.destroy();
            let Some(file) = selected else {
                MainWindow::show_feedback_dialog(
                    &window_for_response,
                    crate::tr!("profile-import-accept").as_str(),
                    crate::tr!("profile-import-invalid-file").as_str(),
                );
                return;
            };
            let Some(csv_path) = file.path() else {
                MainWindow::show_feedback_dialog(
                    &window_for_response,
                    crate::tr!("profile-import-accept").as_str(),
                    crate::tr!("profile-import-invalid-path").as_str(),
                );
                return;
            };

            let Some(master_key) = MainWindow::snapshot_session_master_key(&session_for_response)
            else {
                MainWindow::show_feedback_dialog(
                    &window_for_response,
                    crate::tr!("profile-import-accept").as_str(),
                    crate::tr!("profile-import-session-locked").as_str(),
                );
                return;
            };

            let (sender, receiver) = tokio::sync::oneshot::channel();
            let import_for_task = Arc::clone(&import_for_response);
            let secret_for_task = Arc::clone(&secret_for_response);
            let vault_for_task = Arc::clone(&vault_for_response);
            let runtime_for_task = runtime_for_response.clone();
            begin_critical_for_response();
            std::thread::spawn(move || {
                let result = runtime_for_task.block_on(async move {
                    import_for_task
                        .import_csv(
                            csv_path.as_path(),
                            user_id,
                            SecretBox::new(Box::new(master_key)),
                            secret_for_task,
                            vault_for_task,
                        )
                        .await
                });
                let _ = sender.send(result);
            });

            let window_for_result = window_for_response.clone();
            let refresh_for_result = Rc::clone(&refresh_for_response);
            let end_critical_for_result = Rc::clone(&end_critical_for_response);
            glib::MainContext::default().spawn_local(async move {
                let result = receiver.await;
                end_critical_for_result();
                match result {
                    Ok(Ok(count)) => {
                        refresh_for_result();
                        let message = crate::i18n::tr_args(
                            "profile-import-success-body",
                            &[("count", crate::i18n::I18nArg::Num(count as i64))],
                        );
                        MainWindow::show_feedback_dialog(
                            &window_for_result,
                            crate::tr!("profile-import-accept").as_str(),
                            message.as_str(),
                        );
                    }
                    _ => {
                        MainWindow::show_feedback_dialog(
                            &window_for_result,
                            crate::tr!("profile-import-accept").as_str(),
                            crate::tr!("profile-import-failed").as_str(),
                        );
                    }
                }
            });
        });

        chooser.show();
    });

    ProfileViewWidgets {
        container,
        back_button,
    }
}
