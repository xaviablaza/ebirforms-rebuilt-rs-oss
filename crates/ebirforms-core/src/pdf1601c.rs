//! Deterministic overlay renderer for the January 2018 BIR Form 1601C PDF.
//!
//! The official blank form is deliberately not distributed. Callers supply it
//! as bytes; this module performs no network or filesystem access.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;
use std::io::Cursor;
use thiserror::Error;

const FONT_NAME: &[u8] = b"EBF1601C_F1";
const FONT_SIZE: f32 = 8.0;

#[derive(Debug, Error)]
pub enum Pdf1601cError {
    #[error("invalid XML: {0}")]
    Xml(String),
    #[error("duplicate field record: {0}")]
    DuplicateField(String),
    #[error("malformed field record: {0}")]
    MalformedRecord(String),
    #[error("unknown 1601C field: {0}")]
    UnknownField(String),
    #[error("XML contains no nonempty printable 1601C fields")]
    NoPrintableFields,
    #[error("invalid PDF template: {0}")]
    Template(String),
    #[error("mutually exclusive fields are both selected: {0} and {1}")]
    MutuallyExclusive(&'static str, &'static str),
    #[error("field {field} contains unsupported WinAnsi text: {value:?}")]
    UnsupportedText { field: String, value: String },
    #[error("field {field} exceeds its box capacity of {capacity} characters")]
    Overlong { field: String, capacity: usize },
    #[error("field {field} has invalid boolean value {value:?}")]
    InvalidBoolean { field: String, value: String },
    #[error("field {field} has invalid monetary value {value:?}")]
    InvalidAmount { field: String, value: String },
    #[error("could not write PDF: {0}")]
    Pdf(String),
}

/// Parse either eBIR's repeated `<div>key=valuekey=</div>` envelope records or
/// the repository's synthetic `<field name="key">value</field>` fixtures.
pub fn parse_1601c_xml(xml: &[u8]) -> Result<BTreeMap<String, String>, Pdf1601cError> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut fields = BTreeMap::new();
    let mut field_name: Option<String> = None;
    let mut field_value = String::new();
    let mut div_value: Option<String> = None;
    // Keep fragment/multiple-root support while detecting generic wrappers
    // that quick-xml otherwise permits to remain open at EOF.
    let mut elements: Vec<Vec<u8>> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"field" => {
                elements.push(b"field".to_vec());
                if field_name.is_some() {
                    return Err(Pdf1601cError::MalformedRecord("nested <field>".into()));
                }
                let mut name = None;
                for attr in e.attributes() {
                    let attr = attr.map_err(|e| Pdf1601cError::Xml(e.to_string()))?;
                    if attr.key.as_ref() == b"name" {
                        name = Some(
                            attr.decode_and_unescape_value(reader.decoder())
                                .map_err(|e| Pdf1601cError::Xml(e.to_string()))?
                                .into_owned(),
                        );
                    }
                }
                field_name = Some(name.ok_or_else(|| {
                    Pdf1601cError::MalformedRecord("<field> missing name".into())
                })?);
                field_value.clear();
            }
            Ok(Event::Start(e)) if e.name().as_ref() == b"div" => {
                elements.push(b"div".to_vec());
                if div_value.is_some() {
                    return Err(Pdf1601cError::MalformedRecord("nested <div>".into()));
                }
                div_value = Some(String::new());
            }
            Ok(Event::Start(e)) => elements.push(e.name().as_ref().to_vec()),
            Ok(Event::Empty(e)) if e.name().as_ref() == b"field" => {
                let mut name = None;
                for attr in e.attributes() {
                    let attr = attr.map_err(|e| Pdf1601cError::Xml(e.to_string()))?;
                    if attr.key.as_ref() == b"name" {
                        name = Some(
                            attr.decode_and_unescape_value(reader.decoder())
                                .map_err(|e| Pdf1601cError::Xml(e.to_string()))?
                                .into_owned(),
                        );
                    }
                }
                insert_field(
                    &mut fields,
                    name.ok_or_else(|| {
                        Pdf1601cError::MalformedRecord("<field> missing name".into())
                    })?,
                    String::new(),
                )?;
            }
            Ok(Event::Text(e)) => {
                let text = e
                    .unescape()
                    .map_err(|e| Pdf1601cError::Xml(e.to_string()))?
                    .into_owned();
                if field_name.is_some() {
                    field_value.push_str(&text);
                } else if let Some(div) = &mut div_value {
                    div.push_str(&text);
                }
            }
            Ok(Event::CData(e)) => {
                let text = reader
                    .decoder()
                    .decode(e.as_ref())
                    .map_err(|e| Pdf1601cError::Xml(e.to_string()))?;
                if field_name.is_some() {
                    field_value.push_str(&text);
                } else if let Some(div) = &mut div_value {
                    div.push_str(&text);
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"field" => {
                close_element(&mut elements, b"field")?;
                let name = field_name
                    .take()
                    .ok_or_else(|| Pdf1601cError::MalformedRecord("unexpected </field>".into()))?;
                insert_field(&mut fields, name, std::mem::take(&mut field_value))?;
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"div" => {
                close_element(&mut elements, b"div")?;
                let body = div_value
                    .take()
                    .ok_or_else(|| Pdf1601cError::MalformedRecord("unexpected </div>".into()))?;
                if body.contains('=') {
                    let (name, value) = parse_div_record(&body)?;
                    insert_field(&mut fields, name, value)?;
                }
            }
            Ok(Event::End(e)) => close_element(&mut elements, e.name().as_ref())?,
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(Pdf1601cError::Xml(e.to_string())),
        }
        buf.clear();
    }
    if let Some(name) = field_name {
        return Err(Pdf1601cError::MalformedRecord(format!(
            "unclosed <field name={name:?}>"
        )));
    }
    if div_value.is_some() {
        return Err(Pdf1601cError::MalformedRecord("unclosed <div>".into()));
    }
    if let Some(name) = elements.last() {
        return Err(Pdf1601cError::Xml(format!(
            "unclosed <{}> at end of input",
            String::from_utf8_lossy(name)
        )));
    }
    Ok(fields)
}

