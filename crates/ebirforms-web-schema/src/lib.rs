use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const TEMPLATE_1701Q: &str = include_str!("../../../tests/fixtures/1701Q/input.json");
const TEMPLATE_1702Q: &str = include_str!("../../../tests/fixtures/1702Q/input.json");

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct GuidedField {
    pub label: &'static str,
    pub path: &'static str,
    pub input_type: &'static str,
    pub hint: &'static str,
    pub required: bool,
    pub sample: &'static str,
}

macro_rules! field {
    ($label:expr,$path:expr,$kind:expr,$hint:expr,$required:expr,$sample:expr) => {
        GuidedField {
            label: $label,
            path: $path,
            input_type: $kind,
            hint: $hint,
            required: $required,
            sample: $sample,
        }
    };
}

pub const COMMON_FIELDS: &[GuidedField] = &[
    field!(
        "Taxpayer identification number (TIN)",
        "profile.tin",
        "text",
        "Nine-digit TIN followed by the five-digit branch code",
        true,
        "12345678900000"
    ),
    field!(
        "Tax year",
        "period.year",
        "number",
        "Four-digit filing year",
        true,
        "2026"
    ),
    field!(
        "Quarter",
        "period.quarter",
        "quarter",
        "First, second, or third quarter",
        true,
        "2"
    ),
    field!(
        "Amended return?",
        "guided.amended",
        "yes_no",
        "Choose Yes only when correcting a filed return",
        true,
        "no"
    ),
];

