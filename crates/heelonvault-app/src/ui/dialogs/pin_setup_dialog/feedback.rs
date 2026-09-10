use gtk4::prelude::*;

/// Affiche un message de feedback sur le label.
///
/// # Arguments
/// * `label` - Le label GTK sur lequel afficher le feedback
/// * `msg` - Le message a afficher
/// * `is_error` - Si vrai, applique le style error, sinon le style success
pub fn show_feedback(label: &gtk4::Label, msg: &str, is_error: bool) {
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

/// Cache le feedback du label.
///
/// # Arguments
/// * `label` - Le label GTK dont on veut cacher le feedback
pub fn hide_feedback(label: &gtk4::Label) {
    label.set_text("");
    label.set_visible(false);
}
