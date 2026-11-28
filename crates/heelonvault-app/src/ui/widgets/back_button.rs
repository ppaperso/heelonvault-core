//! The one "Back" button of the application (3.0 look): a flat button with a left arrow and a
//! label, styled by `page-back-button` only. Every page reached from the sidebar, the secret
//! editor and the two-step sign-in step use it, so the control looks the same everywhere.

use gtk4::prelude::*;

/// Where the button leads, which selects its label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackTarget {
    /// Back to the secret list, from any page of the main window.
    Page,
    /// Back to the credentials, from the two-step sign-in step.
    Credentials,
}

impl BackTarget {
    // Literal keys, so the i18n usage test checks that both catalogs define them.
    fn label(self) -> String {
        match self {
            Self::Page => heelonvault_core::i18n::tr("main-view-back"),
            Self::Credentials => heelonvault_core::i18n::tr("login-totp-back"),
        }
    }
}

/// A back button and its target, kept for live retranslation.
#[derive(Clone)]
pub struct BackButton {
    button: gtk4::Button,
    label: gtk4::Label,
    target: BackTarget,
}

impl BackButton {
    /// Build the button for `target`. The arrow is an icon, never part of the text.
    pub fn new(target: BackTarget) -> Self {
        let label = gtk4::Label::new(Some(target.label().as_str()));
        let content = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        content.append(&gtk4::Image::from_icon_name("hv-back-symbolic"));
        content.append(&label);

        let button = gtk4::Button::new();
        button.set_child(Some(&content));
        button.add_css_class("flat");
        button.add_css_class("page-back-button");
        button.set_halign(gtk4::Align::Start);
        button.set_valign(gtk4::Align::Center);
        Self {
            button,
            label,
            target,
        }
    }

    /// The underlying button, to connect `clicked` or place it in a layout.
    pub fn button(&self) -> &gtk4::Button {
        &self.button
    }

    /// Re-apply the label in the current language without replacing the arrow.
    pub fn refresh_i18n(&self) {
        self.label.set_text(self.target.label().as_str());
    }
}

/// Native check: the arrow survives live retranslation (a `set_label` used to replace it).
#[cfg(test)]
pub(crate) fn native_back_button_scenario() -> Result<(), Box<dyn std::error::Error>> {
    fn parts(back: &BackButton) -> Result<(bool, String), Box<dyn std::error::Error>> {
        let content = back
            .button()
            .child()
            .and_downcast::<gtk4::Box>()
            .ok_or("back button content is not a box")?;
        let icon = content
            .first_child()
            .and_downcast::<gtk4::Image>()
            .is_some_and(|image| image.icon_name().as_deref() == Some("hv-back-symbolic"));
        let text = content
            .last_child()
            .and_downcast::<gtk4::Label>()
            .ok_or("back button has no label")?
            .text()
            .to_string();
        Ok((icon, text))
    }

    for target in [BackTarget::Page, BackTarget::Credentials] {
        let back = BackButton::new(target);
        assert!(back.button().has_css_class("flat"));
        assert!(back.button().has_css_class("page-back-button"));
        assert!(heelonvault_core::i18n::set_language("fr"));
        back.refresh_i18n();
        let (icon, french) = parts(&back)?;
        assert!(icon, "the arrow is an icon");
        assert!(!french.contains('←'), "no arrow in the text");
        assert!(heelonvault_core::i18n::set_language("en"));
        back.refresh_i18n();
        let (icon, english) = parts(&back)?;
        assert!(icon, "retranslation keeps the arrow");
        assert_ne!(french, english, "the label follows the language");
    }
    Ok(())
}
