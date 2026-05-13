use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;
use genpdf::elements;
use genpdf::error::Error as GenpdfError;
use genpdf::fonts::{FontData, FontFamily};
use genpdf::style::{Color, Style};
use genpdf::{Alignment, Context, Element as _, Position};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use thiserror::Error;
use tokio::runtime::Handle;

use crate::models::LicenseTier;
use crate::services::license_service::{AuditSigningError, LicenseService};

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("a Professional or Enterprise license is required")]
    LicenseRequired,
    #[error("audit signing key is missing")]
    SigningKeyMissing,
    #[error("downloads directory not found")]
    DownloadsDirectoryNotFound,
    #[error("failed to initialize PDF fonts: {0}")]
    FontInitialization(String),
    #[error("failed to load audit log entries: {0}")]
    AuditData(String),
    #[error("failed to sign audit report: {0}")]
    Signature(String),
    #[error("failed to render or write PDF: {0}")]
    PdfWrite(String),
}

pub struct GeneratedAuditReport {
    pub path: String,
    pub hash_hex: String,
}

impl GeneratedAuditReport {
    pub fn hash_prefix(&self) -> &str {
        let prefix_len = self.hash_hex.len().min(12);
        &self.hash_hex[..prefix_len]
    }
}

pub struct AuditReportService {
    license_service: Arc<LicenseService>,
    runtime_handle: Handle,
    db_pool: SqlitePool,
}

const LIBERATION_SANS_REGULAR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fonts/LiberationSans-Regular.ttf"
));
const LIBERATION_SANS_BOLD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fonts/LiberationSans-Bold.ttf"
));
const LIBERATION_SANS_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fonts/LiberationSans-Italic.ttf"
));
const LIBERATION_SANS_BOLD_ITALIC: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/fonts/LiberationSans-BoldItalic.ttf"
));
const BRAND_GOLD_DARK: Color = Color::Rgb(143, 112, 51);
const HEADER_PANEL: Color = Color::Rgb(26, 37, 47);

impl AuditReportService {
    pub fn new(
        license_service: Arc<LicenseService>,
        runtime_handle: Handle,
        db_pool: SqlitePool,
    ) -> Self {
        Self {
            license_service,
            runtime_handle,
            db_pool,
        }
    }