fn close_element(elements: &mut Vec<Vec<u8>>, name: &[u8]) -> Result<(), Pdf1601cError> {
    match elements.pop() {
        Some(open) if open == name => Ok(()),
        Some(open) => Err(Pdf1601cError::Xml(format!(
            "mismatched closing element </{}> for <{}>",
            String::from_utf8_lossy(name),
            String::from_utf8_lossy(&open)
        ))),
        None => Err(Pdf1601cError::Xml(format!(
            "unexpected closing element </{}>",
            String::from_utf8_lossy(name)
        ))),
    }
}

fn parse_div_record(body: &str) -> Result<(String, String), Pdf1601cError> {
    let (raw_name, rest) = body
        .split_once('=')
        .ok_or_else(|| Pdf1601cError::MalformedRecord(body.into()))?;
    if raw_name.is_empty() {
        return Err(Pdf1601cError::MalformedRecord(body.into()));
    }
    let suffix = format!("{raw_name}=");
    let value = rest
        .strip_suffix(&suffix)
        .ok_or_else(|| Pdf1601cError::MalformedRecord(body.into()))?;
    let name = raw_name.strip_prefix("frm1601c:").unwrap_or(raw_name);
    if name.is_empty() {
        return Err(Pdf1601cError::MalformedRecord(body.into()));
    }
    Ok((name.to_string(), value.to_string()))
}

fn insert_field(
    fields: &mut BTreeMap<String, String>,
    name: String,
    value: String,
) -> Result<(), Pdf1601cError> {
    if !is_known_field(&name) {
        return Err(Pdf1601cError::UnknownField(name));
    }
    if fields.insert(name.clone(), value).is_some() {
        return Err(Pdf1601cError::DuplicateField(name));
    }
    Ok(())
}

const NON_PRINTING_FIELDS: &[&str] = &["txtCurrentPage", "txtMaxPage", "txtLineBus"];

fn is_known_field(name: &str) -> bool {
    LAYOUT.iter().any(|spec| spec.key == name) || NON_PRINTING_FIELDS.contains(&name)
}

#[derive(Clone, Copy, PartialEq)]
enum Alignment {
    Left,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Text(Alignment),
    /// A fixed number of equal-width boxes. Each glyph is centered in its box.
    Segmented {
        cells: usize,
    },
    /// Use the printed character guides when the value fits; otherwise render
    /// the complete value as ordinary contiguous text across the same box.
    AdaptiveSegmented {
        cells: usize,
    },
    /// Whole-number area end, followed by the form's printed decimal marker and
    /// a two-cell cents area. The decimal point is deliberately not overprinted.
    Amount {
        whole_end: f32,
        cents_start: f32,
        cents_end: f32,
    },
    Check,
}

#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    page: u32,
    x: f32,
    y: f32,
    width: f32,
    kind: Kind,
}

