use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Orientation};

use crate::errors::AppError;
use crate::ui::widgets::password_strength_bar::PasswordStrengthBar;

use super::{feedback, window_state};

pub(super) fn present_restore_dialog(
    parent: &gtk4::Window,
    on_restore_requested: Arc<
        dyn Fn(PathBuf, String, String) -> Result<(), AppError> + Send + Sync,
    >,
    on_restore_completed: Rc<dyn Fn()>,
) {
    let (restore_width, restore_height) = window_state::resolve_restore_window_size();

    let dialog = gtk4::Window::builder()
        .transient_for(parent)
        .title(crate::tr!("login-restore-dialog-title").as_str())
        .modal(true)
        .resizable(true)
        .default_width(restore_width)
        .default_height(restore_height)
        .build();
    let (restore_min_width, restore_min_height) = window_state::restore_min_size();
    dialog.set_size_request(restore_min_width, restore_min_height);

    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(14)
        .margin_top(20)
        .margin_bottom(20)
        .margin_start(20)
        .margin_end(20)
        .build();

    let title = gtk4::Label::new(Some(crate::tr!("login-restore-description").as_str()));
    title.set_wrap(true);
    title.set_halign(Align::Start);

    let file_label = gtk4::Label::new(Some(crate::tr!("login-restore-file-label").as_str()));
    file_label.add_css_class("login-field-label");
    file_label.set_halign(Align::Start);

    let file_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();

    let file_entry = gtk4::Entry::builder()
        .placeholder_text(crate::tr!("login-restore-file-placeholder").as_str())
        .hexpand(true)
        .build();
    file_entry.add_css_class("login-entry");

    let browse_button = gtk4::Button::with_label(crate::tr!("login-restore-browse").as_str());
    browse_button.add_css_class("secondary-pill");

    file_row.append(&file_entry);
    file_row.append(&browse_button);

    let phrase_label = gtk4::Label::new(Some(crate::tr!("login-restore-phrase-label").as_str()));
    phrase_label.add_css_class("login-field-label");
    phrase_label.set_halign(Align::Start);

    let phrase_entry = gtk4::Entry::builder()
        .placeholder_text(crate::tr!("login-restore-phrase-placeholder").as_str())
        .hexpand(true)
        .build();
    phrase_entry.add_css_class("login-entry");

    let password_label =
        gtk4::Label::new(Some(crate::tr!("login-restore-password-label").as_str()));
    password_label.add_css_class("login-field-label");
    password_label.set_halign(Align::Start);

    let new_password_entry = gtk4::PasswordEntry::builder()
        .placeholder_text(crate::tr!("login-restore-password-placeholder").as_str())
        .hexpand(true)
        .show_peek_icon(true)
        .build();
    new_password_entry.add_css_class("login-entry");

    let strength_bar = PasswordStrengthBar::new();
    strength_bar.connect_to_password_entry(&new_password_entry);

    let confirm_label = gtk4::Label::new(Some(crate::tr!("login-restore-confirm-label").as_str()));
    confirm_label.add_css_class("login-field-label");
    confirm_label.set_halign(Align::Start);

    let confirm_password_entry = gtk4::PasswordEntry::builder()
        .placeholder_text(crate::tr!("login-restore-confirm-placeholder").as_str())
        .hexpand(true)
        .show_peek_icon(true)
        .build();
    confirm_password_entry.add_css_class("login-entry");

    let error_label = gtk4::Label::new(None);
    error_label.add_css_class("login-error");
    error_label.set_wrap(true);
    error_label.set_halign(Align::Start);
    error_label.set_visible(false);

    let button_box = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    let cancel_button = gtk4::Button::with_label(crate::tr!("login-restore-cancel").as_str());
    cancel_button.add_css_class("secondary-pill");

    let restore_button = gtk4::Button::builder()
        .hexpand(true)
        .halign(Align::Fill)
        .sensitive(false)
        .build();
    restore_button.add_css_class("primary-pill");

    let restore_content = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .halign(Align::Center)
        .build();
    let restore_spinner = gtk4::Spinner::new();
    restore_spinner.set_visible(false);
    let restore_label = gtk4::Label::new(Some(crate::tr!("login-restore-submit").as_str()));
    restore_content.append(&restore_spinner);
    restore_content.append(&restore_label);
    restore_button.set_child(Some(&restore_content));

    button_box.append(&cancel_button);
    button_box.append(&restore_button);

    content.append(&title);
    content.append(&file_label);
    content.append(&file_row);
    content.append(&phrase_label);
    content.append(&phrase_entry);
    content.append(&password_label);
    content.append(&new_password_entry);
    content.append(strength_bar.root());
    content.append(&confirm_label);
    content.append(&confirm_password_entry);
    content.append(&error_label);
    content.append(&button_box);

    let content_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .min_content_width(420)
        .build();
    content_scroll.set_child(Some(&content));
    dialog.set_child(Some(&content_scroll));

    dialog.connect_close_request(move |win| {
        window_state::save_restore_window_size(win.width(), win.height());
        glib::Propagation::Proceed
    });

    let busy = Rc::new(Cell::new(false));
    let update_action_state: Rc<dyn Fn()> = {
        let busy = Rc::clone(&busy);
        let restore_button = restore_button.clone();
        let file_entry = file_entry.clone();
        let phrase_entry = phrase_entry.clone();
        let new_password_entry = new_password_entry.clone();
        let confirm_password_entry = confirm_password_entry.clone();
        Rc::new(move || {
            let has_file = !file_entry.text().trim().is_empty();
            let has_phrase = !phrase_entry.text().trim().is_empty();
            let has_password = !new_password_entry.text().is_empty();
            let matches_confirmation = new_password_entry.text() == confirm_password_entry.text();
            restore_button.set_sensitive(
                !busy.get() && has_file && has_phrase && has_password && matches_confirmation,
            );
        })
    };

    for editable in [
        file_entry.clone().upcast::<gtk4::Editable>(),
        phrase_entry.clone().upcast::<gtk4::Editable>(),
        new_password_entry.clone().upcast::<gtk4::Editable>(),
        confirm_password_entry.clone().upcast::<gtk4::Editable>(),
    ] {
        let update_action_state = Rc::clone(&update_action_state);
        editable.connect_changed(move |_| {
            update_action_state();
        });
    }

    let parent_for_chooser = dialog.clone();
    let file_entry_for_chooser = file_entry.clone();
    let update_action_for_chooser = Rc::clone(&update_action_state);
    browse_button.connect_clicked(move |_| {
        let chooser = gtk4::FileChooserNative::builder()
            .title("Choisir un export .hvb")
            .transient_for(&parent_for_chooser)
            .action(gtk4::FileChooserAction::Open)
            .accept_label("Selectionner")
            .cancel_label("Annuler")
            .build();

        let filter = gtk4::FileFilter::new();
        filter.add_pattern("*.hvb");
        filter.set_name(Some("Sauvegardes HeelonVault (*.hvb)"));
        chooser.set_filter(&filter);

        let file_entry_for_response = file_entry_for_chooser.clone();
        let update_action_for_response = Rc::clone(&update_action_for_chooser);
        chooser.connect_response(move |dialog, response| {
            if response == gtk4::ResponseType::Accept {
                if let Some(file) = dialog.file() {
                    if let Some(path) = file.path() {
                        file_entry_for_response.set_text(&path.display().to_string());
                        update_action_for_response();
                    }
                }
            }
            dialog.destroy();
        });

        chooser.show();
    });

    let dialog_for_cancel = dialog.clone();
    cancel_button.connect_clicked(move |_| {
        dialog_for_cancel.close();
    });

    let dialog_for_submit = dialog.clone();
    let error_for_submit = error_label.clone();
    let file_for_submit = file_entry.clone();
    let phrase_for_submit = phrase_entry.clone();
    let password_for_submit = new_password_entry.clone();
    let confirm_for_submit = confirm_password_entry.clone();
    let restore_button_for_submit = restore_button.clone();
    let restore_spinner_for_submit = restore_spinner.clone();
    let busy_for_submit = Rc::clone(&busy);
    restore_button.connect_clicked(move |_| {
        let file_path = file_for_submit.text().trim().to_string();
        let recovery_phrase = phrase_for_submit.text().trim().to_string();
        let new_password = password_for_submit.text().to_string();
        let confirmation = confirm_for_submit.text().to_string();

        feedback::clear_feedback(&error_for_submit);

        if file_path.is_empty() {
            feedback::show_feedback(
                &error_for_submit,
                "Selectionnez un export .hvb a restaurer.",
            );
            return;
        }

        if !PathBuf::from(&file_path).exists() {
            feedback::show_feedback(
                &error_for_submit,
                "Le fichier .hvb selectionne est introuvable.",
            );
            return;
        }

        if recovery_phrase.split_whitespace().count() != 24 {
            feedback::show_feedback(
                &error_for_submit,
                "La phrase de recuperation doit contenir exactement 24 mots.",
            );
            return;
        }

        if new_password != confirmation {
            feedback::show_feedback(
                &error_for_submit,
                "La confirmation du nouveau mot de passe ne correspond pas.",
            );
            return;
        }

        if strength_bar.last_score() < 3 {
            feedback::show_feedback(
                &error_for_submit,
                "Choisissez un mot de passe principal au moins solide avant de restaurer.",
            );
            return;
        }

        busy_for_submit.set(true);
        feedback::set_pending_state(
            &restore_button_for_submit,
            &restore_spinner_for_submit,
            true,
        );

        let (sender, receiver) = tokio::sync::oneshot::channel();
        let restore_handler = Arc::clone(&on_restore_requested);
        std::thread::spawn(move || {
            let result = restore_handler(PathBuf::from(file_path), recovery_phrase, new_password);
            let _ = sender.send(result);
        });

        let dialog_for_result = dialog_for_submit.clone();
        let error_for_result = error_for_submit.clone();
        let restore_button_for_result = restore_button_for_submit.clone();
        let restore_spinner_for_result = restore_spinner_for_submit.clone();
        let busy_for_result = Rc::clone(&busy_for_submit);
        let on_restore_completed = Rc::clone(&on_restore_completed);
        let update_action_state = Rc::clone(&update_action_state);
        glib::MainContext::default().spawn_local(async move {
            match receiver.await {
                Ok(Ok(())) => {
                    busy_for_result.set(false);
                    feedback::set_pending_state(
                        &restore_button_for_result,
                        &restore_spinner_for_result,
                        false,
                    );

                    let info_dialog = gtk4::MessageDialog::builder()
                        .transient_for(&dialog_for_result)
                        .modal(true)
                        .buttons(gtk4::ButtonsType::Ok)
                        .text("Restauration terminee")
                        .secondary_text("La base a ete restauree et l'application va redemarrer.")
                        .build();
                    let dialog_for_close = dialog_for_result.clone();
                    let on_restore_completed = Rc::clone(&on_restore_completed);
                    info_dialog.connect_response(move |message, _| {
                        message.close();
                        dialog_for_close.close();
                        on_restore_completed();
                    });
                    info_dialog.show();
                }
                Ok(Err(error)) => {
                    busy_for_result.set(false);
                    feedback::set_pending_state(
                        &restore_button_for_result,
                        &restore_spinner_for_result,
                        false,
                    );
                    feedback::show_feedback(&error_for_result, &error.to_string());
                    update_action_state();
                }
                Err(_) => {
                    busy_for_result.set(false);
                    feedback::set_pending_state(
                        &restore_button_for_result,
                        &restore_spinner_for_result,
                        false,
                    );
                    feedback::show_feedback(
                        &error_for_result,
                        "La restauration a ete interrompue avant son terme.",
                    );
                    update_action_state();
                }
            }
        });
    });

    dialog.present();
}
