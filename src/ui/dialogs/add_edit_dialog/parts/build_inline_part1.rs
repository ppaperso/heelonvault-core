        let on_saved: Rc<dyn Fn(String)> = Rc::new(on_saved);
        let on_cancel: Rc<dyn Fn()> = Rc::new(on_cancel);

        let container = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();

        let root = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        root.add_css_class("add-edit-dialog");

        let header_card = gtk4::Frame::new(None);
        header_card.add_css_class("login-hero");

        let header_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(14)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();

        let back_button = gtk4::Button::with_label(crate::tr!("add-edit-button-back").as_str());
        back_button.add_css_class("flat");
        let on_cancel_for_back = Rc::clone(&on_cancel);
        back_button.connect_clicked(move |_| {
            on_cancel_for_back();
        });

        let header_icon = gtk4::Image::from_resource(
            "/com/heelonvault/rust/icons/hicolor/128x128/apps/heelonvault.png",
        );
        header_icon.set_pixel_size(42);
        header_icon.add_css_class("login-hero-icon");

        let header_text = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .build();

        let inline_header_title_text = match mode {
            DialogMode::Create | DialogMode::CreateInVault(_) => {
                crate::tr!("add-edit-header-title-create")
            }
            DialogMode::Edit(_) => crate::tr!("add-edit-header-title-edit"),
        };
        let title = gtk4::Label::new(Some(inline_header_title_text.as_str()));
        title.add_css_class("title-2");
        title.add_css_class("login-hero-title");
        title.set_halign(Align::Start);

        let inline_subtitle_text = match mode {
            DialogMode::Create | DialogMode::CreateInVault(_) => {
                crate::tr!("add-edit-header-subtitle-create")
            }
            DialogMode::Edit(_) => crate::tr!("add-edit-header-subtitle-edit"),
        };
        let subtitle = gtk4::Label::new(Some(inline_subtitle_text.as_str()));
        subtitle.add_css_class("login-hero-copy");
        subtitle.set_halign(Align::Start);
        subtitle.set_wrap(true);

        header_text.append(&title);
        header_text.append(&subtitle);
        header_box.append(&back_button);
        header_box.append(&header_icon);
        header_box.append(&header_text);
        header_card.set_child(Some(&header_box));

        let form_card = gtk4::Frame::new(None);
        form_card.add_css_class("login-card");

        let form_box = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();

        let (title_row, title_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-title-label").as_str(),
            crate::tr!("add-edit-field-title-placeholder").as_str(),
            "dialog-title-entry",
        );
        let (category_row, category_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-category-label").as_str(),
            crate::tr!("add-edit-field-category-placeholder").as_str(),
            "dialog-category-entry",
        );
        let (tags_row, tags_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-tags-label").as_str(),
            crate::tr!("add-edit-field-tags-placeholder").as_str(),
            "dialog-tags-entry",
        );

        let type_label = gtk4::Label::new(Some(crate::tr!("add-edit-field-type-label").as_str()));
        type_label.add_css_class("login-field-label");
        type_label.set_halign(Align::Start);

        let type_items =
            gtk4::StringList::new(&["password", "api_token", "ssh_key", "secure_document"]);
        let type_dropdown = gtk4::DropDown::builder().model(&type_items).build();
        type_dropdown.add_css_class("dialog-type-dropdown");
        type_dropdown.set_selected(SecretType::Password.dropdown_index());

        let dynamic_stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::SlideLeftRight)
            .hexpand(true)
            .build();
        dynamic_stack.add_css_class("dialog-dynamic-stack");

        let password_hint = match mode {
            DialogMode::Edit(_) => Some(crate::tr!("add-edit-password-edit-hint")),
            DialogMode::Create | DialogMode::CreateInVault(_) => None,
        };
        let (password_panel, password_entry, password_strength_bar) =
            Self::build_password_panel(password_hint.as_deref());
        let initial_password_snapshot: Rc<std::cell::RefCell<Option<String>>> =
            Rc::new(std::cell::RefCell::new(None));
        dynamic_stack.add_titled(&password_panel, Some("password"), "password");

        let (api_token_panel, api_token_entry, api_provider_entry) = Self::build_api_token_panel();
        dynamic_stack.add_titled(&api_token_panel, Some("api_token"), "api_token");

        let (ssh_key_panel, ssh_private_text, ssh_public_entry, ssh_passphrase_entry) =
            Self::build_ssh_key_panel();
        dynamic_stack.add_titled(&ssh_key_panel, Some("ssh_key"), "ssh_key");

        let (secure_doc_panel, secure_doc_path_entry, secure_doc_mime_entry) =
            Self::build_secure_document_panel();
        dynamic_stack.add_titled(
            &secure_doc_panel,
            Some("secure_document"),
            "secure_document",
        );
        dynamic_stack.set_visible_child_name(Self::stack_name_for_secret_type(SecretType::Password));

        let stack_for_type = dynamic_stack.clone();
        type_dropdown.connect_selected_notify(move |dropdown| {
            match SecretType::from_dropdown_index(dropdown.selected()) {
                Some(secret_type) => {
                    stack_for_type
                        .set_visible_child_name(Self::stack_name_for_secret_type(secret_type));
                }
                None => {
                    warn!(selected_index = dropdown.selected(), "unexpected secret type dropdown index");
                }
            }
        });

        let (username_row, username_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-login-label").as_str(),
            crate::tr!("add-edit-field-login-placeholder").as_str(),
            "dialog-username-entry",
        );
        let (url_row, url_entry) = Self::build_labeled_entry(
            crate::tr!("add-edit-field-url-label").as_str(),
            crate::tr!("add-edit-field-url-placeholder").as_str(),
            "dialog-url-entry",
        );

        let notes_label = gtk4::Label::new(Some(crate::tr!("add-edit-field-notes-label").as_str()));
        notes_label.add_css_class("login-field-label");
        notes_label.set_halign(Align::Start);

        let notes_scrolled = gtk4::ScrolledWindow::builder()
            .min_content_height(120)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();
        notes_scrolled.add_css_class("dialog-notes-scroll");

        let notes_text = gtk4::TextView::new();
        notes_text.set_wrap_mode(gtk4::WrapMode::WordChar);
        notes_text.set_left_margin(10);
        notes_text.set_right_margin(10);
        notes_text.set_top_margin(10);
        notes_text.set_bottom_margin(10);
        notes_text.add_css_class("dialog-notes-text");
        notes_scrolled.set_child(Some(&notes_text));

        let validity_label =
            gtk4::Label::new(Some(crate::tr!("add-edit-field-validity-label").as_str()));
        validity_label.add_css_class("login-field-label");
        validity_label.set_halign(Align::Start);

        let validity_box = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();

        let validity_unlimited =
            gtk4::CheckButton::with_label(crate::tr!("add-edit-field-validity-unlimited").as_str());
        validity_unlimited.add_css_class("dialog-validity-check");

        let validity_adjustment = gtk4::Adjustment::new(90.0, 1.0, 3650.0, 1.0, 30.0, 0.0);
        let validity_days = gtk4::SpinButton::builder()
            .adjustment(&validity_adjustment)
            .digits(0)
            .numeric(true)
            .build();
        validity_days.add_css_class("dialog-validity-spin");
        validity_box.append(&validity_unlimited);
        validity_box.append(&validity_days);

        let days_for_toggle = validity_days.clone();
        validity_unlimited.connect_toggled(move |toggle| {
            days_for_toggle.set_sensitive(!toggle.is_active());
        });

        let error_label = gtk4::Label::new(None);
        error_label.add_css_class("login-error");
        error_label.set_halign(Align::Start);
        error_label.set_wrap(true);
        error_label.set_visible(false);

        let button_row = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(10)
            .halign(Align::End)
            .build();

        let cancel_button = gtk4::Button::with_label(crate::tr!("add-edit-button-cancel").as_str());
        cancel_button.add_css_class("secondary-pill");
        let on_cancel_for_cancel = Rc::clone(&on_cancel);
        cancel_button.connect_clicked(move |_| {
            on_cancel_for_cancel();
        });

        let save_button = gtk4::Button::new();
        save_button.add_css_class("primary-pill");
        let save_button_content = gtk4::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .halign(Align::Center)
            .build();
        let save_spinner = gtk4::Spinner::new();
        save_spinner.set_visible(false);
        let inline_save_label_text = match mode {
            DialogMode::Create | DialogMode::CreateInVault(_) => {
                crate::tr!("add-edit-button-save-create")
            }
            DialogMode::Edit(_) => crate::tr!("add-edit-button-save-edit"),
        };
        let save_label = gtk4::Label::new(Some(inline_save_label_text.as_str()));
        save_button_content.append(&save_spinner);
        save_button_content.append(&save_label);
        save_button.set_child(Some(&save_button_content));

        {
            let strength = password_strength_bar.clone();
            let dropdown = type_dropdown.clone();
            let btn = save_button.clone();
            let password_for_check = password_entry.clone();
            let initial_password_snapshot_for_check = Rc::clone(&initial_password_snapshot);
            let is_edit_mode = matches!(mode, DialogMode::Edit(_));
            let check: Rc<dyn Fn()> = Rc::new(move || {
                let selected_type = SecretType::from_dropdown_index(dropdown.selected());
                let is_password_type = matches!(selected_type, Some(SecretType::Password));
                let password_text = password_for_check.text();
                let password_raw = password_text.trim().to_string();
                let password_is_filled = !password_raw.is_empty();
                let unchanged_prefilled_password = if is_edit_mode {
                    initial_password_snapshot_for_check
                        .borrow()
                        .as_ref()
                        .map(|value| value == &password_raw)
                        .unwrap_or(false)
                } else {
                    false
                };

                let can_save =
                    if is_password_type && password_is_filled && !unchanged_prefilled_password {
                        // Password field is filled: enforce robuste score
                        strength.last_score() >= 4
                    } else if is_password_type && !password_is_filled && is_edit_mode {
                        // In edit mode with empty password: can update other fields
                        true
                    } else if is_password_type && unchanged_prefilled_password && is_edit_mode {
                        // Existing password only displayed: allow updating other fields
                        true
                    } else if is_password_type && !password_is_filled && !is_edit_mode {
                        // In create mode with empty password: cannot save
                        false
                    } else {
                        // Not password type: always allow
                        true
                    };
                btn.set_sensitive(can_save);
            });
            check();
            let c = Rc::clone(&check);
            password_entry.connect_text_notify(move |_| c());
            let c = Rc::clone(&check);
            type_dropdown.connect_selected_notify(move |_| c());
        }

        let title_for_save = title_entry.clone();
        let category_for_save = category_entry.clone();
        let tags_for_save = tags_entry.clone();
        let type_for_save = type_dropdown.clone();
        let username_for_save = username_entry.clone();
        let url_for_save = url_entry.clone();
        let notes_for_save = notes_text.buffer();
        let validity_unlimited_for_save = validity_unlimited.clone();
        let validity_days_for_save = validity_days.clone();
        let password_for_save = password_entry.clone();
        let api_token_for_save = api_token_entry.clone();
        let api_provider_for_save = api_provider_entry.clone();
        let ssh_private_for_save = ssh_private_text.clone();
        let ssh_public_for_save = ssh_public_entry.clone();
        let ssh_passphrase_for_save = ssh_passphrase_entry.clone();
        let secure_doc_for_save = secure_doc_path_entry.clone();
        let secure_doc_mime_for_save = secure_doc_mime_entry.clone();