macro_rules! t {
    ($k:literal,$p:literal,$x:literal,$y:literal,$w:literal) => {
        Spec {
            key: $k,
            page: $p,
            x: $x as f32,
            y: $y as f32,
            width: $w as f32,
            kind: Kind::Text(Alignment::Left),
        }
    };
}
macro_rules! a {
    ($k:literal,$p:literal,$x:literal,$y:literal,$whole_end:literal,$cents_start:literal,$cents_end:literal) => {
        Spec {
            key: $k,
            page: $p,
            x: $x as f32,
            y: $y as f32,
            width: ($cents_end - $x) as f32,
            kind: Kind::Amount {
                whole_end: $whole_end as f32,
                cents_start: $cents_start as f32,
                cents_end: $cents_end as f32,
            },
        }
    };
}
macro_rules! s {
    ($k:literal,$p:literal,$x:literal,$y:literal,$w:literal,$cells:literal) => {
        Spec {
            key: $k,
            page: $p,
            x: $x as f32,
            y: $y as f32,
            width: $w as f32,
            kind: Kind::Segmented { cells: $cells },
        }
    };
}
macro_rules! ad {
    ($k:literal,$p:literal,$x:literal,$y:literal,$w:literal,$cells:literal) => {
        Spec {
            key: $k,
            page: $p,
            x: $x as f32,
            y: $y as f32,
            width: $w as f32,
            kind: Kind::AdaptiveSegmented { cells: $cells },
        }
    };
}
macro_rules! c {
    ($k:literal,$p:literal,$x:literal,$y:literal) => {
        Spec {
            key: $k,
            page: $p,
            x: $x as f32,
            y: $y as f32,
            width: 10.0,
            kind: Kind::Check,
        }
    };
}

// Coordinates are PDF user-space points (origin at bottom left). This is an
// explicit clean-room map for the two-page January 2018 612 x 936 template.
const LAYOUT: &[Spec] = &[
    s!("txtMonth", 1, 46, 812, 29, 2),
    s!("txtYear", 1, 75, 812, 57, 4),
    c!("AmendedRtn_1", 1, 184.5, 813),
    c!("AmendedRtn_2", 1, 232, 813),
    c!("TaxWithheld_1", 1, 304.5, 813),
    c!("TaxWithheld_2", 1, 346, 813),
    s!("txtSheets", 1, 448, 812, 72, 5),
    t!("txtATC", 1, 534, 812, 60),
    s!("txtTIN1", 1, 234, 781, 43, 3),
    s!("txtTIN2", 1, 291, 781, 43, 3),
    s!("txtTIN3", 1, 348, 781, 43, 3),
    s!("txtBranchCode", 1, 406, 781, 72, 5),
    s!("txtRDOCode", 1, 550, 781, 44, 3),
    // Boxes 8, 9, 10 and 12 use short vertical character guides. Short
    // values are centered one glyph per guide cell; long values remain whole.
    ad!("txtTaxpayerName", 1, 17.5, 758, 577, 40),
    ad!("txtAddress", 1, 17.5, 733, 577, 40),
    ad!("txtAddress2", 1, 17.5, 717, 444.5, 31),
    s!("txtZipCode", 1, 534, 717, 60, 4),
    ad!("txtTelNum", 1, 104, 696, 172, 12),
    c!("CatAgent_P", 1, 449, 695),
    c!("CatAgent_G", 1, 521, 695),
    ad!("txtEmail", 1, 104, 680, 490, 34),
    c!("SpecialTax_1", 1, 175, 662),
    c!("SpecialTax_2", 1, 218, 662),
    t!("selTreaty", 1, 348, 662, 232),
    a!("txtTax14", 1, 391, 628, 549, 565, 594),
    a!("txtTax15", 1, 391, 602, 549, 565, 594),
    a!("txtTax16", 1, 391, 586, 549, 565, 594),
    a!("txtTax17", 1, 391, 570, 549, 565, 594),
    a!("txtTax18", 1, 391, 554, 549, 565, 594),
    a!("txtTax19", 1, 391, 538, 549, 565, 594),
    t!("txt20Other", 1, 204, 522, 186),
    a!("txtTax20", 1, 391, 522, 549, 565, 594),
    a!("txtTax21", 1, 391, 506, 549, 565, 594),
    a!("txtTax22", 1, 391, 490, 549, 565, 594),
    a!("txtTax23", 1, 391, 474, 549, 565, 594),
    a!("txtTax24", 1, 391, 458, 549, 565, 594),
    a!("txtTax25", 1, 391, 442, 549, 565, 594),
    a!("txtTax26", 1, 391, 426, 549, 565, 594),
    a!("txtTax27", 1, 391, 410, 549, 565, 594),
    a!("txtTax28", 1, 391, 394, 549, 565, 594),
    t!("txt29Other", 1, 204, 378, 186),
    a!("txtTax29", 1, 391, 378, 549, 565, 594),
    a!("txtTax30", 1, 391, 362, 549, 565, 594),
    a!("txtTax31", 1, 391, 346, 549, 565, 594),
    a!("txtTax32", 1, 391, 330, 549, 565, 594),
    a!("txtTax33", 1, 391, 314, 549, 565, 594),
    a!("txtTax34", 1, 391, 298, 549, 565, 594),
    a!("txtTax35", 1, 391, 282, 549, 565, 594),
    a!("txtTax36", 1, 391, 266, 549, 565, 594),
    // The accreditation values share the label row. Keep them on its lower
    // baseline so they do not bleed into the signature boxes above.
    t!("txtTaxAgentNo", 1, 132, 182, 141),
    t!("txtDateIssue", 1, 284, 179, 62),
    t!("txtDateExpiry", 1, 450, 179, 56),
    // Payment baselines sit above the ruled lower edge of each handwriting row.
    t!("txtAgency37", 1, 120, 137, 70),
    t!("txtNumber37", 1, 191, 137, 85),
    t!("txtDate37", 1, 277, 137, 113),
    a!("txtAmount37", 1, 391, 137, 549, 565, 594),
    t!("txtAgency38", 1, 120, 119, 70),
    t!("txtNumber38", 1, 191, 119, 85),
    t!("txtDate38", 1, 277, 119, 113),
    a!("txtAmount38", 1, 391, 119, 549, 565, 594),
    t!("txtNumber39", 1, 191, 101, 85),
    t!("txtDate39", 1, 277, 101, 113),
    a!("txtAmount39", 1, 391, 101, 549, 565, 594),
    t!("txtParticular40", 1, 18, 75, 100),
    t!("txtAgency40", 1, 120, 75, 70),
    t!("txtNumber40", 1, 191, 75, 85),
    t!("txtDate40", 1, 277, 75, 113),
    a!("txtAmount40", 1, 391, 75, 549, 565, 594),
    s!("txtPg2TIN1", 2, 31, 831, 43, 3),
    s!("txtPg2TIN2", 2, 74, 831, 43, 3),
    s!("txtPg2TIN3", 2, 117, 831, 43, 3),
    s!("txtPg2BranchCode", 2, 160, 831, 58, 5),
    t!("txtPg2TaxpayerName", 2, 224, 831, 356),
    a!("sched1:txtTotal1", 2, 333, 637, 491, 506, 534),
];

