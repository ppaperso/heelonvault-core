use std::fs;
use std::path::Path;
use std::sync::{mpsc::Sender as ProgressSender, Arc};

use chrono::Local;
use secrecy::{ExposeSecret, SecretBox};
use serde::Serialize;
use tracing::{error, info};
use url::Url;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::SecretType;
use crate::services::secret_service::SecretService;
use crate::services::vault_service::VaultService;

#[derive(Debug, Clone)]
pub struct ImportCsvFailure {
    pub source_row: usize,
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ImportCsvPreview {
    pub total_rows: usize,
    pub importable_rows: usize,
    pub failed_rows: usize,
}

#[derive(Debug, Clone)]
pub struct ImportCsvReport {
    pub total_rows: usize,
    pub imported: usize,
    pub failed: usize,
    pub failures: Vec<ImportCsvFailure>,
    pub reject_report_path: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ImportProgressEvent {
    Started {
        total_rows: usize,
        importable_rows: usize,
        failed_rows: usize,
    },
    Progress {
        processed: usize,
        total_rows: usize,
        imported: usize,
        failed: usize,
        current_title: Option<String>,
    },
}

#[allow(clippy::too_many_arguments)]
#[trait_variant::make(ImportService: Send)]
pub trait LocalImportService {
    fn preview_csv(&self, csv_file_path: &Path) -> Result<ImportCsvPreview, AppError>;

    #[allow(clippy::too_many_arguments)]
    async fn import_csv<TSecret, TVault>(
        &self,
        csv_file_path: &Path,
        admin_user_id: Uuid,
        admin_master_key: SecretBox<Vec<u8>>,
        target_vault_id: Option<Uuid>,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
        progress: Option<ProgressSender<ImportProgressEvent>>,
    ) -> Result<ImportCsvReport, AppError>
    where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static;
}

pub struct ImportServiceImpl;

const MAX_CSV_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_CSV_ROWS: usize = 10_000;
const MAX_FIELD_LEN: usize = 2048;
const EXPECTED_CSV_COLUMNS: usize = 7;

impl ImportServiceImpl {
    pub fn new() -> Self {
        Self
    }

    pub fn preview_csv(csv_file_path: &Path) -> Result<ImportCsvPreview, AppError> {
        let outcome = Self::parse_csv_rows(csv_file_path)?;
        Ok(ImportCsvPreview {
            total_rows: outcome.total_rows,
            importable_rows: outcome.rows.len(),
            failed_rows: outcome.failures.len(),
        })
    }

    fn validate_csv_file_size(csv_file_path: &Path) -> Result<(), AppError> {
        let metadata = std::fs::metadata(csv_file_path)
            .map_err(|error| AppError::Storage(format!("failed to stat csv file: {error}")))?;
        if metadata.len() > MAX_CSV_FILE_SIZE_BYTES {
            return Err(AppError::Validation(format!(
                "csv file is too large (max {} bytes)",
                MAX_CSV_FILE_SIZE_BYTES
            )));
        }
        Ok(())
    }

    fn validate_field_length(name: &str, value: &str) -> Result<(), AppError> {
        if value.len() > MAX_FIELD_LEN {
            return Err(AppError::Validation(format!(
                "csv field '{name}' exceeds maximum length of {MAX_FIELD_LEN}"
            )));
        }
        Ok(())
    }

    fn validate_optional_url(value: &str) -> Result<(), AppError> {
        if value.is_empty() {
            return Ok(());
        }

        let parsed = Url::parse(value)
            .map_err(|error| AppError::Validation(format!("invalid url format in csv: {error}")))?;
        match parsed.scheme() {
            "http" | "https" => Ok(()),
            _ => Err(AppError::Validation(
                "url must start with http:// or https://".to_string(),
            )),
        }
    }

    fn read_csv_source(csv_file_path: &Path) -> Result<String, AppError> {
        let bytes = fs::read(csv_file_path)
            .map_err(|error| AppError::Storage(format!("failed to read csv file: {error}")))?;

        if let Ok(text) = String::from_utf8(bytes.clone()) {
            return Ok(text);
        }

        let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
        Ok(decoded.into_owned())
    }

