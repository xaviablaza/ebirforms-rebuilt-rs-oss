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
        "guided.sales",
        "number",
        "Amount for this quarter",
        true,
        "100000"
    ),
    field!(
        "Cost of sales or services",
        "guided.cost",
        "number",
        "Enter 0 if none",
        true,
        "0"
    ),
    field!(
        "Itemized deductions",
        "guided.itemized_deductions",
        "number",
        "Used only for itemized deductions",
        true,
        "20000"
    ),
    field!(
        "Taxable income from previous quarters",
        "guided.previous_taxable_income",
        "number",
        "Year-to-date prior quarter amount",
        true,
        "0"
    ),
    field!(
        "Other non-operating income",
        "guided.other_income",
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
        if let Some(value) = payload
            .get("guided")
            .and_then(|v| v.get(key))
            .and_then(Value::as_str)
        {
            return value.into();
        }
        let eight_percent = payload
            .pointer("/guided/atc")
            .and_then(Value::as_str)
            .is_some_and(atc_is_eight_percent);
        let legacy_key = match key {
            "sales" if eight_percent => Some("frm1701q:txt40A"),
            "sales" => Some("frm1701q:txt36A"),
            "cost" => Some("frm1701q:txt37A"),
            "itemized_deductions" => Some("frm1701q:txt38C"),
            "previous_taxable_income" if eight_percent => Some("frm1701q:txt40G"),
            "previous_taxable_income" => Some("frm1701q:txt38I"),
            "other_income" if eight_percent => Some("frm1701q:txt40C"),
            "other_income" => Some("frm1701q:txt38K"),
            _ => None,
        };
        legacy_key
            .and_then(|field| payload.get("fields").and_then(|fields| fields.get(field)))
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

fn parse_centavos(value: &str) -> i64 {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let negative = trimmed.starts_with('-');
    let unsigned = trimmed.trim_start_matches(['-', '+']);
    let mut parts = unsigned.splitn(2, '.');
    let whole = parts
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse::<i64>()
        .unwrap_or_default();
    let mut cents = parts
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .take(2)
        .collect::<String>();
    while cents.len() < 2 {
        cents.push('0');
    }
    let amount = whole
        .saturating_mul(100)
        .saturating_add(cents.parse::<i64>().unwrap_or_default());
    if negative {
        -amount
    } else {
        amount
    }
}

fn guided_centavos(payload: &Value, path: &str) -> i64 {
    parse_centavos(&value_at(payload, path))
}

fn field_centavos(payload: &Value, key: &str) -> i64 {
    parse_centavos(&value_at(payload, &format!("field:{key}")))
}

fn format_centavos(value: i64) -> String {
    let negative = value < 0;
    let absolute = value.saturating_abs();
    let whole = absolute / 100;
    let cents = absolute % 100;
    let digits = whole.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}{}.{cents:02}", if negative { "-" } else { "" }, grouped)
}

fn format_plain_centavos(value: i64) -> String {
    let negative = value < 0;
    let absolute = value.saturating_abs();
    format!(
        "{}{}.{:02}",
        if negative { "-" } else { "" },
        absolute / 100,
        absolute % 100
    )
}

fn percent(amount: i64, rate: i64) -> i64 {
    if amount <= 0 {
        0
    } else {
        amount.saturating_mul(rate).saturating_add(50) / 100
    }
}