const EXCLUSIVE_PAIRS: &[(&str, &str)] = &[
    ("AmendedRtn_1", "AmendedRtn_2"),
    ("TaxWithheld_1", "TaxWithheld_2"),
    ("CatAgent_P", "CatAgent_G"),
    ("SpecialTax_1", "SpecialTax_2"),
];

/// Overlay parsed 1601C XML values on an operator-supplied official template.
pub fn render_1601c_pdf(template: &[u8], xml: &[u8]) -> Result<Vec<u8>, Pdf1601cError> {
    let fields = parse_1601c_xml(xml)?;
    if !LAYOUT.iter().any(|spec| {
        fields
            .get(spec.key)
            .is_some_and(|value| !value.trim().is_empty())
    }) {
        return Err(Pdf1601cError::NoPrintableFields);
    }
    validate_exclusive(&fields)?;
    let mut doc = Document::load_mem(template)
        .map_err(|e| Pdf1601cError::Template(format!("cannot parse PDF: {e}")))?;
    validate_template(&doc)?;

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier",
        "Encoding" => "WinAnsiEncoding"
    });
    let pages = doc.get_pages();
    for page_number in 1..=2 {
        let page_id = pages[&page_number];
        let operations = overlay_operations(page_number, &fields)?;
        if operations.is_empty() {
            continue;
        }
        install_font_resource(&mut doc, page_id, font_id)?;
        let bytes = Content { operations }
            .encode()
            .map_err(|e| Pdf1601cError::Pdf(e.to_string()))?;
        let stream_id = doc.add_object(Stream::new(dictionary! {}, bytes));
        wrap_and_append_page_content(&mut doc, page_id, stream_id)?;
    }

    let mut out = Cursor::new(Vec::new());
    doc.save_to(&mut out)
        .map_err(|e| Pdf1601cError::Pdf(e.to_string()))?;
    Ok(out.into_inner())
}

fn selected(fields: &BTreeMap<String, String>, key: &str) -> Result<bool, Pdf1601cError> {
    let Some(value) = fields.get(key) else {
        return Ok(false);
    };
    if value.trim().is_empty() {
        return Ok(false);
    }
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "x" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(Pdf1601cError::InvalidBoolean {
            field: key.into(),
            value: value.clone(),
        }),
    }
}

