use crate::submission::{SubmissionError, SubmissionRecord, SubmissionStore};
use crate::transport::SubmissionStatus;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptMetadata {
    pub receipt_id: String,
    pub status_text: String,
    pub filename: String,
    pub form_code: String,
    #[serde(rename = "period_mmYYYY")]
    pub period_mm_yyyy: String,
    pub received_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ReceiptError {
    #[error("receipt is missing required field `{0}`")]
    MissingField(&'static str),
    #[error("receipt `{receipt_id}` did not match any submission record by filename/form/period")]
    NoMatchingSubmission { receipt_id: String },
    #[error("receipt status `{0}` is not an accepted/confirmed status")]
    NotAccepted(String),
    #[error("receipt poll failed: {0}")]
    Poll(String),
    #[error(transparent)]
    Submission(#[from] SubmissionError),
}

pub fn parse_receipt(text: &str) -> Result<ReceiptMetadata, ReceiptError> {
    let fields = parse_key_value_lines(text);
    let receipt_id = required(&fields, "receipt_id", "Receipt-ID")?;
    let status_text = required(&fields, "status", "Status")?;
    let filename = required(&fields, "filename", "Filename")?;
    let form_code = required(&fields, "form", "Form")?;
    let period_mm_yyyy = required(&fields, "period", "Period")?;
    let received_at = required(&fields, "received_at", "Received-At")?;

    Ok(ReceiptMetadata {
        receipt_id,
        status_text,
        filename,
        form_code,
        period_mm_yyyy,
        received_at,
    })
}

pub fn apply_receipt_to_store(
    store: &SubmissionStore,
    receipt: ReceiptMetadata,
) -> Result<SubmissionRecord, ReceiptError> {
    if !is_accepted_status(&receipt.status_text) {
        return Err(ReceiptError::NotAccepted(receipt.status_text));
    }

    let mut records = store.load()?;
    let record = records
        .iter_mut()
        .find(|record| {
            record.filename == receipt.filename
                && record.form_code == receipt.form_code
                && record.period_mm_yyyy == receipt.period_mm_yyyy
        })
        .ok_or_else(|| ReceiptError::NoMatchingSubmission {
            receipt_id: receipt.receipt_id.clone(),
        })?;

    record.status = SubmissionStatus::Confirmed;
    record.last_error = None;
    record.receipt = Some(receipt);
    record.touch_for_receipt();
    let out = record.clone();
    store.save(&records)?;
    Ok(out)
}

pub fn parse_and_apply_receipt(
    store: &SubmissionStore,
    text: &str,
) -> Result<SubmissionRecord, ReceiptError> {
    let receipt = parse_receipt(text)?;
    apply_receipt_to_store(store, receipt)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptPollReport {
    pub scanned: usize,
    pub confirmed: Vec<SubmissionRecord>,
    pub errors: Vec<String>,
}

pub fn poll_receipt_directory(
    store: &SubmissionStore,
    dir: &Path,
) -> Result<ReceiptPollReport, ReceiptError> {
    let mut report = ReceiptPollReport {
        scanned: 0,
        confirmed: Vec::new(),
        errors: Vec::new(),
    };
    let entries = fs::read_dir(dir).map_err(|err| ReceiptError::Poll(err.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|err| ReceiptError::Poll(err.to_string()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if !matches!(extension.to_ascii_lowercase().as_str(), "txt" | "eml") {
            continue;
        }
        report.scanned += 1;
        match fs::read_to_string(&path)
            .map_err(|err| ReceiptError::Poll(err.to_string()))
            .and_then(|text| parse_and_apply_receipt(store, &text))
        {
            Ok(record) => report.confirmed.push(record),
            Err(err) => report.errors.push(format!("{}: {err}", path.display())),
        }
    }
    Ok(report)
}

fn parse_key_value_lines(text: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let normalized = key
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .replace(' ', "_");
        fields.insert(normalized, value.trim().to_string());
    }
    fields
}

fn required(
    fields: &BTreeMap<String, String>,
    key: &'static str,
    display: &'static str,
) -> Result<String, ReceiptError> {
    fields
        .get(key)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or(ReceiptError::MissingField(display))
}

fn is_accepted_status(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "accepted" | "confirmed" | "received"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::build_submission_package;
    use crate::submission::{submit_with_store, SubmitMode};
    use crate::transport::DryRunTransport;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_input() -> Value {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/1601C/input.json");
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("{name}-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn parses_receipt_fixture() {
        let receipt = parse_receipt(
            "Receipt-ID: TEST-1601C-001\nStatus: ACCEPTED\nFilename: 12345678900000-1601C-062026V2#authorized@example.test#.xml\nForm: 1601C\nPeriod: 062026\nReceived-At: 2026-07-09T10:00:00Z\n",
        )
        .unwrap();
        assert_eq!(receipt.receipt_id, "TEST-1601C-001");
        assert_eq!(receipt.status_text, "ACCEPTED");
    }

    #[test]
    fn receipt_directory_poll_confirms_matching_submission_record() {
        let records_path = temp_path("ebirforms-receipt-poll-records");
        let dir_path =
            std::env::temp_dir().join(format!("ebirforms-receipt-poll-dir-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir_path);
        fs::create_dir_all(&dir_path).unwrap();
        let store = SubmissionStore::new(&records_path);
        let package = build_submission_package("1601C", &fixture_input()).unwrap();
        let mut transport = DryRunTransport::new();
        submit_with_store(&package, &store, &mut transport, SubmitMode::DryRun).unwrap();
        fs::write(
            dir_path.join("receipt.txt"),
            format!(
                "Receipt-ID: TEST-1601C-001\nStatus: ACCEPTED\nFilename: {}\nForm: 1601C\nPeriod: 062026\nReceived-At: 2026-07-09T10:00:00Z\n",
                package.manifest.filename
            ),
        )
        .unwrap();

        let report = poll_receipt_directory(&store, &dir_path).unwrap();

        assert_eq!(report.scanned, 1);
        assert_eq!(report.confirmed.len(), 1);
        assert!(report.errors.is_empty());
        assert_eq!(report.confirmed[0].status, SubmissionStatus::Confirmed);
        let _ = fs::remove_file(&records_path);
        let _ = fs::remove_dir_all(&dir_path);
    }
}
