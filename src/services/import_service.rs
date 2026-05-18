use std::path::Path;
use std::sync::{mpsc::Sender as ProgressSender, Arc};

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

#[trait_variant::make(ImportService: Send)]
pub trait LocalImportService {
    fn preview_csv(&self, csv_file_path: &Path) -> Result<ImportCsvPreview, AppError>;

    async fn import_csv<TSecret, TVault>(
        &self,
        csv_file_path: &Path,
        admin_user_id: Uuid,
        admin_master_key: SecretBox<Vec<u8>>,
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

    fn parse_csv_rows(csv_file_path: &Path) -> Result<CsvParseOutcome, AppError> {
        Self::validate_csv_file_size(csv_file_path)?;

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(csv_file_path)
            .map_err(|error| AppError::Storage(format!("failed to open csv file: {error}")))?;

        let headers = reader
            .headers()
            .map_err(|error| AppError::Validation(format!("failed to read csv headers: {error}")))?
            .clone();

        let required = ["name", "url", "username", "password", "notes"];
        for field in required {
            if !headers.iter().any(|h| h.eq_ignore_ascii_case(field)) {
                return Err(AppError::Validation(format!(
                    "csv is missing required column: {field}"
                )));
            }
        }

        let idx = |name: &str| -> Result<usize, AppError> {
            headers
                .iter()
                .position(|h| h.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    AppError::Validation(format!("csv is missing required column: {name}"))
                })
        };

        let name_idx = idx("name")?;
        let url_idx = idx("url")?;
        let username_idx = idx("username")?;
        let password_idx = idx("password")?;
        let notes_idx = idx("notes")?;

        let mut rows = Vec::new();
        let mut failures = Vec::new();
        for (row_number, record) in reader.records().enumerate() {
            if rows.len() + failures.len() >= MAX_CSV_ROWS {
                break;
            }

            let csv_row = row_number + 2;
            let record = match record {
                Ok(record) => record,
                Err(error) => {
                    failures.push(ImportCsvFailure {
                        source_row: csv_row,
                        title: "Imported secret".to_string(),
                        reason: format!("invalid csv record: {error}"),
                    });
                    continue;
                }
            };

            let name = match Self::required_field(&record, name_idx, "name", csv_row) {
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
            let url = match Self::required_field(&record, url_idx, "url", csv_row) {
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
            let username = match Self::required_field(&record, username_idx, "username", csv_row) {
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
            let password = match Self::required_field(&record, password_idx, "password", csv_row) {
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
            let notes = match Self::required_field(&record, notes_idx, "notes", csv_row) {
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

            if let Err(error) = Self::validate_field_length("name", name.as_str())
                .and_then(|_| Self::validate_field_length("url", url.as_str()))
                .and_then(|_| Self::validate_field_length("username", username.as_str()))
                .and_then(|_| Self::validate_field_length("password", password.as_str()))
                .and_then(|_| Self::validate_field_length("notes", notes.as_str()))
                .and_then(|_| Self::validate_optional_url(url.as_str()))
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
            });
        }

        Ok(CsvParseOutcome {
            total_rows: rows.len() + failures.len(),
            rows,
            failures,
        })
    }

    fn required_field(
        record: &csv::StringRecord,
        index: usize,
        field_name: &str,
        csv_row: usize,
    ) -> Result<String, AppError> {
        record
            .get(index)
            .map(|value| value.trim().to_string())
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "csv row {csv_row} is missing required field: {field_name}"
                ))
            })
    }
}

struct CsvParseOutcome {
    total_rows: usize,
    rows: Vec<CsvRow>,
    failures: Vec<ImportCsvFailure>,
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
            });
        }

        let vaults = vault_service.list_user_vaults(admin_user_id).await.map_err(|error| {
            error!(admin_user_id = %admin_user_id, error = %error, "csv import failed while listing vaults");
            error
        })?;
        let first_vault = vaults
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("no vault found for csv import".to_string()))?;

        let vault_key = vault_service
            .open_vault(
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
        let mut processed_count = failures.len();

        for row in outcome.rows {
            let secret_title = row.title_display();
            let password_bytes = row.password.into_bytes();
            let metadata = serde_json::to_string(&CsvImportMetadata {
                login: row.username,
                url: row.url,
                notes: row.notes,
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
                    Some("import,csv".to_string()),
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

            processed_count += 1;
            if let Some(progress) = progress.as_ref() {
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
    source: &'static str,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::ImportServiceImpl;

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
            "name,url,username,password,notes\nExample,ftp://invalid.local,alice,pw,test\n",
        );

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(parsed.is_ok(), "invalid URL scheme should not abort preview");
        let outcome = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(outcome.rows.is_empty(), "invalid URL rows should not be importable");
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
            "name,url,username,password,notes\nExample,https://example.com,alice,pw,test\nExample2,http://example.org,bob,pw2,test2\n",
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
    fn parse_csv_rows_rejects_truncated_record_missing_required_column_value() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let path = temp.path().join("missing_value.csv");
        write_csv(
            path.as_path(),
            "name,url,username,password,notes\nExample,https://example.com,alice,pw\n",
        );

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(parsed.is_ok(), "truncated records should not abort preview");
        let outcome = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(outcome.rows.is_empty());
        assert_eq!(outcome.failures.len(), 1);
    }

    #[test]
    fn parse_csv_rows_skips_empty_password_but_not_missing_password_field() {
        let temp = tempfile::tempdir();
        assert!(temp.is_ok(), "tempdir should be created");
        let temp = match temp {
            Ok(value) => value,
            Err(_) => return,
        };

        let path = temp.path().join("empty_password.csv");
        write_csv(
            path.as_path(),
            "name,url,username,password,notes\nExample,https://example.com,alice,,note\n",
        );

        let parsed = ImportServiceImpl::parse_csv_rows(path.as_path());
        assert!(
            parsed.is_ok(),
            "empty password should be handled as skip, not error"
        );
        let rows = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(rows.rows.is_empty(), "rows with empty password should be skipped");
        assert_eq!(rows.failures.len(), 1);
    }
}
