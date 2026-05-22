{
    // ── Window ────────────────────────────────────────────────────────────
    let window = gtk4::Window::builder()
        .modal(true)
        .transient_for(parent)
        .title(crate::tr!("pin-setup-title").as_str())
        .default_width(400)
        .resizable(false)
        .build();
    window.add_css_class("pin-setup-window");

    // ── Root ──────────────────────────────────────────────────────────────
    let root = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .build();

    let header = adw::HeaderBar::new();
    header.set_decoration_layout(Some(":close"));
    window.set_titlebar(Some(&header));

    let body = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(20)
        .margin_bottom(24)
        .margin_start(28)
        .margin_end(28)
        .build();

    // Description
    let desc_label = gtk4::Label::new(Some(crate::tr!("pin-setup-description").as_str()));
    desc_label.set_halign(Align::Start);
    desc_label.set_wrap(true);
    desc_label.add_css_class("dim-label");
    body.append(&desc_label);

    // PIN length hint
    let len_hint = gtk4::Label::new(Some(
        &crate::i18n::tr_args(
            "pin-setup-length-hint",
            &[
                ("min", crate::i18n::I18nArg::Num(PIN_MIN_LEN as i64)),
                ("max", crate::i18n::I18nArg::Num(PIN_MAX_LEN as i64)),
            ],
        ),
    ));
    len_hint.set_halign(Align::Start);
    len_hint.set_wrap(true);
    len_hint.add_css_class("caption");
    len_hint.add_css_class("dim-label");
    body.append(&len_hint);

    // New PIN entry
    let pin_entry = gtk4::Entry::new();
    pin_entry.set_visibility(false);
    pin_entry.set_placeholder_text(Some(crate::tr!("pin-setup-entry-placeholder").as_str()));
    pin_entry.set_max_length(PIN_MAX_LEN as i32);
    pin_entry.set_hexpand(true);
    pin_entry.set_input_purpose(InputPurpose::Pin);
    pin_entry.add_css_class("pin-digit-entry");
    body.append(&pin_entry);

    // Confirm PIN entry
    let confirm_entry = gtk4::Entry::new();
    confirm_entry.set_visibility(false);
    confirm_entry
        .set_placeholder_text(Some(crate::tr!("pin-setup-confirm-placeholder").as_str()));
    confirm_entry.set_max_length(PIN_MAX_LEN as i32);
    confirm_entry.set_hexpand(true);
    confirm_entry.set_input_purpose(InputPurpose::Pin);
    confirm_entry.add_css_class("pin-digit-entry");
    body.append(&confirm_entry);

    // Feedback label
    let feedback_label = gtk4::Label::new(None);
    feedback_label.set_halign(Align::Start);
    feedback_label.set_wrap(true);
    feedback_label.add_css_class("inline-status");
    feedback_label.add_css_class("pin-feedback-label");
    feedback_label.set_visible(false);
    body.append(&feedback_label);

    // Action buttons
    let btn_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .hexpand(true)
        .halign(Align::End)
        .build();

    let save_button = gtk4::Button::with_label(crate::tr!("pin-setup-save").as_str());
    save_button.add_css_class("suggested-action");
    btn_row.append(&save_button);

    if pin_currently_active {
        let disable_button = gtk4::Button::with_label(crate::tr!("pin-setup-disable").as_str());
        disable_button.add_css_class("destructive-action");
        btn_row.append(&disable_button);

        let on_pin_disabled = Rc::new(on_pin_disabled);
        let on_pin_disabled_for_btn = Rc::clone(&on_pin_disabled);
        let win_for_disable = window.clone();
        disable_button.connect_clicked(move |_| {
            win_for_disable.close();
            on_pin_disabled_for_btn();
        });
    }

    body.append(&btn_row);
    root.append(&body);
    window.set_child(Some(&root));

    // ── Helpers ───────────────────────────────────────────────────────────
    fn show_feedback(label: &gtk4::Label, msg: &str, is_error: bool) {
        label.set_text(msg);
        label.remove_css_class("inline-status-error");
        label.remove_css_class("inline-status-success");
        if is_error {
            label.add_css_class("inline-status-error");
        } else {
            label.add_css_class("inline-status-success");
        }
        label.set_visible(true);
    }

    fn hide_feedback(label: &gtk4::Label) {
        label.set_text("");
        label.set_visible(false);
    }

    // ── Save handler ──────────────────────────────────────────────────────
    let win_for_save = window.clone();
    let pin_for_save = pin_entry.clone();
    let confirm_for_save = confirm_entry.clone();
    let feedback_for_save = feedback_label.clone();
    let master_key = Rc::new(master_key);
    let on_cache_created = Rc::new(on_cache_created);

    save_button.connect_clicked(move |_| {
        let pin = pin_for_save.text().to_string();
        let confirm = confirm_for_save.text().to_string();

        if pin != confirm {
            show_feedback(
                &feedback_for_save,
                crate::tr!("pin-setup-error-mismatch").as_str(),
                true,
            );
            return;
        }

        if let Err(e) = validate_pin(&pin) {
            show_feedback(&feedback_for_save, &e.to_string(), true);
            return;
        }

        match PinCache::wrap(master_key.as_ref(), &pin, user_id) {
            Ok(cache) => {
                win_for_save.close();
                on_cache_created(cache);
            }
            Err(e) => {
                show_feedback(
                    &feedback_for_save,
                    &format!("{}: {e}", crate::tr!("pin-setup-error-internal").as_str()),
                    true,
                );
            }
        }
    });

    // Reset feedback on any change.
    for entry in [&pin_entry, &confirm_entry] {
        let fb = feedback_label.clone();
        entry.connect_changed(move |_| hide_feedback(&fb));
    }

    Self { window }
}