    pub fn generate_audit_report(
        &self,
        customer_name: &str,
        days: i64,
    ) -> Result<GeneratedAuditReport, ReportError> {
        // A professional cached license has already passed Ed25519 verification during load.
        let is_premium = self
            .license_service
            .get_cached()
            .map(|license| {
                let tier_label = license.tier.to_string().to_ascii_uppercase();
                matches!(
                    tier_label.as_str(),
                    "PROFESSIONAL" | "PRO" | "ENTERPRISE" | "ENT"
                ) || matches!(license.tier, LicenseTier::Professional)
            })
            .unwrap_or(false);

        if !is_premium {
            return Err(ReportError::LicenseRequired);
        }

        let report_period_days = days.max(1);
        let report_data = self
            .runtime_handle
            .block_on(self.collect_report_data(customer_name, report_period_days))?;

        let fonts = initialize_pdf_fonts()?;

        let mut document = genpdf::Document::new(fonts);
        document.set_title("HeelonVault - Rapport d'audit");
        document.set_paper_size(genpdf::PaperSize::A4);
        let decorator = SignedReportDecorator::new(
            report_data.hash_hex.clone(),
            report_data.signature_b64.clone(),
        );
        document.set_page_decorator(decorator);

        let mut content = elements::LinearLayout::vertical();

        let mut date_line =
            elements::Paragraph::new(format!("Date: {}", Local::now().format("%d/%m/%Y")));
        date_line.set_alignment(Alignment::Right);
        content.push(date_line);
        content.push(
            elements::Paragraph::new(format!(
                "Periode analysee: {}",
                period_label(report_period_days)
            ))
            .styled(
                Style::new()
                    .with_font_size(10)
                    .with_color(Color::Rgb(90, 90, 90)),
            ),
        );
        content.push(
            elements::Paragraph::new(format!(
                "Evenements exportes: {}",
                report_data.audit_entries.len()
            ))
            .styled(
                Style::new()
                    .with_font_size(10)
                    .with_color(Color::Rgb(90, 90, 90)),
            ),
        );
        content.push(elements::Break::new(1));

        content.push(
            elements::Paragraph::new("REGISTRE DE TRAÇABILITÉ DES ACCÈS")
                .aligned(Alignment::Center)
                .styled(
                    Style::new()
                        .with_font_size(18)
                        .with_color(Color::Rgb(0, 0, 0))
                        .bold(),
                ),
        );
        content.push(
            elements::Paragraph::new(format!("Etabli pour : {customer_name}"))
                .aligned(Alignment::Center)
                .styled(
                    Style::new()
                        .with_font_size(11)
                        .with_color(Color::Rgb(0, 0, 0))
                        .italic(),
                ),
        );
        content.push(elements::Break::new(0.8));
        content.push(
            elements::Paragraph::new("JOURNAL D'AUDIT SIGNE").styled(
                Style::new()
                    .with_font_size(14)
                    .with_color(HEADER_PANEL)
                    .bold(),
            ),
        );
        content.push(elements::Break::new(0.35));

        if report_data.audit_entries.is_empty() {
            content.push(
                elements::Paragraph::new("Aucun evenement d'audit sur la periode selectionnee.")
                    .styled(Style::new().italic().with_color(Color::Rgb(90, 90, 90))),
            );
        } else {
            let mut table = elements::TableLayout::new(vec![2, 3, 2, 3, 4]);
            table.set_cell_decorator(elements::FrameCellDecorator::new(true, true, false));

            table
                .row()
                .element(
                    elements::Paragraph::new("Date")
                        .styled(Style::new().bold().with_color(HEADER_PANEL))
                        .padded(1),
                )
                .element(
                    elements::Paragraph::new("Action")
                        .styled(Style::new().bold().with_color(HEADER_PANEL))
                        .padded(1),
                )
                .element(
                    elements::Paragraph::new("Acteur")
                        .styled(Style::new().bold().with_color(HEADER_PANEL))
                        .padded(1),
                )
                .element(
                    elements::Paragraph::new("Cible")
                        .styled(Style::new().bold().with_color(HEADER_PANEL))
                        .padded(1),
                )
                .element(
                    elements::Paragraph::new("Détail")
                        .styled(Style::new().bold().with_color(HEADER_PANEL))
                        .padded(1),
                )
                .push()
                .map_err(|error| ReportError::PdfWrite(error.to_string()))?;

            for entry in &report_data.audit_entries {
                table
                    .row()
                    .element(
                        elements::Paragraph::new(entry.performed_at.as_str())
                            .styled(Style::new().with_font_size(9))
                            .padded(1),
                    )
                    .element(
                        elements::Paragraph::new(entry.action.as_str())
                            .styled(Style::new().with_font_size(9))
                            .padded(1),
                    )
                    .element(
                        elements::Paragraph::new(entry.actor.as_str())
                            .styled(Style::new().with_font_size(9))
                            .padded(1),
                    )
                    .element(
                        elements::Paragraph::new(entry.target.as_str())
                            .styled(Style::new().with_font_size(9))
                            .padded(1),
                    )
                    .element(
                        elements::Paragraph::new(entry.detail.as_str())
                            .styled(Style::new().with_font_size(9))
                            .padded(1),
                    )
                    .push()
                    .map_err(|error| ReportError::PdfWrite(error.to_string()))?;
            }

            content.push(table);
        }

        document.push(content);

        let mut output_path =
            dirs::download_dir().ok_or(ReportError::DownloadsDirectoryNotFound)?;
        output_path.push(format!(
            "HeelonVault_Audit_{}j_{}.pdf",
            report_period_days,
            Local::now().format("%Y%m%d")
        ));

        document
            .render_to_file(&output_path)
            .map_err(|error| ReportError::PdfWrite(error.to_string()))?;

        Ok(GeneratedAuditReport {
            path: path_to_string(output_path),
            hash_hex: report_data.hash_hex,
        })
    }