fn validate_exclusive(fields: &BTreeMap<String, String>) -> Result<(), Pdf1601cError> {
    for &(a, b) in EXCLUSIVE_PAIRS {
        if selected(fields, a)? && selected(fields, b)? {
            return Err(Pdf1601cError::MutuallyExclusive(a, b));
        }
    }
    Ok(())
}

fn overlay_operations(
    page: u32,
    fields: &BTreeMap<String, String>,
) -> Result<Vec<Operation>, Pdf1601cError> {
    let mut ops = Vec::new();
    for spec in LAYOUT.iter().filter(|s| s.page == page) {
        let Some(value) = fields.get(spec.key) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        // WW010 is already printed in the January 2018 template.
        if spec.key == "txtATC" && value.trim() == "WW010" {
            continue;
        }
        if spec.kind == Kind::Check {
            if selected(fields, spec.key)? {
                ops.extend(check_operations(spec.x, spec.y));
            }
            continue;
        }
        match spec.kind {
            Kind::Check => unreachable!(),
            Kind::Text(Alignment::Left) => {
                let encoded = encoded_value(spec.key, value)?;
                let capacity = (spec.width / glyph_width()).floor() as usize;
                enforce_capacity(spec.key, encoded.len(), capacity)?;
                // These cells include a printed `(MM/DD/YYYY)` prompt. Mask
                // only that prompt when a real value is present, retaining the
                // surrounding header and ruling.
                match spec.key {
                    "txtDateIssue" => push_white_mask(&mut ops, 282.0, 175.5, 64.0, 10.5),
                    "txtDateExpiry" => push_white_mask(&mut ops, 442.0, 175.5, 66.0, 10.5),
                    _ => {}
                }
                push_text(&mut ops, spec.x + 2.0, spec.y, encoded);
            }
            Kind::Segmented { cells } => {
                let encoded = encoded_value(spec.key, value)?;
                enforce_capacity(spec.key, encoded.len(), cells)?;
                push_segmented_text(&mut ops, spec, cells, encoded);
            }
            Kind::AdaptiveSegmented { cells } => {
                let encoded = encoded_value(spec.key, value)?;
                if encoded.len() <= cells {
                    push_segmented_text(&mut ops, spec, cells, encoded);
                } else {
                    let capacity = ((spec.width - 4.0) / glyph_width()).floor() as usize;
                    enforce_capacity(spec.key, encoded.len(), capacity)?;
                    push_text(&mut ops, spec.x + 2.0, spec.y, encoded);
                }
            }
            Kind::Amount {
                whole_end,
                cents_start,
                cents_end,
            } => {
                let (whole, cents) = split_amount(spec.key, value)?;
                let whole = encoded_value(spec.key, &whole)?;
                let whole_capacity = ((whole_end - spec.x - 2.0) / glyph_width()).floor() as usize;
                enforce_capacity(spec.key, whole.len(), whole_capacity)?;
                let x = whole_end - 2.0 - whole.len() as f32 * glyph_width();
                push_text(&mut ops, x, spec.y, whole);

                // The cents portion has two printed cells. Center each digit so
                // it cannot drift across either the decimal marker or box edge.
                let cents = cents.as_bytes();
                let cell_width = (cents_end - cents_start) / 2.0;
                for (index, byte) in cents.iter().enumerate() {
                    let x = cents_start + cell_width * (index as f32 + 0.5) - glyph_width() / 2.0;
                    push_text(&mut ops, x, spec.y, vec![*byte]);
                }
            }
        }
    }
    if !ops.is_empty() {
        // The official first page leaves a non-black fill color active at the
        // end of its content stream. PDF graphics state carries across page
        // content streams, so establish a deterministic overlay state.
        ops.splice(
            0..0,
            [
                // Restore the graphics state saved before the original page,
                // then establish a private deterministic overlay state.
                Operation::new("Q", vec![]),
                Operation::new("q", vec![]),
                Operation::new("g", vec![0.into()]),
                Operation::new("G", vec![0.into()]),
            ],
        );
        ops.push(Operation::new("Q", vec![]));
    }
    Ok(ops)
}

fn glyph_width() -> f32 {
    FONT_SIZE * 0.6
}

fn push_segmented_text(ops: &mut Vec<Operation>, spec: &Spec, cells: usize, encoded: Vec<u8>) {
    let cell_width = spec.width / cells as f32;
    for (index, byte) in encoded.into_iter().enumerate() {
        let x = spec.x + cell_width * (index as f32 + 0.5) - glyph_width() / 2.0;
        push_text(ops, x, spec.y, vec![byte]);
    }
}

