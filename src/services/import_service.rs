use std::path::Path;
use std::sync::Arc;

use serde::Serialize;
use secrecy::{ExposeSecret, SecretBox};
use tracing::info;
use url::Url;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::SecretType;
use crate::services::secret_service::SecretService;
use crate::services::vault_service::VaultService;

#[trait_variant::make(ImportService: Send)]
pub trait LocalImportService {
    async fn import_csv<TSecret, TVault>(
        &self,
        csv_file_path: &Path,
        admin_user_id: Uuid,
        admin_master_key: SecretBox<Vec<u8>>,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
    ) -> Result<usize, AppError>
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

    fn parse_csv_rows(csv_file_path: &Path) -> Result<Vec<CsvRow>, AppError> {
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
        for (row_number, record) in reader.records().enumerate() {
            let record = record
                .map_err(|error| AppError::Validation(format!("invalid csv record: {error}")))?;

            if rows.len() >= MAX_CSV_ROWS {
                return Err(AppError::Validation(format!(
                    "csv contains too many rows (max {MAX_CSV_ROWS})"
                )));
            }

            let csv_row = row_number + 2;
            let name = Self::required_field(&record, name_idx, "name", csv_row)?;
            let url = Self::required_field(&record, url_idx, "url", csv_row)?;
            let username = Self::required_field(&record, username_idx, "username", csv_row)?;
            let password = Self::required_field(&record, password_idx, "password", csv_row)?;
            let notes = Self::required_field(&record, notes_idx, "notes", csv_row)?;

            Self::validate_field_length("name", name.as_str())?;
            Self::validate_field_length("url", url.as_str())?;
            Self::validate_field_length("username", username.as_str())?;
            Self::validate_field_length("password", password.as_str())?;
            Self::validate_field_length("notes", notes.as_str())?;
            Self::validate_optional_url(url.as_str())?;

            if password.is_empty() {
                continue;
            }

            rows.push(CsvRow {
                name,
                url,
                username,
                password,
                notes,
            });
        }

        Ok(rows)
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

impl Default for ImportServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportService for ImportServiceImpl {
    async fn import_csv<TSecret, TVault>(
        &self,
        csv_file_path: &Path,
        admin_user_id: Uuid,
        admin_master_key: SecretBox<Vec<u8>>,
        secret_service: Arc<TSecret>,
        vault_service: Arc<TVault>,
    ) -> Result<usize, AppError>
    where
        TSecret: SecretService + Send + Sync + 'static,
        TVault: VaultService + Send + Sync + 'static,
    {
        let rows = Self::parse_csv_rows(csv_file_path)?;
        if rows.is_empty() {
            return Ok(0);
        }

        let vaults = vault_service.list_user_vaults(admin_user_id).await?;
        let first_vault = vaults
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound("no vault found for csv import".to_string()))?;

        let vault_key = vault_service
            .open_vault(
                first_vault.id,
                SecretBox::new(Box::new(admin_master_key.expose_secret().clone())),
            )
            .await?;

        let mut imported_count = 0usize;
        for row in rows {
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

            secret_service
                .create_secret(
                    first_vault.id,
                    SecretType::Password,
                    title,
                    Some(metadata),
                    Some("import,csv".to_string()),
                    None,
                    SecretBox::new(Box::new(row.password.into_bytes())),
                    SecretBox::new(Box::new(vault_key.expose_secret().clone())),
                )
                .await?;
            imported_count += 1;
        }

        info!(
            imported_count,
            vault_id = %first_vault.id,
            "csv import completed successfully"
        );
        Ok(imported_count)
    }
}

struct CsvRow {
    name: String,
    url: String,
    username: String,
    password: String,
    notes: String,
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
    use crate::errors::AppError;

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
        assert!(
            matches!(parsed, Err(AppError::Validation(_))),
            "invalid URL scheme should be rejected"
        );
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
        assert!(
            matches!(parsed, Err(AppError::Validation(_))),
            "truncated records should fail validation"
        );
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
        assert!(parsed.is_ok(), "empty password should be handled as skip, not error");
        let rows = match parsed {
            Ok(value) => value,
            Err(_) => return,
        };
        assert!(rows.is_empty(), "rows with empty password should be skipped");
    }
}