    fn normalize_header(header: &str) -> String {
        header.trim_start_matches('\u{feff}').trim().to_string()
    }

    fn parse_csv_rows(csv_file_path: &Path) -> Result<CsvParseOutcome, AppError> {
        Self::validate_csv_file_size(csv_file_path)?;

        let csv_source = Self::read_csv_source(csv_file_path)?;
        let source_lines: Vec<&str> = csv_source.lines().collect();

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(csv_source.as_bytes());

        let headers = reader
            .headers()
            .map_err(|error| AppError::Validation(format!("failed to read csv headers: {error}")))?
            .clone();

        let normalized_headers: Vec<String> = headers
            .iter()
            .map(|header: &str| Self::normalize_header(header))
            .collect();

        let required = [
            "name", "url", "username", "password", "notes", "category", "tags",
        ];
        for field in required {
            if !normalized_headers
                .iter()
                .any(|h: &String| h.eq_ignore_ascii_case(field))
            {
                return Err(AppError::Validation(format!(
                    "csv is missing required column: {field}"
                )));
            }
        }

        let idx = |name: &str| -> Result<usize, AppError> {
            normalized_headers
                .iter()
                .position(|h: &String| h.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    AppError::Validation(format!("csv is missing required column: {name}"))
                })
        };

        let name_idx = idx("name")?;
        let url_idx = idx("url")?;
        let username_idx = idx("username")?;
        let password_idx = idx("password")?;
        let notes_idx = idx("notes")?;

        let category_idx = idx("category")?;
        let tags_idx = idx("tags")?;

        let mut rows = Vec::new();
        let mut failures = Vec::new();
        let mut rejects = Vec::new();
        for (row_number, record) in reader.records().enumerate() {
            if rows.len() + failures.len() >= MAX_CSV_ROWS {
                break;
            }

            let fallback_row = row_number + 2;
            let record = match record {
                Ok(record) => record,
                Err(error) => {
                    let csv_row = error
                        .position()
                        .and_then(|position| usize::try_from(position.line()).ok())
                        .unwrap_or(fallback_row);
                    let raw_line = source_lines
                        .get(csv_row.saturating_sub(1))
                        .map(|line| (*line).to_string());
                    let reason = format!("parsing invalid: {error}");
                    failures.push(ImportCsvFailure {
                        source_row: csv_row,
                        title: "Imported secret".to_string(),
                        reason: reason.clone(),
                    });
                    rejects.push(CsvRejectDetail {
                        source_row: csv_row,
                        reject_type: CsvRejectType::ParsingInvalid,
                        reason,
                        detected_columns: None,
                        raw_line,
                    });
                    continue;
                }
            };

            let csv_row = fallback_row;
            if record.len() != EXPECTED_CSV_COLUMNS {
                let reason = format!(
                    "wrong column count: expected {EXPECTED_CSV_COLUMNS}, got {}",
                    record.len()
                );
                failures.push(ImportCsvFailure {
                    source_row: csv_row,
                    title: "Imported secret".to_string(),
                    reason: reason.clone(),
                });
                rejects.push(CsvRejectDetail {
                    source_row: csv_row,
                    reject_type: CsvRejectType::WrongColumnCount,
                    reason,
                    detected_columns: Some(record.len()),
                    raw_line: source_lines
                        .get(csv_row.saturating_sub(1))
                        .map(|line| (*line).to_string()),
                });
                continue;
            }

            let name = match Self::required_non_empty_field(&record, name_idx, "name", csv_row) {
                Ok(value) => value,
                Err(error) => {
                    failures.push(ImportCsvFailure {
                        source_row: csv_row,
                        title: "Imported secret".to_string(),
                        reason: error.to_string(),
                    });
                    continue;
                }
            };
            let url = Self::optional_field(&record, url_idx);
            let username =
                match Self::required_non_empty_field(&record, username_idx, "username", csv_row) {
                    Ok(value) => value,
                    Err(error) => {
                        failures.push(ImportCsvFailure {
                            source_row: csv_row,
                            title: name.clone(),
                            reason: error.to_string(),
                        });
                        continue;
                    }
                };
            let password =
                match Self::required_non_empty_field(&record, password_idx, "password", csv_row) {
                    Ok(value) => value,
                    Err(error) => {
                        failures.push(ImportCsvFailure {
                            source_row: csv_row,
                            title: name.clone(),
                            reason: error.to_string(),
                        });
                        continue;
                    }
                };
            let notes = Self::optional_field(&record, notes_idx);
            let category = Self::optional_text(&record, category_idx);
            let tags = Self::optional_text(&record, tags_idx);

            if let Err(error) = Self::validate_field_length("name", name.as_str())
                .and_then(|_| Self::validate_field_length("url", url.as_str()))
                .and_then(|_| Self::validate_field_length("username", username.as_str()))
                .and_then(|_| Self::validate_field_length("password", password.as_str()))
                .and_then(|_| Self::validate_field_length("notes", notes.as_str()))
                .and_then(|_| Self::validate_optional_url(url.as_str()))
                .and_then(|_| {
                    category
                        .as_deref()
                        .map_or(Ok(()), |c| Self::validate_field_length("category", c))
                })
                .and_then(|_| {
                    tags.as_deref()
                        .map_or(Ok(()), |t| Self::validate_field_length("tags", t))
                })
            {
                failures.push(ImportCsvFailure {
                    source_row: csv_row,
                    title: name.clone(),
                    reason: error.to_string(),
                });
                continue;
            }

            if password.is_empty() {
                failures.push(ImportCsvFailure {
                    source_row: csv_row,
                    title: name.clone(),
                    reason: "password is empty".to_string(),
                });
                continue;
            }

            rows.push(CsvRow {
                source_row: csv_row,
                name,
                url,
                username,
                password,
                notes,
                category,
                tags,
            });
        }