    async fn collect_report_data(
        &self,
        customer_name: &str,
        days: i64,
    ) -> Result<ReportData, ReportError> {
        let rows = sqlx::query(
            "SELECT
                audit_log.id,
                audit_log.actor_user_id,
                audit_log.action,
                audit_log.target_type,
                audit_log.target_id,
                audit_log.detail,
                audit_log.performed_at,
                users.username AS actor_username,
                     users.display_name AS actor_display_name,
                     vaults.name AS target_vault_name,
                     secret_items.title AS target_secret_title
             FROM audit_log
             LEFT JOIN users ON users.id = audit_log.actor_user_id
                 LEFT JOIN vaults
                     ON audit_log.target_type = 'vault'
                    AND vaults.id = audit_log.target_id
                 LEFT JOIN secret_items
                     ON audit_log.target_type = 'secret'
                    AND secret_items.id = audit_log.target_id
             WHERE performed_at >= datetime('now', ?1)
             ORDER BY performed_at DESC",
        )
        .bind(format!("-{} days", days))
        .fetch_all(&self.db_pool)
        .await
        .map_err(|error| ReportError::AuditData(error.to_string()))?;

        let audit_entries: Vec<AuditEntry> = rows
            .iter()
            .map(|row| {
                let performed_at: String = row.get("performed_at");
                let action: String = row.get("action");
                let actor_user_id: Option<String> = row.get("actor_user_id");
                let actor_username: Option<String> = row.get("actor_username");
                let actor_display_name: Option<String> = row.get("actor_display_name");
                let target_type: Option<String> = row.get("target_type");
                let target_id: Option<String> = row.get("target_id");
                let detail: Option<String> = row.get("detail");
                let target_vault_name: Option<String> = row.get("target_vault_name");
                let target_secret_title: Option<String> = row.get("target_secret_title");

                let actor = resolve_actor_label(
                    actor_user_id.as_deref(),
                    actor_username.as_deref(),
                    actor_display_name.as_deref(),
                );

                AuditEntry {
                    performed_at,
                    action: action.clone(),
                    actor,
                    target: format_target_with_names(
                        target_type.as_deref(),
                        target_id.as_deref(),
                        target_vault_name.as_deref(),
                        target_secret_title.as_deref(),
                    ),
                    detail: format_audit_detail(
                        action.as_str(),
                        detail.as_deref(),
                        target_secret_title.as_deref(),
                    ),
                }
            })
            .collect();

        let mut signed_lines = vec![
            format!("Rapport d'audit HeelonVault pour {customer_name}"),
            format!("Periode analysee: {}", period_label(days)),
            format!(
                "Date de generation: {}",
                Local::now().format("%d/%m/%Y %H:%M:%S")
            ),
            format!("Evenements exportes: {}", audit_entries.len()),
        ];
        signed_lines.extend(audit_entries.iter().map(|entry| {
            format!(
                "[{}] action={} acteur={} cible={} detail={}",
                entry.performed_at, entry.action, entry.actor, entry.target, entry.detail
            )
        }));

        let mut hasher = Sha256::new();
        hasher.update(signed_lines.join("\n").as_bytes());
        let hash_bytes = hasher.finalize().to_vec();
        let hash_hex = hex::encode(&hash_bytes);
        let signature_b64 = self
            .license_service
            .sign_audit_hash(hash_bytes.as_slice())
            .map_err(|error| match error {
                AuditSigningError::LicenseRequired => ReportError::LicenseRequired,
                AuditSigningError::MissingKey(_) => ReportError::SigningKeyMissing,
                AuditSigningError::InvalidKey(message) => ReportError::Signature(message),
            })?;

        Ok(ReportData {
            audit_entries,
            hash_hex,
            signature_b64,
        })
    }
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}

fn initialize_pdf_fonts() -> Result<FontFamily<FontData>, ReportError> {
    Ok(FontFamily {
        regular: FontData::new(LIBERATION_SANS_REGULAR.to_vec(), None)
            .map_err(|error| ReportError::FontInitialization(error.to_string()))?,
        bold: FontData::new(LIBERATION_SANS_BOLD.to_vec(), None)
            .map_err(|error| ReportError::FontInitialization(error.to_string()))?,
        italic: FontData::new(LIBERATION_SANS_ITALIC.to_vec(), None)
            .map_err(|error| ReportError::FontInitialization(error.to_string()))?,
        bold_italic: FontData::new(LIBERATION_SANS_BOLD_ITALIC.to_vec(), None)
            .map_err(|error| ReportError::FontInitialization(error.to_string()))?,
    })
}

fn period_label(days: i64) -> String {
    if days == 1 {
        "les dernieres 24 heures".to_string()
    } else {
        format!("les {} derniers jours", days)
    }
}

fn format_target(target_type: Option<&str>, target_id: Option<&str>) -> String {
    match (target_type, target_id) {
        (Some(target_type), Some(target_id)) => format!("{}:{}", target_type, target_id),
        (Some(target_type), None) => target_type.to_string(),
        (None, Some(target_id)) => target_id.to_string(),
        (None, None) => "-".to_string(),
    }
}

fn format_target_with_names(
    target_type: Option<&str>,
    target_id: Option<&str>,
    target_vault_name: Option<&str>,
    target_secret_title: Option<&str>,
) -> String {
    match target_type {
        Some("vault") => {
            if let Some(name) = target_vault_name
                .map(str::trim)
                .filter(|name| !name.is_empty())
            {
                format!("vault:{}", name)
            } else {
                format_target(target_type, target_id)
            }
        }
        Some("secret") => {
            if let Some(title) = target_secret_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
            {
                format!("secret:{}", title)
            } else {
                format_target(target_type, target_id)
            }
        }
        _ => format_target(target_type, target_id),
    }
}

