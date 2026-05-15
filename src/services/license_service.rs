use crate::models::{License, LicenseTier};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hex::FromHex;
use serde::Deserialize;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};
use zeroize::Zeroize;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LicensePayload {
    JsonString(String),
    JsonObject(Value),
}

#[derive(Debug, Deserialize)]
struct SignedLicenseEnvelope {
    payload: LicensePayload,
    signature: String,
}

fn sanitize_hex_input(input: &str) -> String {
    let trimmed = input.trim();
    let without_prefix = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    without_prefix
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Public key for verifying license signatures.
/// This is the HEELONYS public key hardcoded at build time.
/// Format: 32 bytes in hex (64 characters).
const LICENSE_SIGNING_PUBLIC_KEY: &str =
    "0c00513e16abc701916dd3e8fbd9ae8cacd7f73f3cc09cfac12c91de2bd3177d";
const AUDIT_SIGNING_KEY_ENV: &str = "HEELONVAULT_AUDIT_SIGNING_KEY";
const AUDIT_SIGNING_KEY_PATH_ENV: &str = "HEELONVAULT_AUDIT_SIGNING_KEY_PATH";
const AUDIT_SIGNING_KEY_FILENAME: &str = "audit.key";
const LEGACY_AUDIT_SIGNING_KEY_FILENAME: &str = "audit-signing.key";
const AUTO_GENERATED_AUDIT_KEY_MARKER: &str = "# origin: auto-generated";

#[derive(Debug)]
pub enum AuditSigningError {
    LicenseRequired,
    MissingKey(String),
    InvalidKey(String),
}

#[derive(Clone, Copy, Debug)]
pub struct AuditCertificationStatus {
    pub is_certified_license: bool,
    pub signing_key_present: bool,
    pub signing_key_auto_generated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditKeyProvisioningOutcome {
    Existing,
    Generated,
}

/// Community (free) edition - always valid, no signature check needed.
fn create_community_license() -> License {
    License {
        id: uuid::Uuid::new_v4().to_string(),
        customer_name: "Community Edition".to_string(),
        slots_count: 1,
        expiration_date: "9999-12-31T23:59:59Z".to_string(),
        features: vec!["audit_log".to_string()],
        tier: LicenseTier::Community,
    }
}

/// LicenseService handles license validation and loading.
pub struct LicenseService {
    /// Cached license after first successful load / verification.
    cached_license: Option<License>,
    audit_key_auto_generated: AtomicBool,
}

impl LicenseService {
    pub fn new() -> Self {
        Self {
            cached_license: None,
            audit_key_auto_generated: AtomicBool::new(false),
        }
    }

    /// Attempt to load and verify a license from the system.
    /// Returns Community tier if no Professional license found.
    pub async fn load_license(&mut self) -> Result<License, Box<dyn std::error::Error>> {
        // If cached, return it
        if let Some(cached) = &self.cached_license {
            return Ok(cached.clone());
        }

        // Attempt to load Professional license from platform-specific path
        let license_path = Self::get_license_path();
        debug!(path = ?license_path, "attempting to load professional license");

        match fs::read_to_string(&license_path) {
            Ok(content) => {
                match self.verify_and_parse_license(&content).await {
                    Ok(license) => {
                        info!(
                            customer = license.customer_name,
                            slots = license.slots_count,
                            "professional license loaded and verified"
                        );
                        self.cached_license = Some(license.clone());
                        if matches!(license.tier, LicenseTier::Professional) {
                            match self.ensure_audit_key_exists() {
                                Ok(AuditKeyProvisioningOutcome::Generated) => {
                                    info!("audit signing key auto-generated for certified audit exports");
                                }
                                Ok(AuditKeyProvisioningOutcome::Existing) => {}
                                Err(error) => {
                                    warn!(error = ?error, "unable to provision audit signing key automatically");
                                }
                            }
                        }
                        Ok(license)
                    }
                    Err(e) => {
                        warn!(path = ?license_path, error = %e, "professional license verification failed, falling back to community");
                        let community = create_community_license();
                        self.cached_license = Some(community.clone());
                        Ok(community)
                    }
                }
            }
            Err(_) => {
                debug!(path = ?license_path, "no professional license file found, using community edition");
                let community = create_community_license();
                self.cached_license = Some(community.clone());
                Ok(community)
            }
        }
    }

    /// Verify ed25519 signature and parse license from signed bundle.
    async fn verify_and_parse_license(
        &self,
        signed_content: &str,
    ) -> Result<License, Box<dyn std::error::Error>> {
        // Parse the signed license JSON
        let signed: SignedLicenseEnvelope = serde_json::from_str(signed_content)
            .map_err(|e| format!("invalid license JSON format: {}", e))?;

        let (payload_to_verify, license_payload) = match signed.payload {
            LicensePayload::JsonString(raw) => {
                let license: License = serde_json::from_str(&raw)
                    .map_err(|e| format!("invalid license payload JSON: {}", e))?;
                (raw, license)
            }
            LicensePayload::JsonObject(value) => {
                // Canonical compact serialization for signature verification when payload is embedded as object.
                let payload_json = serde_json::to_string(&value)
                    .map_err(|e| format!("invalid embedded payload JSON: {}", e))?;
                let license: License = serde_json::from_value(value)
                    .map_err(|e| format!("invalid embedded license payload: {}", e))?;
                (payload_json, license)
            }
        };

        // Decode public key from hex
        let normalized_public_key = sanitize_hex_input(LICENSE_SIGNING_PUBLIC_KEY);
        let pubkey_bytes: [u8; 32] = <[u8; 32]>::from_hex(normalized_public_key.as_str())
            .map_err(|e| format!("invalid public key format: {}", e))?;
        let verifying_key = VerifyingKey::from_bytes(&pubkey_bytes)
            .map_err(|e| format!("failed to construct verifying key: {}", e))?;

        // Decode signature from hex (should be 64 bytes = 128 hex chars)
        let normalized_signature = sanitize_hex_input(&signed.signature);
        let sig_bytes: Vec<u8> = match hex::decode(normalized_signature.as_str()) {
            Ok(bytes) => bytes,
            Err(hex_error) => STANDARD.decode(&signed.signature).map_err(|b64_error| {
                format!(
                    "invalid signature format (hex/base64): hex={}, base64={}",
                    hex_error, b64_error
                )
            })?,
        };

        if sig_bytes.len() != 64 {
            return Err(format!(
                "invalid signature length: expected 64 bytes, got {}",
                sig_bytes.len()
            )
            .into());
        }

        let signature = Signature::from_slice(&sig_bytes)
            .map_err(|e| format!("failed to parse signature: {}", e))?;

        // Verify signature
        verifying_key
            .verify_strict(payload_to_verify.as_bytes(), &signature)
            .map_err(|e| format!("signature verification failed: {}", e))?;

        // Check expiration
        if !license_payload.is_valid() {
            return Err("license has expired".into());
        }

        Ok(license_payload)
    }

    /// Get platform-specific license file path.
    /// - Linux development: ~/.config/heelonvault/license.hvl
    /// - Linux production: /etc/heelonvault/license.hvl (requires permissions)
    /// - Windows: %PROGRAMDATA%\HeelonVault\license.hvl
    /// - macOS: /Library/Application Support/heelonvault/license.hvl
    fn get_license_path() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            // Check if we're in development (current_exe is in target/debug or target/release)
            if let Ok(exe_path) = std::env::current_exe() {
                if exe_path.to_string_lossy().contains("target/debug")
                    || exe_path.to_string_lossy().contains("target/release")
                {
                    // Development mode: use ~/.config/heelonvault/
                    if let Ok(home) = std::env::var("HOME") {
                        return PathBuf::from(format!("{}/.config/heelonvault/license.hvl", home));
                    }
                }
            }
            // Production mode: use /etc/heelonvault/
            PathBuf::from("/etc/heelonvault/license.hvl")
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(program_data) = std::env::var("PROGRAMDATA") {
                PathBuf::from(format!("{}\\HeelonVault\\license.hvl", program_data))
            } else {
                PathBuf::from("C:\\ProgramData\\HeelonVault\\license.hvl")
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(format!(
                    "{}/Library/Application Support/heelonvault/license.hvl",
                    home
                ))
            } else {
                PathBuf::from("/Library/Application Support/heelonvault/license.hvl")
            }
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            PathBuf::from("license.hvl")
        }
    }

    /// Get the current cached license (if loaded).
    pub fn get_cached(&self) -> Option<&License> {
        self.cached_license.as_ref()
    }

    pub fn audit_certification_status(&self) -> AuditCertificationStatus {
        let signing_key_present = self.resolve_audit_signing_key_material().is_ok();
        AuditCertificationStatus {
            is_certified_license: self.has_certified_license(),
            signing_key_present,
            signing_key_auto_generated: signing_key_present && self.is_audit_key_auto_generated(),
        }
    }

    pub fn ensure_audit_key_exists(
        &self,
    ) -> Result<AuditKeyProvisioningOutcome, AuditSigningError> {
        if !self.has_certified_license() {
            return Err(AuditSigningError::LicenseRequired);
        }

        if let Ok(key) = std::env::var(AUDIT_SIGNING_KEY_ENV) {
            if !key.trim().is_empty() {
                self.audit_key_auto_generated
                    .store(false, Ordering::Relaxed);
                return Ok(AuditKeyProvisioningOutcome::Existing);
            }
        }

        if let Some((_, auto_generated)) = self.find_existing_audit_key_file()? {
            self.audit_key_auto_generated
                .store(auto_generated, Ordering::Relaxed);
            return Ok(AuditKeyProvisioningOutcome::Existing);
        }

        let target_path = Self::primary_audit_key_path();
        let parent_dir = target_path.parent().ok_or_else(|| {
            AuditSigningError::InvalidKey(format!(
                "invalid audit signing key path: {}",
                target_path.display()
            ))
        })?;

        fs::create_dir_all(parent_dir).map_err(|error| {
            AuditSigningError::InvalidKey(format!(
                "unable to create audit signing key directory {}: {}",
                parent_dir.display(),
                error
            ))
        })?;

        #[cfg(unix)]
        {
            let _ = fs::set_permissions(parent_dir, fs::Permissions::from_mode(0o700));
        }

        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).map_err(|error| {
            AuditSigningError::InvalidKey(format!(
                "unable to generate audit signing key material: {}",
                error
            ))
        })?;

        let encoded_key = STANDARD.encode(seed);
        let serialized = format!(
            "# HeelonVault audit signing key\n{}\n{}\n",
            AUTO_GENERATED_AUDIT_KEY_MARKER, encoded_key
        );

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target_path)
        {
            Ok(mut file) => {
                #[cfg(unix)]
                {
                    let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
                }

                file.write_all(serialized.as_bytes()).map_err(|error| {
                    AuditSigningError::InvalidKey(format!(
                        "unable to write audit signing key at {}: {}",
                        target_path.display(),
                        error
                    ))
                })?;
                file.flush().map_err(|error| {
                    AuditSigningError::InvalidKey(format!(
                        "unable to flush audit signing key at {}: {}",
                        target_path.display(),
                        error
                    ))
                })?;
                self.audit_key_auto_generated.store(true, Ordering::Relaxed);
                seed.zeroize();
                Ok(AuditKeyProvisioningOutcome::Generated)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                seed.zeroize();
                if let Some((_, auto_generated)) = self.find_existing_audit_key_file()? {
                    self.audit_key_auto_generated
                        .store(auto_generated, Ordering::Relaxed);
                    Ok(AuditKeyProvisioningOutcome::Existing)
                } else {
                    Err(AuditSigningError::MissingKey(format!(
                        "audit signing key creation raced and no readable key was found at {}",
                        target_path.display()
                    )))
                }
            }
            Err(error) => {
                seed.zeroize();
                Err(AuditSigningError::InvalidKey(format!(
                    "unable to create audit signing key at {}: {}",
                    target_path.display(),
                    error
                )))
            }
        }
    }

    pub fn sign_audit_hash(&self, hash: &[u8]) -> Result<String, AuditSigningError> {
        let Some(license) = self.cached_license.as_ref() else {
            return Err(AuditSigningError::LicenseRequired);
        };

        if matches!(license.tier, LicenseTier::Community) {
            return Err(AuditSigningError::LicenseRequired);
        }

        self.ensure_audit_key_exists()?;
        let raw_key_material = self.resolve_audit_signing_key_material()?;
        let key_bytes = decode_key_material(raw_key_material.as_str())
            .map_err(AuditSigningError::InvalidKey)?;
        let secret_key: [u8; 32] = match key_bytes.len() {
            32 => key_bytes.as_slice().try_into().map_err(|_| {
                AuditSigningError::InvalidKey("invalid 32-byte signing key".to_string())
            })?,
            64 => key_bytes[..32].try_into().map_err(|_| {
                AuditSigningError::InvalidKey("invalid 64-byte signing key".to_string())
            })?,
            other => {
                return Err(AuditSigningError::InvalidKey(format!(
                    "invalid signing key length: expected 32 or 64 bytes, got {}",
                    other
                )));
            }
        };

        let signing_key = SigningKey::from_bytes(&secret_key);
        let signature = signing_key.sign(hash);
        Ok(STANDARD.encode(signature.to_bytes()))
    }

    fn has_certified_license(&self) -> bool {
        self.cached_license
            .as_ref()
            .map(|license| matches!(license.tier, LicenseTier::Professional))
            .unwrap_or(false)
    }

    fn is_audit_key_auto_generated(&self) -> bool {
        if self.audit_key_auto_generated.load(Ordering::Relaxed) {
            return true;
        }

        self.find_existing_audit_key_file()
            .ok()
            .flatten()
            .map(|(_, auto_generated)| auto_generated)
            .unwrap_or(false)
    }

    fn resolve_audit_signing_key_material(&self) -> Result<String, AuditSigningError> {
        if let Ok(key) = std::env::var(AUDIT_SIGNING_KEY_ENV) {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }

        if let Some((key_material, auto_generated)) = self.find_existing_audit_key_file()? {
            self.audit_key_auto_generated
                .store(auto_generated, Ordering::Relaxed);
            return Ok(key_material);
        }

        Err(AuditSigningError::MissingKey(format!(
            "audit signing key not found at {}",
            Self::primary_audit_key_path().display()
        )))
    }

    /// Check if a given number of slots is available under current license.
    pub fn has_capacity(&self, current_slots: u32) -> bool {
        self.cached_license
            .as_ref()
            .map(|lic| lic.has_capacity(current_slots))
            .unwrap_or(true)
    }

    #[cfg(test)]
    pub(crate) fn set_cached_license_for_tests(&mut self, license: License) {
        self.cached_license = Some(license);
    }
}