        Ok(CsvParseOutcome {
            total_rows: rows.len() + failures.len(),
            rows,
            failures,
            rejects,
        })
    }

    fn resolve_reject_report_dir() -> Result<std::path::PathBuf, AppError> {
        if let Some(path_raw) = std::env::var("HEELONVAULT_LOG_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Ok(std::path::PathBuf::from(path_raw));
        }

        let current_dir = std::env::current_dir().map_err(|error| {
            AppError::Storage(format!("failed to resolve current directory: {error}"))
        })?;
        Ok(current_dir.join("logs"))
    }

    fn write_reject_report(
        csv_file_path: &Path,
        rejects: &[CsvRejectDetail],
    ) -> Result<Option<String>, AppError> {
        let report_dir = Self::resolve_reject_report_dir()?;
        Self::write_reject_report_to_dir(report_dir.as_path(), csv_file_path, rejects)
    }

    fn write_reject_report_to_dir(
        report_dir: &Path,
        csv_file_path: &Path,
        rejects: &[CsvRejectDetail],
    ) -> Result<Option<String>, AppError> {
        if rejects.is_empty() {
            return Ok(None);
        }

        fs::create_dir_all(report_dir).map_err(|error| {
            AppError::Storage(format!(
                "failed to create csv reject report directory {}: {error}",
                report_dir.display()
            ))
        })?;

        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let report_path = report_dir.join(format!("csv_import_rejects_{timestamp}.txt"));

        let mut lines = vec![
            "HeelonVault CSV Import Reject Report".to_string(),
            format!("Generated at: {}", Local::now().to_rfc3339()),
            format!("Source file: {}", csv_file_path.display()),
            format!("Rejected rows: {}", rejects.len()),
            String::new(),
        ];

        for reject in rejects {
            lines.push(format!("- CSV row {}", reject.source_row));
            lines.push(format!("  Type: {}", reject.reject_type.label()));
            lines.push(format!("  Reason: {}", reject.reason));
            if let Some(columns) = reject.detected_columns {
                lines.push(format!("  Detected columns: {columns}"));
            }
            if let Some(raw_line) = reject.raw_line.as_deref() {
                lines.push(format!("  Raw line: {raw_line}"));
            } else {
                lines.push("  Raw line: <unavailable>".to_string());
            }
            lines.push(String::new());
        }

        fs::write(report_path.as_path(), lines.join("\n")).map_err(|error| {
            AppError::Storage(format!(
                "failed to write csv reject report {}: {error}",
                report_path.display()
            ))
        })?;

        Ok(Some(report_path.display().to_string()))
    }

