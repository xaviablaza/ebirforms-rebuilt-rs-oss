use crate::submission::{SubmissionError, SubmissionRecord, SubmissionStore};
use crate::transport::SubmissionStatus;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

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
    let bir_email = parse_bir_confirmation_email(text);
    let filename = optional(&fields, "filename")
        .or_else(|| optional(&fields, "file_name"))
        .or_else(|| bir_email.as_ref().map(|email| email.filename.clone()))
        .ok_or(ReceiptError::MissingField("Filename"))?;
    let receipt_id = optional(&fields, "receipt_id")
        .or_else(|| {
            bir_email
                .as_ref()
                .map(|email| format!("BIR-{}", email.filename))
        })
        .unwrap_or_else(|| format!("BIR-{filename}"));
    let status_text = optional(&fields, "status")
        .or_else(|| bir_email.as_ref().map(|email| email.status_text.clone()))
        .or_else(|| bir_received_status(&fields))
        .ok_or(ReceiptError::MissingField("Status"))?;
    let form_code = optional(&fields, "form")
        .or_else(|| bir_email.as_ref().map(|email| email.form_code.clone()))
        .or_else(|| infer_form_code(&filename))
        .ok_or(ReceiptError::MissingField("Form"))?;
    let period_mm_yyyy = optional(&fields, "period")
        .or_else(|| bir_email.as_ref().map(|email| email.period_mm_yyyy.clone()))
        .or_else(|| infer_period_mm_yyyy(&filename))
        .ok_or(ReceiptError::MissingField("Period"))?;
    let received_at = optional(&fields, "received_at")
        .or_else(|| bir_email.as_ref().map(|email| email.received_at.clone()))
        .or_else(|| bir_received_at(&fields))
        .ok_or(ReceiptError::MissingField("Received-At"))?;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HimalayaReceiptPollOptions {
    pub account: Option<String>,
    pub folder: Option<String>,
    pub query: Vec<String>,
    pub limit: usize,
    pub binary: Option<String>,
}

impl Default for HimalayaReceiptPollOptions {
    fn default() -> Self {
        Self {
            account: None,
            folder: Some("INBOX".to_string()),
            query: vec![
                "subject".to_string(),
                "Tax Return Receipt Confirmation".to_string(),
            ],
            limit: 25,
            binary: None,
        }
    }
}

