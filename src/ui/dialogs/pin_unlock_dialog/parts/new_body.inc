{
    // ── Window ────────────────────────────────────────────────────────────
    let window = gtk4::Window::builder()
        .modal(true)
        .transient_for(parent)
        .title(crate::tr!("pin-unlock-title").as_str())
        .default_width(360)
        .resizable(false)
        .deletable(false)
        .build();
    window.add_css_class("pin-unlock-window");

    // ── Root container ────────────────────────────────────────────────────
    let root = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(0)
        .build();

    // Header bar (no close button — user must use PIN or "use master password")
    let header = adw::HeaderBar::new();
    header.set_show_start_title_buttons(false);
    header.set_show_end_title_buttons(false);
    header.set_decoration_layout(Some(""));
    window.set_titlebar(Some(&header));

    // ── Body ──────────────────────────────────────────────────────────────
    let body = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(18)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(32)
        .margin_end(32)
        .build();

    // Lock icon
    let lock_icon = gtk4::Image::from_icon_name("changes-prevent-symbolic");
    lock_icon.set_pixel_size(48);
    lock_icon.add_css_class("pin-unlock-icon");
    body.append(&lock_icon);

    // Title label
    let title_label = gtk4::Label::new(Some(crate::tr!("pin-unlock-title").as_str()));
    title_label.add_css_class("title-2");
    title_label.add_css_class("pin-unlock-heading");
    title_label.set_halign(Align::Center);
    body.append(&title_label);

    // Subtitle
    let subtitle_label = gtk4::Label::new(Some(crate::tr!("pin-unlock-prompt").as_str()));
    subtitle_label.add_css_class("dim-label");
    subtitle_label.set_halign(Align::Center);
    subtitle_label.set_wrap(true);
    subtitle_label.set_justify(gtk4::Justification::Center);
    body.append(&subtitle_label);

    // PIN entry — digits-only, masked, max 8 chars
    let pin_entry = gtk4::Entry::new();
    pin_entry.set_visibility(false);
    pin_entry.set_placeholder_text(Some(crate::tr!("pin-unlock-entry-placeholder").as_str()));
    pin_entry.set_max_length(8);
    pin_entry.set_hexpand(true);
    pin_entry.add_css_class("pin-digit-entry");
    // Hint to on-screen keyboards to show a numeric pad
    pin_entry.set_input_purpose(InputPurpose::Pin);
    body.append(&pin_entry);

    // Inline feedback label (remaining attempts / error)
    let feedback_label = gtk4::Label::new(None);
    feedback_label.set_halign(Align::Center);
    feedback_label.set_wrap(true);
    feedback_label.add_css_class("inline-status");
    feedback_label.add_css_class("pin-feedback-label");
    feedback_label.set_visible(false);
    body.append(&feedback_label);

    // Buttons row
    let btn_row = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .hexpand(true)
        .build();

    let unlock_button = gtk4::Button::with_label(crate::tr!("pin-unlock-button").as_str());
    unlock_button.add_css_class("suggested-action");
    unlock_button.add_css_class("pill");
    unlock_button.set_hexpand(true);
    btn_row.append(&unlock_button);

    let fallback_button =
        gtk4::Button::with_label(crate::tr!("pin-use-master-password").as_str());
    fallback_button.add_css_class("flat");
    fallback_button.add_css_class("pin-fallback-btn");
    fallback_button.set_hexpand(true);
    btn_row.append(&fallback_button);

    body.append(&btn_row);
    root.append(&body);
    window.set_child(Some(&root));

    // ── State ─────────────────────────────────────────────────────────────
    let locked = Rc::new(Cell::new(false));

    // ── Helpers ───────────────────────────────────────────────────────────
    fn show_feedback(label: &gtk4::Label, message: &str, is_error: bool) {
        label.set_text(message);
        label.remove_css_class("inline-status-error");
        label.remove_css_class("inline-status-success");
        if is_error {
            label.add_css_class("inline-status-error");
        }
        label.set_visible(true);
    }

    fn clear_feedback(label: &gtk4::Label) {
        label.set_text("");
        label.set_visible(false);
    }

    // ── Unlock handler ────────────────────────────────────────────────────
    let main_for_unlock = Rc::clone(&main);
    let win_for_unlock = window.clone();
    let pin_entry_for_unlock = pin_entry.clone();
    let feedback_for_unlock = feedback_label.clone();
    let locked_for_unlock = Rc::clone(&locked);
    let on_unlocked = Rc::new(on_unlocked);
    let on_unlocked_for_btn = Rc::clone(&on_unlocked);

    let do_unlock = move || {
        if locked_for_unlock.get() {
            return;
        }
        let pin = pin_entry_for_unlock.text().to_string();
        if pin.is_empty() {
            show_feedback(
                &feedback_for_unlock,
                crate::tr!("pin-error-empty").as_str(),
                true,
            );
            return;
        }

        locked_for_unlock.set(true);
        clear_feedback(&feedback_for_unlock);

        match main_for_unlock.try_pin_unlock(&pin) {
            Ok(master_key) => {
                win_for_unlock.close();
                on_unlocked_for_btn(master_key);
            }
            Err(PinUnlockError::WrongPin { remaining }) => {
                locked_for_unlock.set(false);
                pin_entry_for_unlock.set_text("");
                let msg = if remaining == 1 {
                    crate::i18n::tr("pin-error-wrong-last")
                } else {
                    crate::i18n::tr_args("pin-error-wrong", &[("remaining", crate::i18n::I18nArg::Num(remaining as i64))])
                };
                show_feedback(&feedback_for_unlock, &msg, true);

                // Small enforced delay between attempts (1 s) to slow manual brute-force.
                let locked_delay = Rc::clone(&locked_for_unlock);
                glib::timeout_add_local_once(std::time::Duration::from_millis(1000), move || {
                    locked_delay.set(false);
                });
            }
            Err(PinUnlockError::Exhausted) => {
                // Cache already wiped by try_pin_unlock.  Close dialog and fall
                // back to full master-password login.
                warn!("PIN cache exhausted after max failed attempts — falling back to full login");
                win_for_unlock.close();
                // Treat as "use master password" — on_use_master_password will be
                // called via the fallback path that the caller sets up.
                on_unlocked_for_btn(Vec::new()); // empty vec signals exhaustion to caller
            }
        }
    };

    // Enter key in the PIN field triggers unlock.
    let do_unlock_rc = Rc::new(do_unlock);
    let do_unlock_for_btn = Rc::clone(&do_unlock_rc);
    let do_unlock_for_entry = Rc::clone(&do_unlock_rc);

    unlock_button.connect_clicked(move |_| do_unlock_for_btn());
    pin_entry.connect_activate(move |_| do_unlock_for_entry());

    // Reset feedback on input change.
    let feedback_for_change = feedback_label.clone();
    pin_entry.connect_changed(move |_| clear_feedback(&feedback_for_change));

    // ── Fallback button ───────────────────────────────────────────────────
    let main_for_fallback = Rc::clone(&main);
    let win_for_fallback = window.clone();
    let on_use_master_password = Rc::new(on_use_master_password);
    fallback_button.connect_clicked(move |_| {
        main_for_fallback.clear_pin_cache();
        win_for_fallback.close();
        on_use_master_password();
    });

    Self { window }
}