    fn optional_field(record: &csv::StringRecord, index: usize) -> String {
        record
            .get(index)
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    }

    fn optional_text(record: &csv::StringRecord, index: usize) -> Option<String> {
        record
            .get(index)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn required_non_empty_field(
        record: &csv::StringRecord,
        index: usize,
        field_name: &str,
        csv_row: usize,
    ) -> Result<String, AppError> {
        let value = record
            .get(index)
            .map(|value| value.trim().to_string())
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "csv row {csv_row} is missing required field: {field_name}"
                ))
            })?;

        if value.is_empty() {
            return Err(AppError::Validation(format!(
                "csv row {csv_row} has an empty required field: {field_name}"
            )));
        }

        Ok(value)
    }
}

struct CsvParseOutcome {
    total_rows: usize,
    rows: Vec<CsvRow>,
    failures: Vec<ImportCsvFailure>,
    rejects: Vec<CsvRejectDetail>,
}

#[derive(Clone)]
struct CsvRejectDetail {
    source_row: usize,
    reject_type: CsvRejectType,
    reason: String,
    detected_columns: Option<usize>,
    raw_line: Option<String>,
}

#[derive(Clone, Copy)]
enum CsvRejectType {
    ParsingInvalid,
    WrongColumnCount,
}

impl CsvRejectType {
    fn label(&self) -> &'static str {
        match self {
            Self::ParsingInvalid => "Parsing invalid",
            Self::WrongColumnCount => "Wrong column count",
        }
    }
}

impl Default for ImportServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportService for ImportServiceImpl {
    fn preview_csv(&self, csv_file_path: &Path) -> Result<ImportCsvPreview, AppError> {
        Self::preview_csv(csv_file_path)
    }

    async fn import_csv<TSecret, TVault>(
        &self,
        csv_file_path: &Path,
        admin_user_id: Uuid,
        admin_master_key: SecretBox<Vec<u8>>,
        target_vault_id: Option<Uuid>,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
        progress: Option<ProgressSender<ImportProgressEvent>>,
    ) -> Result<ImportCsvReport, AppError>
    where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        let outcome = match Self::parse_csv_rows(csv_file_path) {
            Ok(rows) => rows,
            Err(error) => {
                error!(csv_file = %csv_file_path.display(), error = %error, "csv import parsing failed");
                return Err(error);
            }
        };
        if let Some(progress) = progress.as_ref() {
            let _ = progress.send(ImportProgressEvent::Started {
                total_rows: outcome.total_rows,
                importable_rows: outcome.rows.len(),
                failed_rows: outcome.failures.len(),
            });
        }

        if outcome.rows.is_empty() && outcome.failures.is_empty() {
            return Ok(ImportCsvReport {
                total_rows: 0,
                imported: 0,
                failed: 0,
                failures: vec![],
                reject_report_path: None,
            });
        }

        let reject_report_path =
            Self::write_reject_report(csv_file_path, outcome.rejects.as_slice())?;

        let vaults = vault_service.list_user_vaults(admin_user_id).await.map_err(|error| {
            error!(admin_user_id = %admin_user_id, error = %error, "csv import failed while listing vaults");
            error
        })?;
        let first_vault = if let Some(vault_id) = target_vault_id {
            vaults
                .into_iter()
                .find(|v| v.id == vault_id)
                .ok_or_else(|| {
                    AppError::NotFound("specified vault not found or not accessible".to_string())
                })?
        } else {
            vaults
                .into_iter()
                .next()
                .ok_or_else(|| AppError::NotFound("no vault found for csv import".to_string()))?
        };