fn graduated_income_tax(year: i64, taxable_income: i64) -> i64 {
    let income = taxable_income.max(0);
    let pesos = |value: i64| value.saturating_mul(100);
    if year >= 2023 {
        match income {
            value if value <= pesos(250_000) => 0,
            value if value <= pesos(400_000) => percent(value - pesos(250_000), 15),
            value if value <= pesos(800_000) => pesos(22_500) + percent(value - pesos(400_000), 20),
            value if value <= pesos(2_000_000) => {
                pesos(102_500) + percent(value - pesos(800_000), 25)
            }
            value if value <= pesos(8_000_000) => {
                pesos(402_500) + percent(value - pesos(2_000_000), 30)
            }
            value => pesos(2_202_500) + percent(value - pesos(8_000_000), 35),
        }
    } else {
        match income {
            value if value <= pesos(250_000) => 0,
            value if value <= pesos(400_000) => percent(value - pesos(250_000), 20),
            value if value <= pesos(800_000) => pesos(30_000) + percent(value - pesos(400_000), 25),
            value if value <= pesos(2_000_000) => {
                pesos(130_000) + percent(value - pesos(800_000), 30)
            }
            value if value <= pesos(8_000_000) => {
                pesos(490_000) + percent(value - pesos(2_000_000), 32)
            }
            value => pesos(2_410_000) + percent(value - pesos(8_000_000), 35),
        }
    }
}

fn atc_is_eight_percent(atc: &str) -> bool {
    matches!(atc, "II015" | "II017" | "II016")
}

fn set_1701q_atc_fields(payload: &mut Value, atc: &str) {
    let slot = match atc {
        "II012" | "II015" => Some(1),
        "II014" | "II017" => Some(2),
        "II013" | "II016" => Some(3),
        _ => None,
    };
    for (index, suffix) in ["A", "B", "C"].iter().enumerate() {
        let selected = slot == Some(index + 1);
        set_field(
            payload,
            &format!("frm1701q:txt20{suffix}"),
            if selected { atc } else { "" },
        );
        set_field(
            payload,
            &format!("frm1701q:optATC20_{}", index + 1),
            selected.to_string(),
        );
    }
    for code in ["II012", "II014", "II013", "II015", "II017", "II016"] {
        set_field(
            payload,
            &format!("ui1701q:taxpayer_atc_{}", code.to_ascii_lowercase()),
            (atc == code).to_string(),
        );
    }
    let eight_percent = atc_is_eight_percent(atc);
    let graduated = matches!(atc, "II012" | "II014" | "II013");
    set_field(
        payload,
        "ui1701q:taxpayer_rate_graduated",
        graduated.to_string(),
    );
    set_field(
        payload,
        "ui1701q:taxpayer_rate_8",
        eight_percent.to_string(),
    );
}

fn clear_1701q_fields(payload: &mut Value, keys: &[&str]) {
    for key in keys {
        set_field(payload, key, "0.00");
    }
}