fn format_audit_detail(
    action: &str,
    raw_detail: Option<&str>,
    target_secret_title: Option<&str>,
) -> String {
    let normalized = normalize_optional_text(raw_detail);
    if normalized != "-" {
        return normalized;
    }

    if action == "secret.created" || action == "secret.updated" || action == "secret.deleted" {
        if let Some(title) = target_secret_title
            .map(str::trim)
            .filter(|title| !title.is_empty())
        {
            return format!("{{\"title\":\"{}\"}}", title);
        }
    }

    normalized
}

struct ReportData {
    audit_entries: Vec<AuditEntry>,
    hash_hex: String,
    signature_b64: String,
}

struct AuditEntry {
    performed_at: String,
    action: String,
    actor: String,
    target: String,
    detail: String,
}

fn resolve_actor_label(
    actor_user_id: Option<&str>,
    actor_username: Option<&str>,
    actor_display_name: Option<&str>,
) -> String {
    if actor_user_id.is_none() {
        return "SYSTEM".to_string();
    }

    if let Some(display_name) = actor_display_name {
        let clean = display_name.trim();
        if !clean.is_empty() {
            return clean.to_string();
        }
    }

    if let Some(username) = actor_username {
        let clean = username.trim();
        if !clean.is_empty() {
            return clean.to_string();
        }
    }

    let fallback_id = actor_user_id.unwrap_or_default();
    if fallback_id.is_empty() {
        "SYSTEM".to_string()
    } else {
        let short: String = fallback_id.chars().take(8).collect();
        format!("Utilisateur ({})", short)
    }
}

fn normalize_optional_text(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("-")
        .to_string()
}

struct SignedReportDecorator {
    hash_hex: String,
    signature_b64: String,
}

impl SignedReportDecorator {
    fn new(hash_hex: String, signature_b64: String) -> Self {
        Self {
            hash_hex,
            signature_b64,
        }
    }
}

impl genpdf::PageDecorator for SignedReportDecorator {
    fn decorate_page<'a>(
        &mut self,
        context: &Context,
        mut area: genpdf::render::Area<'a>,
        style: Style,
    ) -> Result<genpdf::render::Area<'a>, GenpdfError> {
        area.add_margins(10);

        let footer_reserved_height: genpdf::Mm = 14.0.into();
        let line_y = area.size().height - footer_reserved_height;
        let hash_y = line_y + genpdf::Mm::from(2.0);
        let legal_y = line_y + genpdf::Mm::from(5.2);
        let signature_y = line_y + genpdf::Mm::from(8.4);
        let footer_style = style.with_font_size(5).with_color(Color::Rgb(95, 95, 95));

        area.draw_line(
            vec![
                Position::new(0, line_y),
                Position::new(area.size().width, line_y),
            ],
            Style::new().with_color(BRAND_GOLD_DARK),
        );

        let _ = area.print_str(
            &context.font_cache,
            Position::new(0, hash_y),
            footer_style,
            format!("SHA-256: {}", self.hash_hex),
        )?;
        let _ = area.print_str(
            &context.font_cache,
            Position::new(0, legal_y),
            footer_style,
            "Document certifié intègre par HeelonVault.",
        )?;
        let _ = area.print_str(
            &context.font_cache,
            Position::new(0, signature_y),
            footer_style,
            format!("Signature ID: {}", self.signature_b64),
        )?;

        area.set_height(line_y - genpdf::Mm::from(1.0));
        Ok(area)
    }
}

#[cfg(test)]
mod tests {
    use super::{AuditReportService, ReportError};
    use crate::models::{License, LicenseTier};
    use crate::services::license_service::LicenseService;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::{Arc, Mutex, OnceLock};
    use tempfile::tempdir;
    use tokio::runtime::Runtime;

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_license(tier: LicenseTier) -> License {
        License {
            id: "test-license".to_string(),
            customer_name: "LABO TEST".to_string(),
            slots_count: 10,
            expiration_date: "9999-12-31T23:59:59Z".to_string(),
            features: vec!["audit_log".to_string()],
            tier,
        }
    }

