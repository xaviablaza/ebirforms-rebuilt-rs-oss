pub const MAX_BOX_NUMBER: usize = 68;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeductionMethod {
    Itemized,
    OptionalStandard,
}

pub fn encode_bir_text(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

pub fn decode_bir_text(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (
                char::from(bytes[index + 1]).to_digit(16),
                char::from(bytes[index + 2]).to_digit(16),
            ) {
                decoded.push(((high << 4) | low) as u8);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

pub fn parse_money_to_centavos(value: &str) -> i64 {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let negative = trimmed.starts_with('-');
    let unsigned = trimmed.trim_start_matches(['-', '+']);
    let mut parts = unsigned.splitn(2, '.');
    let whole_digits: String = parts
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let mut centavos_digits: String = parts
        .next()
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .take(2)
        .collect();
    while centavos_digits.len() < 2 {
        centavos_digits.push('0');
    }

    let whole = whole_digits.parse::<i64>().unwrap_or(0);
    let centavos = centavos_digits.parse::<i64>().unwrap_or(0);
    let amount = whole.saturating_mul(100).saturating_add(centavos);
    if negative { -amount } else { amount }
}

pub fn format_centavos(value: i64) -> String {
    let negative = value < 0;
    let absolute = value.saturating_abs();
    let whole = absolute / 100;
    let centavos = absolute % 100;
    let digits = whole.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}{}.{centavos:02}", if negative { "-" } else { "" }, grouped)
}

fn percent(amount: i64, rate: i64) -> i64 {
    if amount <= 0 {
        return 0;
    }
    amount.saturating_mul(rate).saturating_add(50) / 100
}

pub fn graduated_income_tax(year: i32, taxable_income: i64) -> i64 {
    let income = taxable_income.max(0);
    let pesos = |value: i64| value.saturating_mul(100);

    if year >= 2023 {
        match income {
            value if value <= pesos(250_000) => 0,
            value if value <= pesos(400_000) => percent(value - pesos(250_000), 15),
            value if value <= pesos(800_000) => pesos(22_500) + percent(value - pesos(400_000), 20),
            value if value <= pesos(2_000_000) => pesos(102_500) + percent(value - pesos(800_000), 25),
            value if value <= pesos(8_000_000) => pesos(402_500) + percent(value - pesos(2_000_000), 30),
            value => pesos(2_202_500) + percent(value - pesos(8_000_000), 35),
        }
    } else {
        match income {
            value if value <= pesos(250_000) => 0,
            value if value <= pesos(400_000) => percent(value - pesos(250_000), 20),
            value if value <= pesos(800_000) => pesos(30_000) + percent(value - pesos(400_000), 25),
            value if value <= pesos(2_000_000) => pesos(130_000) + percent(value - pesos(800_000), 30),
            value if value <= pesos(8_000_000) => pesos(490_000) + percent(value - pesos(2_000_000), 32),
            value => pesos(2_410_000) + percent(value - pesos(8_000_000), 35),
        }
    }
}

pub fn calculate_column(
    year: i32,
    graduated_rate: bool,
    deduction_method: DeductionMethod,
    boxes: &mut [i64; MAX_BOX_NUMBER + 1],
) {
    boxes[38] = boxes[36] - boxes[37];
    boxes[40] = if deduction_method == DeductionMethod::OptionalStandard {
        percent(boxes[36], 40)
    } else {
        0
    };
    boxes[41] = match deduction_method {
        DeductionMethod::Itemized => boxes[38] - boxes[39],
        DeductionMethod::OptionalStandard => boxes[36] - boxes[40],
    };
    boxes[45] = boxes[41] + boxes[42] + boxes[43] + boxes[44];
    boxes[46] = graduated_income_tax(year, boxes[45]);

    boxes[49] = boxes[47] + boxes[48];
    boxes[51] = boxes[49] + boxes[50];
    boxes[53] = boxes[51] - boxes[52];
    boxes[54] = percent(boxes[53], 8);

    boxes[62] = (55..=61).map(|item| boxes[item]).sum();
    let tax_due = if graduated_rate { boxes[46] } else { boxes[54] };
    boxes[63] = tax_due - boxes[62];
    boxes[67] = boxes[64] + boxes[65] + boxes[66];
    boxes[68] = boxes[63] + boxes[67];

    boxes[26] = tax_due;
    boxes[27] = boxes[62];
    boxes[28] = boxes[63];
    boxes[29] = boxes[67];
    boxes[30] = boxes[68];
}

pub fn aggregate_amount_payable(taxpayer: i64, spouse: i64) -> i64 {
    taxpayer + spouse
}

pub fn is_calculated_box(item: &str) -> bool {
    matches!(
        item,
        "26" | "27" | "28" | "29" | "30" | "31" | "38" | "40" | "41" | "45" | "46"
            | "49" | "51" | "53" | "54" | "62" | "63" | "67" | "68"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pesos(value: i64) -> i64 {
        value * 100
    }

    #[test]
    fn renderer_contains_every_numbered_box_and_sub_box() {
        let source = include_str!("main.rs");
        for item in 1..=MAX_BOX_NUMBER {
            assert!(
                source.contains(&format!("(\"{item}\",")),
                "1701Q renderer is missing Box {item}"
            );
        }
        for item in ["10A", "16A", "25A"] {
            assert!(
                source.contains(&format!("(\"{item}\",")),
                "1701Q renderer is missing Box {item}"
            );
        }
        assert!(!source[ source.find("fn render_1701q_physical_form").unwrap() .. source.find("fn render_1701q_digits_box").unwrap() ].contains("Calendar"));
        assert!(!source[ source.find("fn render_1701q_physical_form").unwrap() .. source.find("fn render_1701q_digits_box").unwrap() ].contains("Fiscal"));
    }

    #[test]
    fn bir_text_encoding_round_trips_human_readable_profile_values() {
        let human = "JUAN DELA CRUZ – 1/F";
        let encoded = encode_bir_text(human);
        assert_eq!(encoded, "JUAN%20DELA%20CRUZ%20%E2%80%93%201%2FF");
        assert_eq!(decode_bir_text(&encoded), human);
    }

    #[test]
    fn money_parser_and_formatter_preserve_negative_centavos_and_grouping() {
        assert_eq!(parse_money_to_centavos("-1,234,567.89"), -123_456_789);
        assert_eq!(format_centavos(-123_456_789), "-1,234,567.89");
        assert_eq!(parse_money_to_centavos("250000"), 25_000_000);
        assert_eq!(format_centavos(25_000_000), "250,000.00");
    }

    #[test]
    fn graduated_itemized_schedule_calculates_and_rolls_into_part_three() {
        let mut boxes = [0; MAX_BOX_NUMBER + 1];
        boxes[36] = pesos(1_000_000);
        boxes[37] = pesos(400_000);
        boxes[39] = pesos(100_000);
        boxes[42] = pesos(50_000);
        boxes[43] = pesos(10_000);
        boxes[44] = pesos(40_000);
        boxes[55] = pesos(10_000);
        boxes[58] = pesos(5_000);
        boxes[64] = pesos(1_000);
        boxes[65] = pesos(500);
        boxes[66] = pesos(250);

        calculate_column(2026, true, DeductionMethod::Itemized, &mut boxes);

        assert_eq!(boxes[38], pesos(600_000));
        assert_eq!(boxes[40], 0);
        assert_eq!(boxes[41], pesos(500_000));
        assert_eq!(boxes[45], pesos(600_000));
        assert_eq!(boxes[46], pesos(62_500));
        assert_eq!(boxes[62], pesos(15_000));
        assert_eq!(boxes[63], pesos(47_500));
        assert_eq!(boxes[67], pesos(1_750));
        assert_eq!(boxes[68], pesos(49_250));
        assert_eq!(boxes[26], boxes[46]);
        assert_eq!(boxes[27], boxes[62]);
        assert_eq!(boxes[28], boxes[63]);
        assert_eq!(boxes[29], boxes[67]);
        assert_eq!(boxes[30], boxes[68]);
    }

    #[test]
    fn osd_and_eight_percent_schedules_calculate_from_their_source_boxes() {
        let mut graduated = [0; MAX_BOX_NUMBER + 1];
        graduated[36] = pesos(1_000_000);
        graduated[37] = pesos(250_000);
        calculate_column(2026, true, DeductionMethod::OptionalStandard, &mut graduated);
        assert_eq!(graduated[40], pesos(400_000));
        assert_eq!(graduated[41], pesos(600_000));

        let mut eight_percent = [0; MAX_BOX_NUMBER + 1];
        eight_percent[47] = pesos(1_000_000);
        eight_percent[48] = pesos(50_000);
        eight_percent[50] = pesos(100_000);
        eight_percent[52] = pesos(250_000);
        calculate_column(2026, false, DeductionMethod::Itemized, &mut eight_percent);
        assert_eq!(eight_percent[49], pesos(1_050_000));
        assert_eq!(eight_percent[51], pesos(1_150_000));
        assert_eq!(eight_percent[53], pesos(900_000));
        assert_eq!(eight_percent[54], pesos(72_000));
        assert_eq!(eight_percent[26], boxes_value(&eight_percent, 54));
    }

    fn boxes_value(boxes: &[i64; MAX_BOX_NUMBER + 1], item: usize) -> i64 {
        boxes[item]
    }

    #[test]
    fn tax_tables_switch_at_2023_and_aggregate_combines_columns() {
        assert_eq!(graduated_income_tax(2022, pesos(600_000)), pesos(80_000));
        assert_eq!(graduated_income_tax(2023, pesos(600_000)), pesos(62_500));
        assert_eq!(aggregate_amount_payable(pesos(49_250), -pesos(10_000)), pesos(39_250));
    }

    #[test]
    fn every_formula_output_is_marked_calculated() {
        for item in [
            "26", "27", "28", "29", "30", "31", "38", "40", "41", "45", "46", "49",
            "51", "53", "54", "62", "63", "67", "68",
        ] {
            assert!(is_calculated_box(item), "Box {item} must be readonly/calculated");
        }
        for item in ["36", "37", "39", "42", "47", "48", "55", "64"] {
            assert!(!is_calculated_box(item), "Box {item} must remain an input");
        }
    }
}