fn normalize_1701q_tax(payload: &mut Value, year: i64) {
    const GRADUATED: &[&str] = &[
        "frm1701q:txt36A",
        "frm1701q:txt37A",
        "frm1701q:txt38A",
        "frm1701q:txt38C",
        "frm1701q:txt38E",
        "frm1701q:txt38G",
        "frm1701q:txt38I",
        "frm1701q:txt38K",
        "frm1701q:txt38M",
        "frm1701q:txt39A",
        "ui1701q:txt46A",
    ];
    const EIGHT_PERCENT: &[&str] = &[
        "frm1701q:txt40A",
        "frm1701q:txt40C",
        "frm1701q:txt40E",
        "frm1701q:txt40G",
        "frm1701q:txt41A",
        "ui1701q:txt52A",
        "ui1701q:txt53A",
        "ui1701q:txt54A",
    ];

    let atc = value_at(payload, "guided.atc");
    let eight_percent = atc_is_eight_percent(&atc);
    let deduction = value_at(payload, "guided.deduction_method");
    set_1701q_atc_fields(payload, &atc);
    set_field(
        payload,
        "frm1701:optMethodOfDeduction23:_1",
        (!eight_percent && deduction == "itemized").to_string(),
    );
    set_field(
        payload,
        "frm1701:optMethodOfDeduction23:_2",
        (!eight_percent && deduction == "osd").to_string(),
    );

    let sales = guided_centavos(payload, "guided.sales");
    let cost_input = guided_centavos(payload, "guided.cost");
    let itemized_input = guided_centavos(payload, "guided.itemized_deductions");
    let previous = guided_centavos(payload, "guided.previous_taxable_income");
    let other = guided_centavos(payload, "guided.other_income");
    for (path, amount) in [
        ("guided.sales", sales),
        ("guided.cost", cost_input),
        ("guided.itemized_deductions", itemized_input),
        ("guided.previous_taxable_income", previous),
        ("guided.other_income", other),
    ] {
        set_value(payload, path, &format_plain_centavos(amount));
    }
    if eight_percent {
        clear_1701q_fields(payload, GRADUATED);
        set_field(payload, "frm1701q:txt40A", format_centavos(sales));
        set_field(payload, "frm1701q:txt40C", format_centavos(other));
        let quarter_income = sales + other;
        set_field(payload, "frm1701q:txt40E", format_centavos(quarter_income));
        set_field(payload, "frm1701q:txt40G", format_centavos(previous));
        let cumulative = quarter_income + previous;
        set_field(payload, "frm1701q:txt41A", format_centavos(cumulative));
        let reduction = if matches!(atc.as_str(), "II015" | "II017") {
            250_000 * 100
        } else {
            0
        };
        set_field(payload, "ui1701q:txt52A", format_centavos(reduction));
        let taxable = cumulative - reduction;
        set_field(payload, "ui1701q:txt53A", format_centavos(taxable));
        set_field(
            payload,
            "ui1701q:txt54A",
            format_centavos(percent(taxable, 8)),
        );
    } else {
        clear_1701q_fields(payload, EIGHT_PERCENT);
        let cost = if deduction == "itemized" {
            cost_input
        } else {
            0
        };
        let itemized = if deduction == "itemized" {
            itemized_input
        } else {
            0
        };
        let osd = if deduction == "osd" {
            percent(sales, 40)
        } else {
            0
        };
        set_field(payload, "frm1701q:txt36A", format_centavos(sales));
        set_field(payload, "frm1701q:txt37A", format_centavos(cost));
        let gross = sales - cost;
        set_field(payload, "frm1701q:txt38A", format_centavos(gross));
        set_field(payload, "frm1701q:txt38C", format_centavos(itemized));
        set_field(payload, "frm1701q:txt38E", format_centavos(osd));
        let net = if deduction == "osd" {
            sales - osd
        } else {
            gross - itemized
        };
        set_field(payload, "frm1701q:txt38G", format_centavos(net));
        set_field(payload, "frm1701q:txt38I", format_centavos(previous));
        set_field(payload, "frm1701q:txt38K", format_centavos(other));
        set_field(payload, "frm1701q:txt38M", "0.00");
        let taxable = net + previous + other;
        set_field(payload, "frm1701q:txt39A", format_centavos(taxable));
        set_field(
            payload,
            "ui1701q:txt46A",
            format_centavos(graduated_income_tax(year, taxable)),
        );
    }

    let tax_due = if eight_percent {
        field_centavos(payload, "ui1701q:txt54A")
    } else {
        field_centavos(payload, "ui1701q:txt46A")
    };
    let credits = (55..=61)
        .map(|item| field_centavos(payload, &format!("ui1701q:txt{item}A")))
        .sum::<i64>();
    let payable = tax_due - credits;
    let penalties = (64..=66)
        .map(|item| field_centavos(payload, &format!("ui1701q:txt{item}A")))
        .sum::<i64>();
    let total = payable + penalties;
    for (key, amount) in [
        ("ui1701q:txt62A", credits),
        ("ui1701q:txt63A", payable),
        ("ui1701q:txt67A", penalties),
        ("ui1701q:txt68A", total),
        ("frm1701q:txt26A", tax_due),
        ("frm1701q:txt27A", credits),
        ("frm1701q:txt28A", payable),
        ("frm1701q:txt29A", penalties),
        ("frm1701q:txt30A", total),
        ("frm1701q:txt31A", total),
    ] {
        set_field(payload, key, format_centavos(amount));
    }
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
        let credit = value_at(payload, "guided.foreign_credit") == "yes";
        set_field(payload, "frm1701q:SelTreaty_1", credit.to_string());
        set_field(payload, "frm1701q:SelTreaty_2", (!credit).to_string());
        normalize_1701q_tax(payload, y);
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
    let eight_percent_1701q =
        form_code == "1701Q" && atc_is_eight_percent(&value_at(payload, "guided.atc"));
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
        let deduction_not_applicable =
            eight_percent_1701q && field.path == "guided.deduction_method";
        if field.required && !deduction_not_applicable && value.trim().is_empty() {
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
        let atc = value_at(payload, "guided.atc");
        let expected_regime = match atc.as_str() {
            "II012" | "II014" | "II013" => Some("graduated"),
            "II015" | "II017" | "II016" => Some("eight_percent"),
            _ => None,
        };
        if let Some(expected_regime) = expected_regime {
            if value_at(payload, "guided.tax_regime") != expected_regime {
                errors.push(format!(
                    "ATC {atc} requires the {expected_regime} tax regime"
                ));
            }
        }
        let deduction = value_at(payload, "guided.deduction_method");
        if eight_percent_1701q {
            if !deduction.is_empty() {
                errors.push("The 8% tax regime does not use a deduction method".into());
            }
        } else if !matches!(deduction.as_str(), "itemized" | "osd") {
            errors.push("Graduated rates require an itemized or OSD deduction method".into());
        }
        let cost = guided_centavos(payload, "guided.cost");
        let itemized = guided_centavos(payload, "guided.itemized_deductions");
        if eight_percent_1701q && (cost != 0 || itemized != 0) {
            errors.push("The 8% tax regime cannot include cost or deduction amounts".into());
        } else if deduction == "osd" && (cost != 0 || itemized != 0) {
            errors.push("OSD cannot be combined with cost or itemized deduction amounts".into());
        }
        let taxpayer_type = value_at(payload, "guided.taxpayer_type");
        if matches!(taxpayer_type.as_str(), "estate" | "trust") && atc != "II012" {
            errors.push("Estate and trust filers must use ATC II012".into());
        }
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

    fn field_value<'a>(payload: &'a Value, key: &str) -> &'a str {
        payload["fields"][key].as_str().unwrap_or_default()
    }

    fn sample_1701q() -> Value {
        let mut payload = blank_payload("1701Q", "customer@example.test", 1).unwrap();
        fill_with_schema_samples("1701Q", &mut payload);
        payload
    }

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

    #[test]
    fn every_1701q_atc_sets_the_desktop_slot_rate_and_selection_flags() {
        for (atc, slot, regime) in [
            ("II012", 1, "graduated"),
            ("II014", 2, "graduated"),
            ("II013", 3, "graduated"),
            ("II015", 1, "eight_percent"),
            ("II017", 2, "eight_percent"),
            ("II016", 3, "eight_percent"),
        ] {
            let mut payload = sample_1701q();
            set_value(&mut payload, "guided.atc", atc);
            set_value(&mut payload, "guided.tax_regime", regime);
            if regime == "eight_percent" {
                set_value(&mut payload, "guided.deduction_method", "");
                set_value(&mut payload, "guided.cost", "0");
                set_value(&mut payload, "guided.itemized_deductions", "0");
            }
            normalize("1701Q", &mut payload);

            for index in 1..=3 {
                assert_eq!(
                    field_value(&payload, &format!("frm1701q:optATC20_{index}")),
                    (index == slot).to_string()
                );
                assert_eq!(
                    field_value(
                        &payload,
                        &format!("frm1701q:txt20{}", ["A", "B", "C"][index - 1])
                    ),
                    if index == slot { atc } else { "" }
                );
            }
            for candidate in ["II012", "II014", "II013", "II015", "II017", "II016"] {
                assert_eq!(
                    field_value(
                        &payload,
                        &format!("ui1701q:taxpayer_atc_{}", candidate.to_ascii_lowercase())
                    ),
                    (candidate == atc).to_string()
                );
            }
            assert_eq!(
                field_value(&payload, "ui1701q:taxpayer_rate_graduated"),
                (regime == "graduated").to_string()
            );
            assert_eq!(
                field_value(&payload, "ui1701q:taxpayer_rate_8"),
                (regime == "eight_percent").to_string()
            );
            if regime == "eight_percent" {
                assert_eq!(field_value(&payload, "frm1701q:txt36A"), "0.00");
                assert_eq!(
                    field_value(&payload, "frm1701:optMethodOfDeduction23:_1"),
                    "false"
                );
                assert_eq!(
                    field_value(&payload, "frm1701:optMethodOfDeduction23:_2"),
                    "false"
                );
                assert_eq!(
                    field_value(&payload, "ui1701q:txt52A"),
                    if atc == "II016" { "0.00" } else { "250,000.00" }
                );
                assert_eq!(
                    field_value(&payload, "ui1701q:txt54A"),
                    if atc == "II016" { "8,000.00" } else { "0.00" }
                );
            } else {
                assert_eq!(field_value(&payload, "frm1701q:txt40A"), "0.00");
            }
        }
    }

    #[test]
    fn graduated_and_eight_percent_calculations_match_desktop_semantics() {
        let mut itemized = sample_1701q();
        for (path, value) in [
            ("guided.sales", "1000000"),
            ("guided.cost", "400000"),
            ("guided.itemized_deductions", "100000"),
            ("guided.previous_taxable_income", "50000"),
            ("guided.other_income", "10000"),
            ("field:ui1701q:txt55A", "10000"),
            ("field:ui1701q:txt58A", "5000"),
            ("field:ui1701q:txt64A", "1000"),
        ] {
            set_value(&mut itemized, path, value);
        }
        normalize("1701Q", &mut itemized);
        for (key, value) in [
            ("frm1701:optMethodOfDeduction23:_1", "true"),
            ("frm1701:optMethodOfDeduction23:_2", "false"),
            ("frm1701q:txt38A", "600,000.00"),
            ("frm1701q:txt38G", "500,000.00"),
            ("frm1701q:txt39A", "560,000.00"),
            ("ui1701q:txt46A", "54,500.00"),
            ("frm1701q:txt26A", "54,500.00"),
            ("frm1701q:txt27A", "15,000.00"),
            ("frm1701q:txt28A", "39,500.00"),
            ("frm1701q:txt29A", "1,000.00"),
            ("frm1701q:txt30A", "40,500.00"),
        ] {
            assert_eq!(field_value(&itemized, key), value, "{key}");
        }

        let mut osd = sample_1701q();
        set_value(&mut osd, "guided.sales", "1000000");
        set_value(&mut osd, "guided.cost", "0");
        set_value(&mut osd, "guided.itemized_deductions", "0");
        set_value(&mut osd, "guided.previous_taxable_income", "0");
        set_value(&mut osd, "guided.other_income", "0");
        set_value(&mut osd, "guided.deduction_method", "osd");
        normalize("1701Q", &mut osd);
        assert_eq!(
            field_value(&osd, "frm1701:optMethodOfDeduction23:_1"),
            "false"
        );
        assert_eq!(
            field_value(&osd, "frm1701:optMethodOfDeduction23:_2"),
            "true"
        );
        assert_eq!(field_value(&osd, "frm1701q:txt37A"), "0.00");
        assert_eq!(field_value(&osd, "frm1701q:txt38C"), "0.00");
        assert_eq!(field_value(&osd, "frm1701q:txt38E"), "400,000.00");
        assert_eq!(field_value(&osd, "frm1701q:txt38G"), "600,000.00");
        assert_eq!(field_value(&osd, "ui1701q:txt46A"), "62,500.00");

        let mut eight = sample_1701q();
        for (path, value) in [
            ("guided.atc", "II015"),
            ("guided.tax_regime", "eight_percent"),
            ("guided.deduction_method", ""),
            ("guided.sales", "1000000"),
            ("guided.cost", "0"),
            ("guided.itemized_deductions", "0"),
            ("guided.previous_taxable_income", "100000"),
            ("guided.other_income", "50000"),
            ("field:ui1701q:txt55A", "10000"),
            ("field:ui1701q:txt58A", "5000"),
            ("field:ui1701q:txt64A", "1000"),
            ("field:ui1701q:txt65A", "500"),
            ("field:ui1701q:txt66A", "250"),
        ] {
            set_value(&mut eight, path, value);
        }
        normalize("1701Q", &mut eight);
        for (key, value) in [
            ("frm1701:optMethodOfDeduction23:_1", "false"),
            ("frm1701:optMethodOfDeduction23:_2", "false"),
            ("frm1701q:txt36A", "0.00"),
            ("frm1701q:txt38C", "0.00"),
            ("frm1701q:txt40A", "1,000,000.00"),
            ("frm1701q:txt40E", "1,050,000.00"),
            ("frm1701q:txt41A", "1,150,000.00"),
            ("ui1701q:txt52A", "250,000.00"),
            ("ui1701q:txt53A", "900,000.00"),
            ("ui1701q:txt54A", "72,000.00"),
            ("frm1701q:txt26A", "72,000.00"),
            ("frm1701q:txt27A", "15,000.00"),
            ("frm1701q:txt28A", "57,000.00"),
            ("frm1701q:txt29A", "1,750.00"),
            ("frm1701q:txt30A", "58,750.00"),
            ("frm1701q:txt31A", "58,750.00"),
        ] {
            assert_eq!(field_value(&eight, key), value, "{key}");
        }
        assert!(validate("1701Q", &eight).is_ok());
    }

    #[test]
    fn incompatible_1701q_atc_regime_and_deduction_combinations_are_rejected() {
        let mut payload = sample_1701q();
        set_value(&mut payload, "guided.atc", "II015");
        assert!(validate("1701Q", &payload).is_err());

        set_value(&mut payload, "guided.tax_regime", "eight_percent");
        set_value(&mut payload, "guided.deduction_method", "");
        set_value(&mut payload, "guided.cost", "0");
        set_value(&mut payload, "guided.itemized_deductions", "0");
        assert!(validate("1701Q", &payload).is_ok());

        set_value(&mut payload, "guided.deduction_method", "itemized");
        assert!(validate("1701Q", &payload).is_err());

        set_value(&mut payload, "guided.atc", "II012");
        set_value(&mut payload, "guided.tax_regime", "graduated");
        set_value(&mut payload, "guided.deduction_method", "osd");
        set_value(&mut payload, "guided.cost", "1");
        assert!(validate("1701Q", &payload).is_err());
    }

    #[test]
    fn legacy_drafts_migrate_schedule_inputs_before_canonical_recalculation() {
        let mut payload = sample_1701q();
        let guided = payload["guided"].as_object_mut().unwrap();
        for key in [
            "sales",
            "cost",
            "itemized_deductions",
            "previous_taxable_income",
            "other_income",
        ] {
            guided.remove(key);
        }
        set_field(&mut payload, "frm1701q:txt36A", "1,000,000.00");
        set_field(&mut payload, "frm1701q:txt37A", "400,000.00");
        set_field(&mut payload, "frm1701q:txt38C", "100,000.00");
        set_field(&mut payload, "frm1701q:txt38I", "50,000.00");
        set_field(&mut payload, "frm1701q:txt38K", "10,000.00");

        normalize("1701Q", &mut payload);

        assert_eq!(value_at(&payload, "guided.sales"), "1000000.00");
        assert_eq!(field_value(&payload, "frm1701q:txt36A"), "1,000,000.00");
        assert_eq!(field_value(&payload, "frm1701q:txt38G"), "500,000.00");
        assert_eq!(field_value(&payload, "ui1701q:txt46A"), "54,500.00");
    }
}