        let vault_key = vault_service
            .open_vault_for_user(
                admin_user_id,
                first_vault.id,
                SecretBox::new(Box::new(admin_master_key.expose_secret().clone())),
            )
            .await
            .map_err(|error| {
                error!(vault_id = %first_vault.id, error = %error, "csv import failed while opening vault");
                error
            })?;

        let mut imported_count = 0usize;
        let mut failures = outcome.failures;
        let initial_processed_offset = failures.len();

        for (row_index, row) in outcome.rows.into_iter().enumerate() {
            let secret_title = row.title_display();
            let password_bytes = row.password.into_bytes();
            let metadata = serde_json::to_string(&CsvImportMetadata {
                login: row.username,
                url: row.url,
                notes: row.notes,
                category: row.category.unwrap_or_default(),
                source: "csv_import",
            })
            .map_err(|error| {
                AppError::Validation(format!("failed to serialize csv metadata: {error}"))
            })?;

            let title = if row.name.is_empty() {
                Some("Imported secret".to_string())
            } else {
                Some(row.name)
            };

            let create_result = secret_service
                .create_secret(
                    first_vault.id,
                    SecretType::Password,
                    title,
                    Some(metadata),
                    row.tags,
                    None,
                    SecretBox::new(Box::new(password_bytes)),
                    SecretBox::new(Box::new(vault_key.expose_secret().clone())),
                )
                .await;

            match create_result {
                Ok(_) => {
                    imported_count += 1;
                }
                Err(error) => {
                    error!(
                        vault_id = %first_vault.id,
                        source_row = row.source_row,
                        secret_title = secret_title,
                        error = %error,
                        "csv import failed while creating secret"
                    );
                    failures.push(ImportCsvFailure {
                        source_row: row.source_row,
                        title: secret_title.clone(),
                        reason: error.to_string(),
                    });
                }
            }

            if let Some(progress) = progress.as_ref() {
                let processed_count = initial_processed_offset + row_index + 1;
                let _ = progress.send(ImportProgressEvent::Progress {
                    processed: processed_count,
                    total_rows: outcome.total_rows,
                    imported: imported_count,
                    failed: failures.len(),
                    current_title: Some(secret_title),
                });
            }
        }

        info!(
            imported_count,
            failed_count = failures.len(),
            vault_id = %first_vault.id,
            "csv import completed successfully"
        );
        Ok(ImportCsvReport {
            total_rows: outcome.total_rows,
            imported: imported_count,
            failed: failures.len(),
            failures,
            reject_report_path,
        })
    }
}

struct CsvRow {
    source_row: usize,
    name: String,
    url: String,
    username: String,
    password: String,
    notes: String,
    category: Option<String>,
    tags: Option<String>,
}

impl CsvRow {
    fn title_display(&self) -> String {
        if self.name.is_empty() {
            "Imported secret".to_string()
        } else {
            self.name.clone()
        }
    }
}

#[derive(Serialize)]
struct CsvImportMetadata {
    login: String,
    url: String,
    notes: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    category: String,
    source: &'static str,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{CsvRejectDetail, CsvRejectType, ImportServiceImpl};

    fn write_csv(path: &Path, body: &str) {
        let write_result = fs::write(path, body.as_bytes());
        assert!(write_result.is_ok(), "csv fixture write should succeed");
    }