fn encoded_value(field: &str, value: &str) -> Result<Vec<u8>, Pdf1601cError> {
    encode_winansi(value).ok_or_else(|| Pdf1601cError::UnsupportedText {
        field: field.into(),
        value: value.into(),
    })
}

fn enforce_capacity(field: &str, length: usize, capacity: usize) -> Result<(), Pdf1601cError> {
    if length > capacity {
        return Err(Pdf1601cError::Overlong {
            field: field.into(),
            capacity,
        });
    }
    Ok(())
}

fn push_white_mask(ops: &mut Vec<Operation>, x: f32, y: f32, width: f32, height: f32) {
    ops.push(Operation::new("q", vec![]));
    ops.push(Operation::new("g", vec![1.into()]));
    ops.push(Operation::new(
        "re",
        vec![x.into(), y.into(), width.into(), height.into()],
    ));
    ops.push(Operation::new("f", vec![]));
    ops.push(Operation::new("Q", vec![]));
}

fn push_text(ops: &mut Vec<Operation>, x: f32, y: f32, encoded: Vec<u8>) {
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new(
        "Tf",
        vec![Object::Name(FONT_NAME.to_vec()), FONT_SIZE.into()],
    ));
    ops.push(Operation::new("Tr", vec![0.into()]));
    ops.push(Operation::new("Td", vec![x.into(), y.into()]));
    ops.push(Operation::new("Tj", vec![Object::string_literal(encoded)]));
    ops.push(Operation::new("ET", vec![]));
}

/// Validate a display amount and return the whole and normalized two-digit
/// cents portions. Commas are retained for human-readable output.
fn split_amount(field: &str, value: &str) -> Result<(String, String), Pdf1601cError> {
    let invalid = || Pdf1601cError::InvalidAmount {
        field: field.into(),
        value: value.into(),
    };
    let value = value.trim();
    if value.is_empty() || value.starts_with('+') {
        return Err(invalid());
    }
    let (negative, unsigned) = match value.strip_prefix('-') {
        Some(rest) if !rest.is_empty() => (true, rest),
        Some(_) => return Err(invalid()),
        None => (false, value),
    };
    let mut pieces = unsigned.split('.');
    let raw_whole = pieces.next().unwrap_or_default();
    let fraction = pieces.next();
    if pieces.next().is_some() || raw_whole.is_empty() {
        return Err(invalid());
    }
    let groups: Vec<&str> = raw_whole.split(',').collect();
    let valid_whole = if groups.len() == 1 {
        !groups[0].is_empty() && groups[0].bytes().all(|b| b.is_ascii_digit())
    } else {
        (1..=3).contains(&groups[0].len())
            && groups[0].bytes().all(|b| b.is_ascii_digit())
            && groups[1..]
                .iter()
                .all(|g| g.len() == 3 && g.bytes().all(|b| b.is_ascii_digit()))
    };
    if !valid_whole {
        return Err(invalid());
    }
    let cents = match fraction {
        None | Some("") => "00".to_string(),
        Some(one) if one.len() == 1 && one.bytes().all(|b| b.is_ascii_digit()) => {
            format!("{one}0")
        }
        Some(two) if two.len() == 2 && two.bytes().all(|b| b.is_ascii_digit()) => two.to_string(),
        _ => return Err(invalid()),
    };
    let whole = if negative {
        format!("-{raw_whole}")
    } else {
        raw_whole.to_string()
    };
    Ok((whole, cents))
}

fn check_operations(x: f32, y: f32) -> Vec<Operation> {
    const MARK_SIZE: f32 = 10.5;
    vec![
        Operation::new("q", vec![]),
        Operation::new("w", vec![1.2.into()]),
        Operation::new("m", vec![x.into(), y.into()]),
        Operation::new("l", vec![(x + MARK_SIZE).into(), (y + MARK_SIZE).into()]),
        Operation::new("m", vec![(x + MARK_SIZE).into(), y.into()]),
        Operation::new("l", vec![x.into(), (y + MARK_SIZE).into()]),
        Operation::new("S", vec![]),
        Operation::new("Q", vec![]),
    ]
}