pub fn poll_receipts_himalaya(
    store: &SubmissionStore,
    options: HimalayaReceiptPollOptions,
) -> Result<ReceiptPollReport, ReceiptError> {
    let himalaya_bin = resolve_himalaya_binary(options.binary.as_deref());
    let mut list_cmd = Command::new(&himalaya_bin);
    if let Some(account) = options.account.as_deref().filter(|value| !value.is_empty()) {
        list_cmd.arg("--account").arg(account);
    }
    list_cmd.arg("envelope").arg("list");
    if let Some(folder) = options.folder.as_deref().filter(|value| !value.is_empty()) {
        list_cmd.arg("--folder").arg(folder);
    }
    for part in &options.query {
        if !part.trim().is_empty() {
            list_cmd.arg(part);
        }
    }
    list_cmd
        .arg("--page-size")
        .arg(options.limit.max(1).to_string());
    list_cmd.arg("--output").arg("json");

    let output = list_cmd
        .output()
        .map_err(|err| ReceiptError::Poll(format!("himalaya envelope list failed: {err}")))?;
    if !output.status.success() {
        return Err(ReceiptError::Poll(redact_command_stderr(
            "himalaya envelope list",
            &output.stderr,
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let ids = himalaya_message_ids_from_json(&stdout);
    let mut report = ReceiptPollReport {
        scanned: 0,
        confirmed: Vec::new(),
        errors: Vec::new(),
    };

    for id in ids.into_iter().take(options.limit.max(1)) {
        let mut read_cmd = Command::new(&himalaya_bin);
        if let Some(account) = options.account.as_deref().filter(|value| !value.is_empty()) {
            read_cmd.arg("--account").arg(account);
        }
        read_cmd.arg("message").arg("read").arg(&id);
        if let Some(folder) = options.folder.as_deref().filter(|value| !value.is_empty()) {
            read_cmd.arg("--folder").arg(folder);
        }
        let read_output = read_cmd.output().map_err(|err| {
            ReceiptError::Poll(format!("himalaya message read {id} failed: {err}"))
        })?;
        if !read_output.status.success() {
            report.errors.push(redact_command_stderr(
                &format!("himalaya message read {id}"),
                &read_output.stderr,
            ));
            continue;
        }
        report.scanned += 1;
        let text = String::from_utf8_lossy(&read_output.stdout);
        match parse_and_apply_receipt(store, &text) {
            Ok(record) => report.confirmed.push(record),
            Err(err) => report.errors.push(format!("message {id}: {err}")),
        }
    }

    Ok(report)
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

fn resolve_himalaya_binary(configured: Option<&str>) -> String {
    if let Some(value) = configured.filter(|value| !value.trim().is_empty()) {
        return value.to_string();
    }
    if let Ok(value) = std::env::var("EBIRFORMS_HIMALAYA_BIN") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if let Some(path) = adjacent_executable("himalaya") {
        return path;
    }
    "himalaya".to_string()
}

fn adjacent_executable(name: &str) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let binary_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let candidate = dir.join(binary_name);
    if candidate.is_file() {
        Some(candidate.display().to_string())
    } else {
        None
    }
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

fn optional(fields: &BTreeMap<String, String>, key: &str) -> Option<String> {
    fields
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BirConfirmationEmail {
    filename: String,
    form_code: String,
    period_mm_yyyy: String,
    status_text: String,
    received_at: String,
}

fn parse_bir_confirmation_email(text: &str) -> Option<BirConfirmationEmail> {
    let pattern = r#"(?is)this\s+confirms\s+receipt\s+of\s+your\s+submission\s+with\s+the\s+following\s+details\s+subject\s+to\s+validation\s+by\s+BIR:\s*File\s+name:\s*(?P<filename>\d{9,14}-(?P<form>1601C|1601EQ|1701Q|2550Q|0619E|1702Q|2000)(?:v\d{4}[A-Z]?)?-(?P<period>\d{6}(?:Q[1-4])?|\d{4}Q[1-4])(?:V\d+)?(?:#[^#\r\n]+#)?\.xml)\s*\r?\n\s*Date\s+received\s+by\s+BIR:\s*(?P<date>[^\r\n]+?)\s*\r?\n\s*Time\s+received\s+by\s+BIR:\s*(?P<time>[^\r\n]+)"#;
    let regex = Regex::new(pattern).expect("BIR receipt confirmation regex is valid");
    let captures = regex.captures(text)?;
    let filename = captures.name("filename")?.as_str().trim().to_string();
    let form_code = captures.name("form")?.as_str().trim().to_ascii_uppercase();
    let period_mm_yyyy = captures.name("period")?.as_str().trim().to_string();
    let date = captures.name("date")?.as_str().trim();
    let time = captures.name("time")?.as_str().trim();

    Some(BirConfirmationEmail {
        filename,
        form_code,
        period_mm_yyyy,
        status_text: "RECEIVED".to_string(),
        received_at: format!("{date} {time}"),
    })
}

fn bir_received_status(fields: &BTreeMap<String, String>) -> Option<String> {
    if optional(fields, "file_name").is_some() && optional(fields, "date_received_by_bir").is_some()
    {
        Some("RECEIVED".to_string())
    } else {
        None
    }
}

fn bir_received_at(fields: &BTreeMap<String, String>) -> Option<String> {
    let date = optional(fields, "date_received_by_bir")?;
    match optional(fields, "time_received_by_bir") {
        Some(time) => Some(format!("{date} {time}")),
        None => Some(date),
    }
}

fn infer_form_code(filename: &str) -> Option<String> {
    for form_code in [
        "1601EQ", "1601C", "1701Q", "2550Q", "0619E", "1702Q", "2000",
    ] {
        if filename
            .to_ascii_uppercase()
            .contains(&form_code.to_ascii_uppercase())
        {
            return Some(form_code.to_string());
        }
    }
    None
}

fn infer_period_mm_yyyy(filename: &str) -> Option<String> {
    let quarterly_month_period = Regex::new(r"-(\d{6}Q[1-4])(?:V\d+|#|\.|$)").ok()?;
    if let Some(caps) = quarterly_month_period.captures(filename) {
        return Some(caps[1].to_string());
    }

    let quarterly_year_period = Regex::new(r"-(\d{4}Q[1-4])(?:V\d+|#|\.|$)").ok()?;
    if let Some(caps) = quarterly_year_period.captures(filename) {
        return Some(caps[1].to_string());
    }

    let monthly_period = Regex::new(r"-(\d{6})(?:V\d+|#|\.|$)").ok()?;
    monthly_period
        .captures(filename)
        .map(|caps| caps[1].to_string())
}

fn is_accepted_status(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "accepted" | "confirmed" | "received"
    )
}

fn himalaya_message_ids_from_json(stdout: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(stdout) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    collect_himalaya_ids(&value, &mut ids);
    ids.sort();
    ids.dedup();
    ids
}

fn collect_himalaya_ids(value: &serde_json::Value, ids: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_himalaya_ids(item, ids);
            }
        }
        serde_json::Value::Object(map) => {
            for key in ["id", "uid", "message_id", "message-id"] {
                if let Some(id) = map.get(key).and_then(|value| match value {
                    serde_json::Value::String(text) => Some(text.clone()),
                    serde_json::Value::Number(number) => Some(number.to_string()),
                    _ => None,
                }) {
                    if !id.trim().is_empty() {
                        ids.push(id);
                        return;
                    }
                }
            }
            for value in map.values() {
                collect_himalaya_ids(value, ids);
            }
        }
        _ => {}
    }
}

fn redact_command_stderr(command: &str, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr)
        .lines()
        .take(3)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    if detail.is_empty() {
        format!("{command} exited unsuccessfully")
    } else {
        format!("{command}: {detail}")
    }
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
    fn parses_bir_receipt_confirmation_email() {
        let receipt = parse_receipt(
            r#"SUBJECT: "Tax Return Receipt Confirmation"
FROM: ebirforms-noreply@bir.gov.ph
This confirms receipt of your submission with the following details subject to validation by BIR:
File name: 010961925000-1601Cv2018-012026V1.xml
Date received by BIR: 15 April 2026
Time received by BIR: 03:10 PM
Penalties may be imposed for any violation of the provisions of the NIRC and issuances thereof.
FOR RETURNS WITH TAX PAYABLE:
Please pay through any of the following ePayment Channels:
Land Bank of the Philippines Link.BizPortal
LBP ATM Cards
Bancnet ATM/Debit Cards
PCHC PayGate or PESONeT (RCBC, Robinsons Bank, UnionBank, PSBank, BPI, Asia United Bank)
DBP PayTax Online
Credit Cards (MasterCard/Visa)
Bancnet ATM/Debit Cards
Unionbank of the Philippines
Unionbank Online (for Unionbank Individual and Corporate Account Holders)
UPAY via InstaPay (For Individual Non-Unionbank Account Holders)
Taxpayer Agent/ Tax Software Provider-TSP
(Gcash/PayMaya/MyEG)
This is a system-generated email. Please do not reply.
Bureau of Internal Revenue"#,
        )
        .unwrap();

        assert_eq!(receipt.filename, "010961925000-1601Cv2018-012026V1.xml");
        assert_eq!(receipt.form_code, "1601C");
        assert_eq!(receipt.period_mm_yyyy, "012026");
        assert_eq!(receipt.status_text, "RECEIVED");
        assert_eq!(receipt.received_at, "15 April 2026 03:10 PM");
        assert_eq!(
            receipt.receipt_id,
            "BIR-010961925000-1601Cv2018-012026V1.xml"
        );
    }

    #[test]
    fn parses_bir_receipt_confirmation_email_for_quarterly_and_versioned_forms() {
        let cases = [
            (
                "12345678900000-2550Qv2024-122026Q1#authorized@example.test#.xml",
                "2550Q",
                "122026Q1",
            ),
            (
                "12345678900000-1702Qv2018C-2026Q1#authorized@example.test#.xml",
                "1702Q",
                "2026Q1",
            ),
            ("12345678900000-1701Q-2026Q2.xml", "1701Q", "2026Q2"),
        ];

        for (filename, form_code, period) in cases {
            let receipt = parse_receipt(&format!(
                "SUBJECT: \"Tax Return Receipt Confirmation\"
FROM: ebirforms-noreply@bir.gov.ph
This confirms receipt of your submission with the following details subject to validation by BIR:
File name: {filename}
Date received by BIR: 15 April 2026
Time received by BIR: 03:10 PM
This is a system-generated email. Please do not reply.
"
            ))
            .unwrap();

            assert_eq!(receipt.filename, filename);
            assert_eq!(receipt.form_code, form_code);
            assert_eq!(receipt.period_mm_yyyy, period);
            assert_eq!(receipt.status_text, "RECEIVED");
        }
    }

    #[test]
    fn extracts_himalaya_message_ids_from_json_shapes() {
        let ids = himalaya_message_ids_from_json(
            r#"[{"id":42,"subject":"Tax Return Receipt Confirmation"},{"envelopes":[{"uid":"abc"},{"message_id":"def"}]}]"#,
        );

        assert_eq!(
            ids,
            vec!["42".to_string(), "abc".to_string(), "def".to_string()]
        );
    }

    #[test]
    fn infers_form_and_period_from_supported_bir_filenames() {
        let cases = [
            (
                "12345678900000-1601C-062026#authorized@example.test#.xml",
                "1601C",
                "062026",
            ),
            (
                "12345678900000-2000v2018-022026#authorized@example.test#.xml",
                "2000",
                "022026",
            ),
            (
                "12345678900000-2550Qv2024-122026Q1#authorized@example.test#.xml",
                "2550Q",
                "122026Q1",
            ),
            (
                "12345678900000-0619E-022026#authorized@example.test#.xml",
                "0619E",
                "022026",
            ),
            (
                "12345678900000-1601EQ-2026Q1#authorized@example.test#.xml",
                "1601EQ",
                "2026Q1",
            ),
            (
                "12345678900000-1702Qv2018C-2026Q1#authorized@example.test#.xml",
                "1702Q",
                "2026Q1",
            ),
            ("12345678900000-1701Q-2026Q2.xml", "1701Q", "2026Q2"),
        ];

        for (filename, form_code, period) in cases {
            let receipt = parse_receipt(&format!(
                "File name: {filename}\nDate received by BIR: 15 April 2026\nTime received by BIR: 03:10 PM\n"
            ))
            .unwrap();
            assert_eq!(receipt.form_code, form_code);
            assert_eq!(receipt.period_mm_yyyy, period);
        }
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
