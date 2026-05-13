use std::rc::Rc;
use std::sync::Arc;

use gtk4::gio;
use gtk4::prelude::*;
use gtk4::{Align, Orientation};
use libadwaita as adw;
use sha2::{Digest, Sha256};

use crate::services::license_service::LicenseService;

pub(super) fn build_certification_menu_item(icon_name: &str, label: &str) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.add_css_class("flat");
    button.add_css_class("main-inline-back-button");
    button.set_halign(Align::Fill);
    button.set_hexpand(true);

    let row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();
    row.append(&gtk4::Image::from_icon_name(icon_name));
    let row_label = gtk4::Label::new(Some(label));
    row_label.set_halign(Align::Start);
    row_label.set_hexpand(true);
    row.append(&row_label);
    button.set_child(Some(&row));
    button
}

pub(super) fn show_certification_diagnostics_dialog(
    parent: &adw::ApplicationWindow,
    license_service: Arc<LicenseService>,
    show_feedback_dialog: Rc<dyn Fn(&adw::ApplicationWindow, &str, &str)>,
) {
    let status = license_service.audit_certification_status();
    let dialog = gtk4::Dialog::builder()
        .transient_for(parent)
        .modal(true)
        .use_header_bar(1)
        .title("Console de Confiance")
        .default_width(780)
        .default_height(620)
        .resizable(false)
        .build();
    dialog.add_css_class("trust-console-dialog");

    let content = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(16)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.set_size_request(720, 560);
    content.add_css_class("trust-console-shell");

    let engine_fingerprint = engine_fingerprint();
    let is_fully_certified = status.is_certified_license && status.signing_key_present;

    let trust_visual = gtk4::Image::from_resource("/com/heelonvault/rust/images/nis2-rgpd.png");
    let parent_width = {
        let width = parent.width();
        if width > 0 {
            width
        } else {
            parent.default_width()
        }
    };
    let trust_visual_size = if parent_width >= 1100 { 128 } else { 104 };
    trust_visual.set_halign(Align::Center);
    trust_visual.set_valign(Align::Center);
    trust_visual.set_pixel_size(trust_visual_size);
    trust_visual.add_css_class("trust-console-hero-image");
    content.append(&trust_visual);

    let status_page = adw::StatusPage::new();
    status_page.add_css_class("trust-console-status");
    if is_fully_certified {
        status_page.add_css_class("trust-console-ready");
    } else {
        status_page.add_css_class("trust-console-pending");
    }
    status_page.set_icon_name(None);
    status_page.set_title("Moteur de Confiance Heelonys");
    status_page.set_description(Some(if is_fully_certified {
        "Le moteur peut générer un rapport signé, traçable et officiellement exploitable."
    } else {
        "Un ou plusieurs prérequis manquent encore pour une chaîne de preuve complète."
    }));
    content.append(&status_page);

    let compliance_badges = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .halign(Align::Center)
        .build();
    let rgpd_badge = gtk4::Label::new(Some("Conforme RGPD"));
    rgpd_badge.add_css_class("trust-compliance-badge");
    rgpd_badge.add_css_class("trust-compliance-rgpd");
    let nis2_badge = gtk4::Label::new(Some("Prêt NIS2"));
    nis2_badge.add_css_class("trust-compliance-badge");
    nis2_badge.add_css_class("trust-compliance-nis2");
    compliance_badges.append(&rgpd_badge);
    compliance_badges.append(&nis2_badge);
    content.append(&compliance_badges);

    let details_list = gtk4::ListBox::new();
    details_list.add_css_class("boxed-list");
    details_list.add_css_class("trust-console-list");
    details_list.set_selection_mode(gtk4::SelectionMode::None);
    details_list.append(&build_certification_status_row(
        "Licence",
        if status.is_certified_license {
            "PRO"
        } else {
            "FREE"
        },
        if status.is_certified_license {
            "Version certifiée active"
        } else {
            "Upgrade requis pour les rapports signés"
        },
        if status.is_certified_license {
            Some("compliance")
        } else {
            Some("muted")
        },
    ));
    details_list.append(&build_certification_status_row(
        "Signature",
        if status.signing_key_present {
            if status.signing_key_auto_generated {
                "ACTIVE (Auto-générée)"
            } else {
                "ACTIVE"
            }
        } else {
            "ABSENTE"
        },
        if status.signing_key_present {
            if status.signing_key_auto_generated {
                "Chaîne de signature opérationnelle, provisionnée localement au premier lancement"
            } else {
                "Chaîne de signature opérationnelle"
            }
        } else {
            "Requis pour la conformité NIS2/RGPD"
        },
        if status.signing_key_present {
            Some("emerald")
        } else {
            Some("warning")
        },
    ));
    details_list.append(&build_certification_status_row(
        "Empreinte Moteur",
        engine_fingerprint.as_str(),
        format!("HeelonVault v{}", env!("CARGO_PKG_VERSION")).as_str(),
        Some("info"),
    ));
    content.append(&details_list);

    let signature_note = gtk4::Label::new(Some(
        "La clé de signature garantit que vos rapports PDF ne peuvent pas être falsifiés après export.",
    ));
    signature_note.set_wrap(true);
    signature_note.set_halign(Align::Start);
    signature_note.add_css_class("dim-label");
    signature_note.add_css_class("trust-console-note");
    content.append(&signature_note);

    if status.is_certified_license && !status.signing_key_present {
        let generate_key_button = gtk4::Button::with_label("Générer une clé maintenant");
        generate_key_button.add_css_class("suggested-action");
        generate_key_button.set_halign(Align::Fill);
        let parent_window = parent.clone();
        let dialog_for_close = dialog.clone();
        let license_service = Arc::clone(&license_service);
        let show_feedback_dialog_for_click = Rc::clone(&show_feedback_dialog);
        generate_key_button.connect_clicked(move |_| match license_service.ensure_audit_key_exists() {
            Ok(_) => {
                dialog_for_close.close();
                (show_feedback_dialog_for_click)(
                    &parent_window,
                    "Clé de certification générée",
                    "Une nouvelle clé Ed25519 a été générée et stockée localement pour les exports certifiés.",
                );
                show_certification_diagnostics_dialog(
                    &parent_window,
                    Arc::clone(&license_service),
                    Rc::clone(&show_feedback_dialog_for_click),
                );
            }
            Err(error) => {
                let message = format!("Impossible de générer la clé de signature: {:?}", error);
                (show_feedback_dialog_for_click)(
                    &parent_window,
                    "Provisionnement impossible",
                    message.as_str(),
                );
            }
        });
        content.append(&generate_key_button);
    }

    if !status.is_certified_license {
        let upgrade_button = gtk4::Button::with_label("Upgrade vers Premium");
        upgrade_button.add_css_class("suggested-action");
        upgrade_button.set_halign(Align::Fill);
        upgrade_button.connect_clicked(|_| {
            let _ = gio::AppInfo::launch_default_for_uri(
                "https://www.heelonys.fr",
                None::<&gio::AppLaunchContext>,
            );
        });
        content.append(&upgrade_button);
    }

    let anssi_note = gtk4::Label::new(Some(
        "Ce moteur utilise le protocole Ed25519 pour garantir la non-répudiation de vos preuves d'audit selon les standards de l'ANSSI.",
    ));
    anssi_note.set_wrap(true);
    anssi_note.set_halign(Align::Start);
    anssi_note.add_css_class("caption");
    anssi_note.add_css_class("trust-console-anssi-note");
    content.append(&anssi_note);

    let action_row = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .halign(Align::Center)
        .spacing(12)
        .margin_top(4)
        .build();
    let ok_button = gtk4::Button::with_label(crate::tr!("common-ok").as_str());
    ok_button.add_css_class("suggested-action");
    ok_button.add_css_class("trust-console-ok");
    let dialog_for_close = dialog.clone();
    ok_button.connect_clicked(move |_| {
        dialog_for_close.close();
    });
    action_row.append(&ok_button);
    content.append(&action_row);

    dialog.content_area().append(&content);
    dialog.present();
}