fn encode_winansi(value: &str) -> Option<Vec<u8>> {
    value
        .chars()
        .map(|ch| match ch {
            '\u{20}'..='\u{7e}' | '\u{a0}'..='\u{ff}' => Some(ch as u8),
            '\u{20ac}' => Some(0x80),
            '\u{201a}' => Some(0x82),
            '\u{0192}' => Some(0x83),
            '\u{201e}' => Some(0x84),
            '\u{2026}' => Some(0x85),
            '\u{2020}' => Some(0x86),
            '\u{2021}' => Some(0x87),
            '\u{02c6}' => Some(0x88),
            '\u{2030}' => Some(0x89),
            '\u{0160}' => Some(0x8a),
            '\u{2039}' => Some(0x8b),
            '\u{0152}' => Some(0x8c),
            '\u{017d}' => Some(0x8e),
            '\u{2018}' => Some(0x91),
            '\u{2019}' => Some(0x92),
            '\u{201c}' => Some(0x93),
            '\u{201d}' => Some(0x94),
            '\u{2022}' => Some(0x95),
            '\u{2013}' => Some(0x96),
            '\u{2014}' => Some(0x97),
            '\u{02dc}' => Some(0x98),
            '\u{2122}' => Some(0x99),
            '\u{0161}' => Some(0x9a),
            '\u{203a}' => Some(0x9b),
            '\u{0153}' => Some(0x9c),
            '\u{017e}' => Some(0x9e),
            '\u{0178}' => Some(0x9f),
            _ => None,
        })
        .collect()
}

fn validate_template(doc: &Document) -> Result<(), Pdf1601cError> {
    if doc.trailer.has(b"Encrypt") {
        return Err(Pdf1601cError::Template(
            "encrypted PDFs are not supported".into(),
        ));
    }
    let pages = doc.get_pages();
    if pages.len() != 2 {
        return Err(Pdf1601cError::Template(format!(
            "expected exactly 2 pages, found {}",
            pages.len()
        )));
    }
    for (number, id) in &pages {
        let page = doc
            .get_dictionary(*id)
            .map_err(|e| Pdf1601cError::Template(e.to_string()))?;
        let media = inherited(doc, page, b"MediaBox")
            .ok_or_else(|| Pdf1601cError::Template(format!("page {number} has no MediaBox")))?;
        validate_page_box(doc, media, *number, "MediaBox")?;
        if let Some(crop) = inherited(doc, page, b"CropBox") {
            validate_page_box(doc, crop, *number, "CropBox")?;
        }
        if let Some(unit) = inherited(doc, page, b"UserUnit") {
            let unit = resolve(doc, unit).and_then(number_value).ok_or_else(|| {
                Pdf1601cError::Template(format!("page {number} has invalid UserUnit"))
            })?;
            if unit != 1.0 {
                return Err(Pdf1601cError::Template(format!(
                    "page {number} UserUnit must be 1"
                )));
            }
        }
        let rotation = inherited(doc, page, b"Rotate")
            .and_then(|o| resolve(doc, o))
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        if rotation != 0 {
            return Err(Pdf1601cError::Template(format!(
                "page {number} rotation must be 0"
            )));
        }
    }

    const TEXT_LIMIT: usize = 4 * 1024 * 1024;
    let text = doc
        .extract_text_with_limit(&[1, 2], TEXT_LIMIT)
        .map_err(|e| Pdf1601cError::Template(format!("cannot inspect template text: {e}")))?;
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for marker in [
        "BIR Form No.",
        "1601 - C",
        "January 2018",
        "Monthly Remittance",
        "Adjustment of Taxes Withheld",
    ] {
        if !normalized.contains(marker) {
            return Err(Pdf1601cError::Template(format!(
                "template text is missing required marker {marker:?}"
            )));
        }
    }
    Ok(())
}

fn validate_page_box(
    doc: &Document,
    value: &Object,
    number: u32,
    name: &str,
) -> Result<(), Pdf1601cError> {
    let array = resolve(doc, value)
        .and_then(|o| o.as_array().ok())
        .ok_or_else(|| Pdf1601cError::Template(format!("page {number} has invalid {name}")))?;
    if array.len() != 4
        || number_value(&array[0]) != Some(0.0)
        || number_value(&array[1]) != Some(0.0)
        || number_value(&array[2]) != Some(612.0)
        || number_value(&array[3]) != Some(936.0)
    {
        return Err(Pdf1601cError::Template(format!(
            "page {number} {name} must be [0 0 612 936]"
        )));
    }
    Ok(())
}

fn inherited<'a>(doc: &'a Document, start: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    let mut dict = start;
    loop {
        if let Ok(value) = dict.get(key) {
            return Some(value);
        }
        let parent = dict.get(b"Parent").ok()?.as_reference().ok()?;
        dict = doc.get_dictionary(parent).ok()?;
    }
}