    #[test]
    fn parse_csv_rows_rejects_invalid_url() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let path = temp.path().join("bad_url.csv");
        write_csv(
            path.as_path(),
            "name,url,username,password,notes,category,tags\nExample,ftp://invalid.local,alice,pw,test,,\n",
        );

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(
            parsed.is_ok(),
            "invalid URL scheme should not abort preview"
        );
        let outcome = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(
            outcome.rows.is_empty(),
            "invalid URL rows should not be importable"
        );
        assert_eq!(outcome.failures.len(), 1);
    }

    #[test]
    fn parse_csv_rows_accepts_http_and_https() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let path = temp.path().join("good_urls.csv");
        write_csv(
            path.as_path(),
            "name,url,username,password,notes,category,tags\nExample,https://example.com,alice,pw,test,,\nExample2,http://example.org,bob,pw2,test2,,\n",
        );

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(parsed.is_ok(), "http/https URLs should be accepted");
        let outcome = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(outcome.rows.len(), 2);
        assert!(outcome.failures.is_empty());
    }

    #[test]
    fn parse_csv_rows_rejects_wrong_column_count() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let path = temp.path().join("missing_value.csv");
        write_csv(
            path.as_path(),
            "name,url,username,password,notes,category,tags\nExample,https://example.com,alice,pw\n",
        );

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(parsed.is_ok(), "malformed records should not abort preview");
        let outcome = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(outcome.rows.len(), 0);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].reason.contains("wrong column count"));
    }

    #[test]
    fn parse_csv_rows_rejects_empty_required_fields_and_accepts_optional_empties() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let path = temp.path().join("mixed_fields.csv");
        write_csv(
            path.as_path(),
            "name,url,username,password,notes,category,tags\nExample,https://example.com,alice,pw,note,,\n,,user2,pw2,note2,,\nExample3,,user3,pw3,,,\n",
        );

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(parsed.is_ok(), "optional empties should not abort preview");
        let rows = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(rows.rows.len(), 2);
        assert_eq!(rows.failures.len(), 1);
    }

    #[test]
    fn parse_csv_rows_accepts_quoted_commas_and_quotes() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let path = temp.path().join("quoted_fields.csv");
        write_csv(
            path.as_path(),
            "name,url,username,password,notes,category,tags\nExample,https://example.com,alice,\"pa,ss \"\"quoted\"\" 'test'\",\"note,with,comma\",cat,\"tag1,tag2\"\n",
        );

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(parsed.is_ok(), "quoted csv fields should parse correctly");
        let outcome = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(outcome.rows.len(), 1);
        assert_eq!(outcome.failures.len(), 0);
        let row = &outcome.rows[0];
        assert_eq!(row.password, "pa,ss \"quoted\" 'test'");
        assert_eq!(row.notes, "note,with,comma");
        assert_eq!(row.category.as_deref(), Some("cat"));
        assert_eq!(row.tags.as_deref(), Some("tag1,tag2"));
    }

    #[test]
    fn parse_csv_rows_decodes_iso_8859_1_input() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let path = temp.path().join("latin1.csv");
        let bytes = b"name,url,username,password,notes,category,tags\nAccorHotel_Gmail,https://all.accor.com/,patrickp44@orange.fr,m%Z-d#GT6Hcv5SAnZ,,h\xf4tel,accor\n";
        let write_result = fs::write(path.as_path(), bytes.as_slice());
        assert!(write_result.is_ok(), "latin1 fixture write should succeed");

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(parsed.is_ok(), "latin1 csv should be decoded successfully");
        let outcome = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(outcome.rows.len(), 1);
        assert!(outcome.failures.is_empty());
        let row = &outcome.rows[0];
        assert_eq!(row.category.as_deref(), Some("hôtel"));
        assert_eq!(row.tags.as_deref(), Some("accor"));
    }

    #[test]
    fn write_reject_report_includes_reject_type_and_raw_line() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let report_path = ImportServiceImpl::write_reject_report_to_dir(
            temp.path(),
            Path::new("sample.csv"),
            &[CsvRejectDetail {
                source_row: 3,
                reject_type: CsvRejectType::WrongColumnCount,
                reason: "wrong column count: expected 7, got 8".to_string(),
                detected_columns: Some(8),
                raw_line: Some("A,B,C,D,E,F,G,H".to_string()),
            }],
        );
        assert!(report_path.is_ok(), "report creation should succeed");

        let report_path = match report_path {
            Ok(Some(value)) => value,
            _ => return,
        };
        let content = fs::read_to_string(report_path);
        assert!(content.is_ok(), "report should be readable");
        let content = match content {
            Ok(value) => value,
            Err(_) => return,
        };

        assert!(content.contains("Type: Wrong column count"));
        assert!(content.contains("Raw line: A,B,C,D,E,F,G,H"));
    }
}