fn build_certification_status_row(
    title: &str,
    badge_text: &str,
    description: &str,
    badge_kind: Option<&str>,
) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    let container = gtk4::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(12)
        .margin_end(12)
        .build();

    let text_box = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(3)
        .hexpand(true)
        .build();
    let title_label = gtk4::Label::new(Some(title));
    title_label.set_halign(Align::Start);
    title_label.add_css_class("heading");
    let description_label = gtk4::Label::new(Some(description));
    description_label.set_halign(Align::Start);
    description_label.set_wrap(true);
    description_label.add_css_class("dim-label");
    text_box.append(&title_label);
    text_box.append(&description_label);

    let badge = gtk4::Label::new(Some(badge_text));
    badge.add_css_class("header-badge");
    badge.add_css_class("trust-console-badge");
    match badge_kind {
        Some("emerald") => badge.add_css_class("trust-badge-emerald"),
        Some("compliance") => badge.add_css_class("trust-badge-compliance"),
        Some("success") => badge.add_css_class("success"),
        Some("info") => badge.add_css_class("accent"),
        Some("warning") => badge.add_css_class("warning"),
        _ => badge.add_css_class("dim-label"),
    }

    container.append(&text_box);
    container.append(&badge);
    row.set_child(Some(&container));
    row
}

fn engine_fingerprint() -> String {
    let mut hasher = Sha256::new();
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..6]).to_string()
}