fn resolve<'a>(doc: &'a Document, object: &'a Object) -> Option<&'a Object> {
    match object {
        Object::Reference(id) => doc.get_object(*id).ok(),
        other => Some(other),
    }
}

fn number_value(object: &Object) -> Option<f32> {
    match object {
        Object::Integer(v) => Some(*v as f32),
        Object::Real(v) => Some(*v),
        _ => None,
    }
}

fn install_font_resource(
    doc: &mut Document,
    page_id: ObjectId,
    font_id: ObjectId,
) -> Result<(), Pdf1601cError> {
    let mut resources = {
        let page = doc
            .get_dictionary(page_id)
            .map_err(|e| Pdf1601cError::Template(e.to_string()))?;
        match inherited(doc, page, b"Resources") {
            None => Dictionary::new(),
            Some(value) => resolve(doc, value)
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .ok_or_else(|| {
                    Pdf1601cError::Template(format!("page resources are malformed or unresolvable"))
                })?,
        }
    };
    let mut fonts = match resources.get(b"Font") {
        Err(_) => Dictionary::new(),
        Ok(value) => resolve(doc, value)
            .and_then(|o| o.as_dict().ok())
            .cloned()
            .ok_or_else(|| {
                Pdf1601cError::Template("page Font resources are malformed or unresolvable".into())
            })?,
    };
    if fonts.has(FONT_NAME) {
        return Err(Pdf1601cError::Template(format!(
            "page Font resource /{} already exists",
            String::from_utf8_lossy(FONT_NAME)
        )));
    }
    fonts.set(FONT_NAME, Object::Reference(font_id));
    resources.set("Font", Object::Dictionary(fonts));
    doc.get_dictionary_mut(page_id)
        .map_err(|e| Pdf1601cError::Pdf(e.to_string()))?
        .set("Resources", Object::Dictionary(resources));
    Ok(())
}

fn wrap_and_append_page_content(
    doc: &mut Document,
    page_id: ObjectId,
    overlay_id: ObjectId,
) -> Result<(), Pdf1601cError> {
    let existing = doc
        .get_dictionary(page_id)
        .map_err(|e| Pdf1601cError::Pdf(e.to_string()))?
        .get(b"Contents")
        .ok()
        .cloned();
    let prefix_id = doc.add_object(Stream::new(dictionary! {}, b"q\n".to_vec()));
    let mut items = vec![Object::Reference(prefix_id)];
    if let Some(existing) = existing {
        flatten_page_contents(doc, existing, &mut items)?;
    }
    items.push(Object::Reference(overlay_id));
    doc.get_dictionary_mut(page_id)
        .map_err(|e| Pdf1601cError::Pdf(e.to_string()))?
        .set("Contents", Object::Array(items));
    Ok(())
}

fn flatten_page_contents(
    doc: &mut Document,
    object: Object,
    output: &mut Vec<Object>,
) -> Result<(), Pdf1601cError> {
    match object {
        Object::Array(items) => {
            for item in items {
                flatten_page_contents(doc, item, output)?;
            }
        }
        Object::Reference(id) => match doc.get_object(id).cloned() {
            Ok(Object::Stream(_)) => output.push(Object::Reference(id)),
            Ok(Object::Array(items)) => {
                for item in items {
                    flatten_page_contents(doc, item, output)?;
                }
            }
            Ok(_) => {
                return Err(Pdf1601cError::Template(
                    "page Contents reference is neither a stream nor an array".into(),
                ))
            }
            Err(e) => {
                return Err(Pdf1601cError::Template(format!(
                    "unresolvable page Contents reference: {e}"
                )))
            }
        },
        Object::Stream(stream) => {
            let id = doc.add_object(stream);
            output.push(Object::Reference(id));
        }
        _ => {
            return Err(Pdf1601cError::Template(
                "page Contents is neither a stream nor an array".into(),
            ))
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_covers_every_printable_mapping_key_exactly_once() {
        let mapping = include_str!("../forms/1601C/mapping.toml");
        let excluded = ["txtCurrentPage", "txtMaxPage", "txtLineBus"];
        let expected: Vec<&str> = mapping
            .lines()
            .filter_map(|line| line.strip_prefix('"')?.split_once('"').map(|(key, _)| key))
            .filter(|key| !excluded.contains(key))
            .collect();
        assert_eq!(expected.len(), LAYOUT.len());
        for key in expected {
            assert_eq!(
                LAYOUT.iter().filter(|spec| spec.key == key).count(),
                1,
                "{key}"
            );
        }
        for key in excluded {
            assert!(!LAYOUT.iter().any(|spec| spec.key == key));
        }
    }
}