impl Default for LicenseService {
    fn default() -> Self {
        Self::new()
    }
}

fn decode_key_material(input: &str) -> Result<Vec<u8>, String> {
    let normalized_hex = sanitize_hex_input(input);
    if normalized_hex.len() % 2 == 0 {
        if let Ok(bytes) = hex::decode(normalized_hex.as_str()) {
            return Ok(bytes);
        }
    }

    STANDARD
        .decode(input.trim())
        .map_err(|error| format!("invalid signing key format: {}", error))
}

fn parse_audit_key_file(content: &str) -> Result<(String, bool), AuditSigningError> {
    let mut auto_generated = false;
    let mut key_material = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            if trimmed.eq_ignore_ascii_case(AUTO_GENERATED_AUDIT_KEY_MARKER) {
                auto_generated = true;
            }
            continue;
        }

        key_material = Some(trimmed.to_string());
        break;
    }

    key_material
        .map(|key| (key, auto_generated))
        .ok_or_else(|| AuditSigningError::MissingKey("audit signing key file is empty".to_string()))
}

impl LicenseService {
    fn primary_audit_key_path() -> PathBuf {
        if let Ok(path) = std::env::var(AUDIT_SIGNING_KEY_PATH_ENV) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }

        if let Some(config_dir) = dirs::config_dir() {
            return config_dir
                .join("heelonvault")
                .join(AUDIT_SIGNING_KEY_FILENAME);
        }

        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".config")
                .join("heelonvault")
                .join(AUDIT_SIGNING_KEY_FILENAME);
        }

        PathBuf::from(AUDIT_SIGNING_KEY_FILENAME)
    }

    fn legacy_audit_key_path() -> PathBuf {
        Self::get_license_path().with_file_name(LEGACY_AUDIT_SIGNING_KEY_FILENAME)
    }

    fn find_existing_audit_key_file(&self) -> Result<Option<(String, bool)>, AuditSigningError> {
        let primary_path = Self::primary_audit_key_path();
        if primary_path.exists() {
            let content = fs::read_to_string(&primary_path).map_err(|error| {
                AuditSigningError::InvalidKey(format!(
                    "unable to read audit signing key at {}: {}",
                    primary_path.display(),
                    error
                ))
            })?;
            return parse_audit_key_file(&content).map(Some);
        }

        if std::env::var(AUDIT_SIGNING_KEY_PATH_ENV)
            .ok()
            .map(|path| !path.trim().is_empty())
            .unwrap_or(false)
        {
            return Ok(None);
        }

        let legacy_path = Self::legacy_audit_key_path();
        if legacy_path.exists() {
            let content = fs::read_to_string(&legacy_path).map_err(|error| {
                AuditSigningError::InvalidKey(format!(
                    "unable to read audit signing key at {}: {}",
                    legacy_path.display(),
                    error
                ))
            })?;
            return parse_audit_key_file(&content).map(Some);
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_community_license_creation() {
        let lic = create_community_license();
        assert_eq!(lic.tier, LicenseTier::Community);
        assert!(lic.is_valid());
        assert!(lic.has_capacity(0));
    }
}