pub const FIELDS_1701Q: &[GuidedField] = &[
    field!(
        "Registered taxpayer name",
        "guided.taxpayer_name",
        "text",
        "As shown on the BIR registration",
        true,
        "JUAN DELA CRUZ"
    ),
    field!(
        "RDO code",
        "field:frm1701q:txt5RDOCode",
        "text",
        "Three-digit Revenue District Office code",
        true,
        "044"
    ),
    field!(
        "Registered address",
        "guided.registered_address",
        "text",
        "Complete registered address",
        true,
        "1 SAMPLE STREET QUEZON CITY"
    ),
    field!(
        "ZIP code",
        "field:frm1701q:txt14zip",
        "text",
        "Four digits",
        true,
        "1100"
    ),
    field!(
        "Telephone number",
        "field:frm1701q:txt15Telno",
        "tel",
        "Optional",
        false,
        ""
    ),
    field!(
        "Birth month",
        "field:frm1701q:txt13BirthMonth",
        "number",
        "1 to 12",
        true,
        "1"
    ),
    field!(
        "Birth day",
        "field:frm1701q:txt13BirthDay",
        "number",
        "1 to 31",
        true,
        "15"
    ),
    field!(
        "Birth year",
        "field:frm1701q:txt13BirthYear",
        "number",
        "Four digits",
        true,
        "1990"
    ),
    field!(
        "Citizenship",
        "guided.citizenship",
        "text",
        "Example: Filipino",
        true,
        "FILIPINO"
    ),
    field!(
        "Line of business or profession",
        "guided.business",
        "text",
        "Primary trade or profession",
        true,
        "CONSULTING"
    ),
    field!(
        "Taxpayer type",
        "guided.taxpayer_type",
        "taxpayer_type",
        "Select the registered filer type",
        true,
        "single"
    ),
    field!(
        "Alphanumeric tax code (ATC)",
        "guided.atc",
        "atc_1701",
        "Confirm with the filing adviser",
        true,
        "II012"
    ),
    field!(
        "Tax regime",
        "guided.tax_regime",
        "tax_regime",
        "Graduated rates or 8% income tax rate",
        true,
        "graduated"
    ),
    field!(
        "Deduction method",
        "guided.deduction_method",
        "deduction_method",
        "Itemized or optional standard deduction",
        true,
        "itemized"
    ),
    field!(
        "Claiming foreign tax credits?",
        "guided.foreign_credit",
        "yes_no",
        "Confirm treaty or foreign credit treatment",
        true,
        "no"
    ),
    field!(
        "Sales, revenues, receipts or fees",
        "field:frm1701q:txt36A",
        "number",
        "Amount for this quarter",
        true,
        "100000"
    ),
    field!(
        "Cost of sales or services",
        "field:frm1701q:txt37A",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
    field!(
        "Itemized deductions",
        "field:frm1701q:txt38C",
        "number",
        "Used only for itemized deductions",
        true,
        "20000"
    ),
    field!(
        "Taxable income from previous quarters",
        "field:frm1701q:txt38I",
        "number",
        "Year-to-date prior quarter amount",
        true,
        "0"
    ),
    field!(
        "Other non-operating income",
        "field:frm1701q:txt38K",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
    field!(
        "Prior-year excess credits",
        "field:ui1701q:txt55A",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
    field!(
        "Previous-quarter tax payments",
        "field:ui1701q:txt56A",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
    field!(
        "Creditable tax withheld this quarter",
        "field:ui1701q:txt58A",
        "number",
        "From BIR Form 2307",
        true,
        "0"
    ),
];

pub const FIELDS_1702Q: &[GuidedField] = &[
    field!(
        "Registered corporate name",
        "field:txtTaxpayerName",
        "text",
        "As shown on the BIR registration",
        true,
        "SAMPLE DOMESTIC CORPORATION"
    ),
    field!(
        "RDO code",
        "field:txtRDOCode",
        "text",
        "Three-digit Revenue District Office code",
        true,
        "044"
    ),
    field!(
        "Registered address",
        "field:txtAddress",
        "text",
        "Complete registered address",
        true,
        "1 SAMPLE STREET MAKATI CITY"
    ),
    field!(
        "ZIP code",
        "field:txtZipCode",
        "text",
        "Four digits",
        true,
        "1200"
    ),
    field!(
        "Telephone number",
        "field:txtTelNum",
        "tel",
        "Optional",
        false,
        ""
    ),
    field!(
        "Entity type",
        "guided.entity_type",
        "entity_type",
        "Domestic, resident foreign, or non-resident foreign corporation",
        true,
        "domestic"
    ),
    field!(
        "Alphanumeric tax code (ATC)",
        "field:txtATC",
        "atc_1702",
        "Confirm with the filing adviser",
        true,
        "WC160"
    ),
    field!(
        "Deduction method",
        "guided.deduction_method",
        "deduction_method",
        "Itemized or optional standard deduction",
        true,
        "itemized"
    ),
    field!(
        "Subject to a special tax rate?",
        "guided.special_tax",
        "yes_no",
        "Choose Yes only with adviser-confirmed authority",
        true,
        "no"
    ),
    field!(
        "Gross sales or receipts",
        "field:sched1_txtSales1",
        "number",
        "Amount for this quarter",
        true,
        "100000"
    ),
    field!(
        "Cost of sales or services",
        "field:sched1_txtCost2",
        "number",
        "Enter 0 if none",
        true,
        "20000"
    ),
    field!(
        "Other income",
        "field:sched1_txtOtherIncome4",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
    field!(
        "Allowable deductions",
        "field:sched1_txtDeductions6",
        "number",
        "Enter 0 if none",
        true,
        "10000"
    ),
    field!(
        "Taxable income from previous quarters",
        "field:sched1_txtPrevious8",
        "number",
        "Year-to-date prior quarter amount",
        true,
        "0"
    ),
    field!(
        "Prior-year excess credits",
        "field:sched4_txtPriorYearCredits1",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
    field!(
        "Previous-quarter income tax payments",
        "field:sched4_txtPreviousPayments2",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
    field!(
        "Creditable tax withheld this quarter",
        "field:sched4_txtCwtCurrent5",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
];

pub fn fields_for(form_code: &str) -> &'static [GuidedField] {
    if form_code == "1701Q" {
        FIELDS_1701Q
    } else {
        FIELDS_1702Q
    }
}

pub fn blank_payload(form_code: &str, email: &str, user_id: i64) -> Result<Value, String> {
    let source = if form_code == "1701Q" {
        TEMPLATE_1701Q
    } else if form_code == "1702Q" {
        TEMPLATE_1702Q
    } else {
        return Err("unsupported web form".into());
    };
    let mut payload: Value = serde_json::from_str(source).map_err(|e| e.to_string())?;
    if let Some(fields) = payload.get_mut("fields").and_then(Value::as_object_mut) {
        for value in fields.values_mut() {
            *value = Value::String(String::new())
        }
    }
    payload["profile"] =
        json!({"tin":"","email":email,"profile_id":format!("web-customer-{user_id}")});
    payload["return"] =
        json!({"period":{"year":0,"quarter":0,"month":0},"is_amended":false,"amendment_number":0});
    payload["guided"] = json!({});
    Ok(payload)
}

pub fn value_at(payload: &Value, path: &str) -> String {
    if let Some(key) = path.strip_prefix("field:") {
        payload
            .get("fields")
            .and_then(|v| v.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into()
    } else if let Some(key) = path.strip_prefix("guided.") {
        payload
            .get("guided")
            .and_then(|v| v.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into()
    } else if path == "profile.tin" {
        payload
            .pointer("/profile/tin")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into()
    } else {
        let key = path.strip_prefix("period.").unwrap_or_default();
        payload
            .pointer(&format!("/return/period/{key}"))
            .and_then(Value::as_i64)
            .filter(|v| *v > 0)
            .map(|v| v.to_string())
            .unwrap_or_default()
    }
}
pub fn set_value(payload: &mut Value, path: &str, value: &str) {
    if let Some(key) = path.strip_prefix("field:") {
        payload["fields"][key] = Value::String(value.into())
    } else if let Some(key) = path.strip_prefix("guided.") {
        payload["guided"][key] = Value::String(value.into())
    } else if path == "profile.tin" {
        payload["profile"]["tin"] = Value::String(value.into())
    } else {
        let key = path.strip_prefix("period.").unwrap_or_default();
        payload["return"]["period"][key] =
            Value::Number(value.parse::<i64>().unwrap_or_default().into())
    }
}

fn money(payload: &Value, key: &str) -> f64 {
    value_at(payload, &format!("field:{key}"))
        .replace(',', "")
        .parse()
        .unwrap_or(0.0)
}
fn set_field(payload: &mut Value, key: &str, value: impl Into<String>) {
    payload["fields"][key] = Value::String(value.into())
}
fn encode_bir_text(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(byte) {
            encoded.push(*byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

pub fn normalize(form_code: &str, payload: &mut Value) {
    let tin = value_at(payload, "profile.tin")
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    let y = value_at(payload, "period.year")
        .parse::<i64>()
        .unwrap_or_default();
    let q = value_at(payload, "period.quarter")
        .parse::<i64>()
        .unwrap_or_default();
    let seg = (
        tin.get(0..3).unwrap_or_default().to_string(),
        tin.get(3..6).unwrap_or_default().to_string(),
        tin.get(6..9).unwrap_or_default().to_string(),
        tin.get(9..14).unwrap_or_default().to_string(),
    );
    let amended = value_at(payload, "guided.amended") == "yes";
    payload["return"]["is_amended"] = Value::Bool(amended);
    payload["return"]["amendment_number"] = Value::Number((amended as i64).into());
    if form_code == "1701Q" {
        set_field(payload, "frm1701q:txtYear", y.to_string());
        for n in 1..=3 {
            set_field(
                payload,
                &format!("frm1701q:DateQuarter_{n}"),
                (q == n).to_string(),
            )
        }
        set_field(payload, "frm1701q:AmendedRtn_1", amended.to_string());
        set_field(payload, "frm1701q:AmendedRtn_2", (!amended).to_string());
        set_field(payload, "frm1701q:txt5TIN1", seg.0);
        set_field(payload, "frm1701q:txt5TIN2", seg.1);
        set_field(payload, "frm1701q:txt5TIN3", seg.2);
        set_field(payload, "frm1701q:txt5BranchCode", seg.3);
        set_field(
            payload,
            "frm1701q:txtTaxPayername",
            encode_bir_text(&value_at(payload, "guided.taxpayer_name")),
        );
        set_field(
            payload,
            "frm1701q:txt11Address",
            encode_bir_text(&value_at(payload, "guided.registered_address")),
        );
        set_field(
            payload,
            "ui1701q:taxpayer_citizenship",
            encode_bir_text(&value_at(payload, "guided.citizenship")),
        );
        set_field(
            payload,
            "frm1701q:txt19",
            encode_bir_text(&value_at(payload, "guided.business")),
        );
        let taxpayer = value_at(payload, "guided.taxpayer_type");
        for (name, value) in [
            ("single", "ui1701q:taxpayer_type_single"),
            ("professional", "ui1701q:taxpayer_type_professional"),
            ("estate", "ui1701q:taxpayer_type_estate"),
            ("trust", "ui1701q:taxpayer_type_trust"),
        ] {
            set_field(payload, value, (taxpayer == name).to_string())
        }
        let atc = value_at(payload, "guided.atc");
        set_field(payload, "frm1701q:txt20A", atc);
        let rate = value_at(payload, "guided.tax_regime");
        set_field(
            payload,
            "ui1701q:taxpayer_rate_graduated",
            (rate == "graduated").to_string(),
        );
        set_field(
            payload,
            "ui1701q:taxpayer_rate_8",
            (rate == "eight_percent").to_string(),
        );
        let deduction = value_at(payload, "guided.deduction_method");
        set_field(
            payload,
            "frm1701:optMethodOfDeduction23:_1",
            (deduction == "itemized").to_string(),
        );
        set_field(
            payload,
            "frm1701:optMethodOfDeduction23:_2",
            (deduction == "osd").to_string(),
        );
        let credit = value_at(payload, "guided.foreign_credit") == "yes";
        set_field(payload, "frm1701q:SelTreaty_1", credit.to_string());
        set_field(payload, "frm1701q:SelTreaty_2", (!credit).to_string());
        let sales = money(payload, "frm1701q:txt36A");
        let cost = money(payload, "frm1701q:txt37A");
        let deduction_amount = if deduction == "osd" {
            sales * 0.40
        } else {
            money(payload, "frm1701q:txt38C")
        };
        set_field(
            payload,
            "frm1701q:txt38E",
            format!(
                "{:.2}",
                if deduction == "osd" {
                    deduction_amount
                } else {
                    0.0
                }
            ),
        );
        let net = sales - cost - deduction_amount;
        set_field(payload, "frm1701q:txt38G", format!("{net:.2}"));
        set_field(
            payload,
            "frm1701q:txt39A",
            format!(
                "{:.2}",
                net + money(payload, "frm1701q:txt38I") + money(payload, "frm1701q:txt38K")
            ),
        );
        let credits = ["ui1701q:txt55A", "ui1701q:txt56A", "ui1701q:txt58A"]
            .iter()
            .map(|k| money(payload, k))
            .sum::<f64>();
        set_field(payload, "ui1701q:txt62A", format!("{credits:.2}"));
    } else {
        let md = match q {
            1 => "03/31",
            2 => "06/30",
            3 => "09/30",
            _ => "",
        };
        set_field(
            payload,
            "txtYearEnded",
            if md.is_empty() {
                String::new()
            } else {
                format!("{md}/{y}")
            },
        );
        set_field(payload, "txtTIN1", seg.0);
        set_field(payload, "txtTIN2", seg.1);
        set_field(payload, "txtTIN3", seg.2);
        set_field(payload, "txtBranchCode", seg.3);
        for n in 1..=3 {
            set_field(payload, &format!("optQuarter{n}"), (q == n).to_string())
        }
        set_field(payload, "AmendedRtn_1", amended.to_string());
        set_field(payload, "AmendedRtn_2", (!amended).to_string());
        let deduction = value_at(payload, "guided.deduction_method");
        set_field(
            payload,
            "optItemizedDeductions",
            (deduction == "itemized").to_string(),
        );
        set_field(
            payload,
            "optOptionalStandardDeduction",
            (deduction == "osd").to_string(),
        );
        let special = value_at(payload, "guided.special_tax") == "yes";
        set_field(payload, "SpecialTax_1", special.to_string());
        set_field(payload, "SpecialTax_2", (!special).to_string());
        let sales = money(payload, "sched1_txtSales1");
        let cost = money(payload, "sched1_txtCost2");
        let gross = sales - cost;
        let total = gross + money(payload, "sched1_txtOtherIncome4");
        let deductions = if deduction == "osd" {
            total * 0.40
        } else {
            money(payload, "sched1_txtDeductions6")
        };
        set_field(payload, "sched1_txtGross3", format!("{gross:.2}"));
        set_field(payload, "sched1_txtTotalGross5", format!("{total:.2}"));
        set_field(payload, "sched1_txtDeductions6", format!("{deductions:.2}"));
        let taxable = total - deductions;
        set_field(payload, "sched1_txtTaxable7", format!("{taxable:.2}"));
        set_field(
            payload,
            "sched1_txtTotalTaxable9",
            format!("{:.2}", taxable + money(payload, "sched1_txtPrevious8")),
        );
    }
}

pub fn validate(form_code: &str, payload: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let tin = value_at(payload, "profile.tin")
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    if tin.len() != 14 {
        errors
            .push("TIN must contain exactly 14 digits including the five-digit branch code".into())
    }
    let y = value_at(payload, "period.year")
        .parse::<i64>()
        .unwrap_or_default();
    if !(2000..=2100).contains(&y) {
        errors.push("Tax year must be between 2000 and 2100".into())
    }
    let q = value_at(payload, "period.quarter")
        .parse::<i64>()
        .unwrap_or_default();
    if !(1..=3).contains(&q) {
        errors.push("Quarter must be first, second, or third".into())
    }
    for field in fields_for(form_code).iter().chain(COMMON_FIELDS) {
        let value = value_at(payload, field.path);
        if field.required && value.trim().is_empty() {
            errors.push(format!("{} is required", field.label))
        }
        if field.input_type == "number" && field.path != "period.year" && !value.trim().is_empty() {
            match value.replace(',', "").parse::<f64>() {
                Ok(v) if v.is_finite() && v >= 0.0 => {}
                _ => errors.push(format!("{} must be a non-negative number", field.label)),
            }
        }
    }
    let rdo = if form_code == "1701Q" {
        value_at(payload, "field:frm1701q:txt5RDOCode")
    } else {
        value_at(payload, "field:txtRDOCode")
    };
    if rdo.len() != 3 || !rdo.chars().all(|c| c.is_ascii_digit()) {
        errors.push("RDO code must contain exactly three digits".into())
    }
    let allowed_choices: &[(&str, &[&str])] = if form_code == "1701Q" {
        &[
            ("guided.amended", &["yes", "no"]),
            (
                "guided.taxpayer_type",
                &["single", "professional", "estate", "trust"],
            ),
            (
                "guided.atc",
                &["II012", "II014", "II013", "II015", "II017", "II016"],
            ),
            ("guided.tax_regime", &["graduated", "eight_percent"]),
            ("guided.deduction_method", &["itemized", "osd"]),
            ("guided.foreign_credit", &["yes", "no"]),
        ]
    } else {
        &[
            ("guided.amended", &["yes", "no"]),
            (
                "guided.entity_type",
                &["domestic", "resident_foreign", "nonresident_foreign"],
            ),
            ("field:txtATC", &["WC160", "WC170", "WC180"]),
            ("guided.deduction_method", &["itemized", "osd"]),
            ("guided.special_tax", &["yes", "no"]),
        ]
    };
    for (path, allowed) in allowed_choices {
        if !allowed.contains(&value_at(payload, path).as_str()) {
            errors.push(format!("Invalid selection for {path}"));
        }
    }
    if form_code == "1701Q" {
        let month = value_at(payload, "field:frm1701q:txt13BirthMonth")
            .parse::<u8>()
            .unwrap_or_default();
        let day = value_at(payload, "field:frm1701q:txt13BirthDay")
            .parse::<u8>()
            .unwrap_or_default();
        let birth_year = value_at(payload, "field:frm1701q:txt13BirthYear")
            .parse::<i64>()
            .unwrap_or_default();
        if !(1..=12).contains(&month) {
            errors.push("Birth month must be between 1 and 12".into())
        }
        if !(1..=31).contains(&day) {
            errors.push("Birth day must be between 1 and 31".into())
        }
        if !(1900..=y - 18).contains(&birth_year) {
            errors.push("Birth year must represent an adult taxpayer".into())
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn fill_with_schema_samples(form_code: &str, payload: &mut Value) {
    for field in COMMON_FIELDS.iter().chain(fields_for(form_code)) {
        set_value(payload, field.path, field.sample)
    }
    normalize(form_code, payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blank_templates_are_exhaustive_but_materially_incomplete() {
        for code in ["1701Q", "1702Q"] {
            let payload = blank_payload(code, "customer@example.test", 1).unwrap();
            assert!(payload["fields"].as_object().unwrap().len() > 50);
            assert!(validate(code, &payload).is_err());
        }
    }
    #[test]
    fn schema_samples_normalize_and_preserve_1701q_text_encoding() {
        let mut payload = blank_payload("1701Q", "customer@example.test", 1).unwrap();
        fill_with_schema_samples("1701Q", &mut payload);
        assert!(validate("1701Q", &payload).is_ok());
        assert_eq!(
            payload["fields"]["frm1701q:txtTaxPayername"],
            "JUAN%20DELA%20CRUZ"
        );
    }
}