    fn setup_service(tier: LicenseTier) -> (Runtime, AuditReportService) {
        let runtime = match Runtime::new() {
            Ok(value) => value,
            Err(err) => panic!("create tokio runtime: {err}"),
        };
        let pool = runtime.block_on(async {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await;
            let pool = match pool {
                Ok(value) => value,
                Err(err) => panic!("connect in-memory sqlite: {err}"),
            };

            sqlx::query(
                "CREATE TABLE audit_log (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    actor_user_id TEXT,
                    action TEXT NOT NULL,
                    target_type TEXT,
                    target_id TEXT,
                    detail TEXT,
                    performed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                )",
            )
            .execute(&pool)
            .await
            .unwrap_or_else(|err| panic!("create audit_log table: {err}"));

            sqlx::query(
                "CREATE TABLE users (
                    id TEXT PRIMARY KEY,
                    username TEXT,
                    display_name TEXT
                )",
            )
            .execute(&pool)
            .await
            .unwrap_or_else(|err| panic!("create users table: {err}"));

            sqlx::query(
                "CREATE TABLE vaults (
                    id TEXT PRIMARY KEY,
                    owner_user_id TEXT NOT NULL,
                    name TEXT NOT NULL
                )",
            )
            .execute(&pool)
            .await
            .unwrap_or_else(|err| panic!("create vaults table: {err}"));

            sqlx::query(
                "CREATE TABLE secret_items (
                    id TEXT PRIMARY KEY,
                    vault_id TEXT NOT NULL,
                    secret_type TEXT NOT NULL,
                    title TEXT,
                    deleted_at TEXT
                )",
            )
            .execute(&pool)
            .await
            .unwrap_or_else(|err| panic!("create secret_items table: {err}"));

            sqlx::query(
                "INSERT INTO users (id, username, display_name)
                 VALUES (?1, ?2, ?3)",
            )
            .bind("user-1")
            .bind("patrick")
            .bind("Patrick")
            .execute(&pool)
            .await
            .unwrap_or_else(|err| panic!("insert user row: {err}"));

            sqlx::query(
                "INSERT INTO audit_log (actor_user_id, action, target_type, target_id, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .bind("user-1")
            .bind("auth.login.success")
            .bind("user")
            .bind("user-1")
            .bind("Login successful")
            .execute(&pool)
            .await
            .unwrap_or_else(|err| panic!("insert audit log row: {err}"));

            pool
        });

        let mut license_service = LicenseService::new();
        license_service.set_cached_license_for_tests(test_license(tier));
        let service =
            AuditReportService::new(Arc::new(license_service), runtime.handle().clone(), pool);
        (runtime, service)
    }

    #[test]
    fn generate_report_requires_certified_license() {
        let _guard = match env_lock().lock() {
            Ok(value) => value,
            Err(_) => panic!("lock env"),
        };
        std::env::remove_var("HEELONVAULT_AUDIT_SIGNING_KEY");
        std::env::remove_var("HEELONVAULT_AUDIT_SIGNING_KEY_PATH");

        let (_runtime, service) = setup_service(LicenseTier::Community);
        let result = service.generate_audit_report("LABO TEST", 7);

        assert!(matches!(result, Err(ReportError::LicenseRequired)));
    }

    #[test]
    fn generate_report_auto_provisions_signing_key_for_pro_license() {
        let _guard = match env_lock().lock() {
            Ok(value) => value,
            Err(_) => panic!("lock env"),
        };
        std::env::remove_var("HEELONVAULT_AUDIT_SIGNING_KEY");
        let temp_dir = match tempdir() {
            Ok(value) => value,
            Err(err) => panic!("create temp dir: {err}"),
        };
        let key_path = temp_dir.path().join("audit.key");
        std::env::set_var("HEELONVAULT_AUDIT_SIGNING_KEY_PATH", &key_path);

        let (_runtime, service) = setup_service(LicenseTier::Professional);
        let result = service.generate_audit_report("LABO TEST", 7);

        assert!(!matches!(result, Err(ReportError::SigningKeyMissing)));
        assert!(
            key_path.exists(),
            "auto-generated signing key should be persisted"
        );

        std::env::remove_var("HEELONVAULT_AUDIT_SIGNING_KEY_PATH");
    }

    #[test]
    fn generate_report_rejects_invalid_signing_key() {
        let _guard = match env_lock().lock() {
            Ok(value) => value,
            Err(_) => panic!("lock env"),
        };
        std::env::set_var("HEELONVAULT_AUDIT_SIGNING_KEY", "cle-invalide");
        std::env::remove_var("HEELONVAULT_AUDIT_SIGNING_KEY_PATH");

        let (_runtime, service) = setup_service(LicenseTier::Professional);
        let result = service.generate_audit_report("LABO TEST", 7);

        assert!(matches!(result, Err(ReportError::Signature(_))));

        std::env::remove_var("HEELONVAULT_AUDIT_SIGNING_KEY");
    }
}
