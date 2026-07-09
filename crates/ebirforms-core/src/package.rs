use crate::crypto::{encrypt_payload, CryptoError};
use crate::form::{render_form, FormDefinition, FormError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error(transparent)]
    Form(#[from] FormError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error("missing package input field: {0}")]
    MissingInput(&'static str),
    #[error("invalid package input field `{field}`: {reason}")]
    InvalidInput { field: &'static str, reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionManifest {
    pub form_code: String,
    pub form_version: String,
    pub remote_directory: String,
    pub remote_path: String,
    pub filename: String,
    pub plaintext_sha256: String,
    pub payload_sha256: String,
    pub payload_size: usize,
    #[serde(rename = "period_mmYYYY")]
    pub period_mm_yyyy: String,
    pub profile_id: String,
    pub generated_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionPackage {
    pub plaintext: Vec<u8>,
    pub payload: Vec<u8>,
    pub manifest: SubmissionManifest,
}

pub fn build_submission_package(
    form_code: &str,
    input: &Value,
) -> Result<SubmissionPackage, PackageError> {
    let definition = FormDefinition::builtin(form_code)?;
    let plaintext_string = render_form(form_code, input)?;
    let plaintext = plaintext_string.into_bytes();
    let payload = encrypt_payload(&plaintext)?;

    let tin = input_string(input, "profile.tin", "profile.tin")?;
    let email = input_string(input, "profile.email", "profile.email")?;
    let profile_id = input_string(input, "profile.profile_id", "profile.profile_id")?;
    let month = input_u64(input, "return.period.month", "return.period.month")?;
    let year = input_u64(input, "return.period.year", "return.period.year")?;
    if !(1..=12).contains(&month) {
        return Err(PackageError::InvalidInput {
            field: "return.period.month",
            reason: "must be between 1 and 12".to_string(),
        });
    }
    let period_mm_yyyy = format!("{month:02}{year:04}");
    let amendment_suffix = input
        .pointer("/return/amendment_number")
        .and_then(|v| v.as_u64())
        .filter(|n| *n > 1)
        .map(|n| format!("V{n}"))
        .unwrap_or_default();

    let normalized_tin: String = tin.chars().filter(|c| c.is_ascii_digit()).collect();
    if normalized_tin.len() < 9 {
        return Err(PackageError::InvalidInput {
            field: "profile.tin",
            reason: "must contain at least 9 digits".to_string(),
        });
    }

    let filename = definition
        .metadata
        .filename_pattern
        .replace("{tin}", &normalized_tin)
        .replace("{form_code}", &definition.metadata.code)
        .replace("{period_mmYYYY}", &period_mm_yyyy)
        .replace("{amendment_suffix}", &amendment_suffix)
        .replace("{email}", &email);
    let remote_path = format!("{}{}", definition.metadata.remote_directory, filename);

    let manifest = SubmissionManifest {
        form_code: definition.metadata.code,
        form_version: definition.metadata.version,
        remote_directory: definition.metadata.remote_directory,
        remote_path,
        filename,
        plaintext_sha256: sha256_hex(&plaintext),
        payload_sha256: sha256_hex(&payload),
        payload_size: payload.len(),
        period_mm_yyyy,
        profile_id,
        generated_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    Ok(SubmissionPackage {
        plaintext,
        payload,
        manifest,
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn input_string(
    input: &Value,
    dotted_path: &'static str,
    field: &'static str,
) -> Result<String, PackageError> {
    let mut current = input;
    for part in dotted_path.split('.') {
        current = current.get(part).ok_or(PackageError::MissingInput(field))?;
    }
    current
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| PackageError::InvalidInput {
            field,
            reason: "must be a string".to_string(),
        })
}

fn input_u64(
    input: &Value,
    dotted_path: &'static str,
    field: &'static str,
) -> Result<u64, PackageError> {
    let mut current = input;
    for part in dotted_path.split('.') {
        current = current.get(part).ok_or(PackageError::MissingInput(field))?;
    }
    current.as_u64().ok_or_else(|| PackageError::InvalidInput {
        field,
        reason: "must be an unsigned integer".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/1601C")
    }

    #[test]
    fn packages_public_1601c_fixture_with_expected_filename() {
        let input: Value =
            serde_json::from_slice(&fs::read(fixture_dir().join("input.json")).unwrap()).unwrap();
        let expected_plaintext = fs::read(fixture_dir().join("synthetic_plaintext.xml")).unwrap();
        let expected_filename =
            fs::read_to_string(fixture_dir().join("synthetic_filename.txt")).unwrap();

        let package = build_submission_package("1601C", &input).expect("package fixture");

        let expected_payload = fs::read(fixture_dir().join("synthetic_encrypted.xml")).unwrap();

        assert_eq!(package.plaintext, expected_plaintext);
        assert_eq!(package.payload, expected_payload);
        assert_eq!(package.manifest.filename, expected_filename.trim());
        assert_eq!(package.manifest.payload_size, package.payload.len());
        assert_eq!(
            package.manifest.payload_sha256,
            sha256_hex(&package.payload)
        );
        assert_eq!(
            package.manifest.remote_path,
            format!("/1601C/{}", expected_filename.trim())
        );
    }
}
