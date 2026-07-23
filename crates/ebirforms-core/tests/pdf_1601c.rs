use ebirforms_core::{parse_1601c_xml, render_1601c_pdf};
use lopdf::content::Content;
use lopdf::{dictionary, Document, Object, Stream};
use std::io::Cursor;

const TEMPLATE_MARKERS: &str =
    "BIR Form No. 1601 - C January 2018 Monthly Remittance Adjustment of Taxes Withheld";

fn template_with_markers(
    pages: usize,
    width: i64,
    height: i64,
    rotate: i64,
    markers: &str,
) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding"
    });
    let mut kids = Vec::new();
    for n in 1..=pages {
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!("q Q\n% ORIGINAL-PAGE-{n}\nBT /FTemplate 10 Tf 20 900 Td ({markers}) Tj ET\n")
                .into_bytes(),
        ));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
            "Rotate" => rotate,
            "Resources" => dictionary! { "Font" => dictionary! { "FTemplate" => font } },
            "Contents" => content,
        });
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => kids, "Count" => pages as i64,
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut out = Cursor::new(Vec::new());
    doc.save_to(&mut out).unwrap();
    out.into_inner()
}

fn template(pages: usize, width: i64, height: i64, rotate: i64) -> Vec<u8> {
    template_with_markers(pages, width, height, rotate, TEMPLATE_MARKERS)
}

fn valid_template() -> Vec<u8> {
    template(2, 612, 936, 0)
}

fn mutate_template(mut bytes: Vec<u8>, mutate: impl FnOnce(&mut Document)) -> Vec<u8> {
    let mut doc = Document::load_mem(&bytes).unwrap();
    mutate(&mut doc);
    bytes.clear();
    doc.save_to(&mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn xml(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut result = String::from("<form>");
    for (name, value) in fields {
        result.push_str(&format!("<field name=\"{name}\">{value}</field>"));
    }
    result.push_str("</form>");
    result.into_bytes()
}

fn text_placements(pdf: &[u8], page: u32) -> Vec<(String, f32, f32)> {
    let doc = Document::load_mem(pdf).unwrap();
    let page_id = doc.get_pages()[&page];
    let mut result = Vec::new();
    for stream_id in doc.get_page_contents(page_id) {
        let stream = doc.get_object(stream_id).unwrap().as_stream().unwrap();
        let bytes = stream.decompressed_content().unwrap();
        let content = Content::decode(&bytes).unwrap();
        let mut position = (0.0, 0.0);
        for operation in content.operations {
            match operation.operator.as_str() {
                "Td" => {
                    position = (
                        operation.operands[0].as_float().unwrap(),
                        operation.operands[1].as_float().unwrap(),
                    );
                }
                "Tj" => {
                    if let Object::String(bytes, _) = &operation.operands[0] {
                        result.push((
                            String::from_utf8_lossy(bytes).into_owned(),
                            position.0,
                            position.1,
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    result
}

fn assert_placement(placements: &[(String, f32, f32)], text: &str, x: f32, y: f32) {
    assert!(
        placements.iter().any(|(found, found_x, found_y)| {
            found == text && (found_x - x).abs() < 0.02 && (found_y - y).abs() < 0.02
        }),
        "missing {text:?} at ({x}, {y}); placements={placements:?}"
    );
}

#[test]
fn parser_accepts_synthetic_and_official_records() {
    let synthetic =
        br#"<form><field name="sched1:txtTotal1">12 &amp; 34</field><meta>x</meta></form>"#;
    let fields = parse_1601c_xml(synthetic).unwrap();
    assert_eq!(fields["sched1:txtTotal1"], "12 & 34");

    let official = br#"<?xml version="1.0"?><div>frm1601c:txtMonth=06frm1601c:txtMonth=</div><div>txtEmail=a&amp;b@example.testtxtEmail=</div>"#;
    let fields = parse_1601c_xml(official).unwrap();
    assert_eq!(fields["txtMonth"], "06");
    assert_eq!(fields["txtEmail"], "a&b@example.test");
}

#[test]
fn parser_handles_multiple_roots_colon_keys_empty_values_and_metadata() {
    let input = br#"<metadata>ignored</metadata><div>frm1601c:sched1:txtTotal1=99.00frm1601c:sched1:txtTotal1=</div><div>frm1601c:txtMonth=frm1601c:txtMonth=</div>"#;
    let fields = parse_1601c_xml(input).unwrap();
    assert_eq!(fields["sched1:txtTotal1"], "99.00");
    assert_eq!(fields["txtMonth"], "");
}

#[test]
fn parser_rejects_duplicates_and_malformed_records() {
    let duplicate =
        br#"<form><field name="txtMonth">01</field><field name="txtMonth">02</field></form>"#;
    assert!(parse_1601c_xml(duplicate)
        .unwrap_err()
        .to_string()
        .contains("duplicate"));
    let malformed = br#"<div>frm1601c:txtMonth=01frm1601c:txtYear=</div>"#;
    assert!(parse_1601c_xml(malformed)
        .unwrap_err()
        .to_string()
        .contains("malformed"));
}

#[test]
fn rejects_unknown_fields_unbalanced_wrappers_and_no_printable_values() {
    for input in [
        br#"<form><field name="futureField">x</field></form>"#.as_slice(),
        br#"<div>frm1601c:futureField=xfrm1601c:futureField=</div>"#.as_slice(),
    ] {
        let error = parse_1601c_xml(input).unwrap_err();
        assert!(error.to_string().contains("unknown 1601C field"), "{error}");
    }

    let error = parse_1601c_xml(br#"<form><field name="txtMonth">01</field>"#).unwrap_err();
    assert!(error.to_string().contains("unclosed <form>"), "{error}");

    for input in [
        b"<form/>".as_slice(),
        br#"<form><field name="txtMonth"> </field><field name="txtCurrentPage">1</field></form>"#
            .as_slice(),
    ] {
        let error = render_1601c_pdf(&valid_template(), input).unwrap_err();
        assert!(
            error.to_string().contains("no nonempty printable"),
            "{error}"
        );
    }
}

#[test]
fn validates_page_count_geometry_and_rotation() {
    for (pdf, message) in [
        (template(1, 612, 936, 0), "exactly 2 pages"),
        (template(2, 611, 936, 0), "MediaBox"),
        (template(2, 612, 936, 90), "rotation"),
    ] {
        let error = render_1601c_pdf(&pdf, &xml(&[("txtMonth", "01")])).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn validates_crop_box_user_unit_and_template_identity() {
    for (key, value, message) in [
        (
            "CropBox",
            Object::Array(vec![0.into(), 0.into(), 600.into(), 936.into()]),
            "CropBox",
        ),
        ("UserUnit", Object::Real(2.0), "UserUnit"),
    ] {
        let pdf = mutate_template(valid_template(), |doc| {
            let page_id = doc.get_pages()[&1];
            let parent = doc
                .get_dictionary(page_id)
                .unwrap()
                .get(b"Parent")
                .unwrap()
                .as_reference()
                .unwrap();
            doc.get_dictionary_mut(parent).unwrap().set(key, value);
        });
        let error = render_1601c_pdf(&pdf, &xml(&[("txtMonth", "01")])).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }

    let wrong = template_with_markers(2, 612, 936, 0, "Some unrelated two page tax form");
    let error = render_1601c_pdf(&wrong, &xml(&[("txtMonth", "01")])).unwrap_err();
    assert!(error.to_string().contains("required marker"), "{error}");
}

#[test]
fn wraps_original_graphics_state_and_flattens_indirect_contents_array() {
    let pdf = mutate_template(valid_template(), |doc| {
        let page_id = doc.get_pages()[&1];
        let old_contents = doc
            .get_dictionary(page_id)
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .clone();
        let stream_id = old_contents.as_reference().unwrap();
        let stream = doc
            .get_object_mut(stream_id)
            .unwrap()
            .as_stream_mut()
            .unwrap();
        let mut content = stream.decompressed_content().unwrap();
        content.extend_from_slice(b"1 0 0 1 100 100 cm\n");
        stream.set_plain_content(content);
        let indirect_array = doc.add_object(Object::Array(vec![old_contents]));
        doc.get_dictionary_mut(page_id)
            .unwrap()
            .set("Contents", indirect_array);
    });
    let output = render_1601c_pdf(&pdf, &xml(&[("txtMonth", "01")])).unwrap();
    let doc = Document::load_mem(&output).unwrap();
    let page = doc.get_dictionary(doc.get_pages()[&1]).unwrap();
    let contents = page.get(b"Contents").unwrap().as_array().unwrap();
    assert_eq!(contents.len(), 3);
    let streams: Vec<&Stream> = contents
        .iter()
        .map(|item| {
            let id = item
                .as_reference()
                .expect("every content item is a reference");
            doc.get_object(id)
                .unwrap()
                .as_stream()
                .expect("every content reference resolves to a stream")
        })
        .collect();
    assert_eq!(streams[0].decompressed_content().unwrap(), b"q\n");
    assert!(
        String::from_utf8_lossy(&streams[1].decompressed_content().unwrap())
            .ends_with("1 0 0 1 100 100 cm\n")
    );
    let overlay = streams[2].decompressed_content().unwrap();
    assert!(String::from_utf8_lossy(&overlay).starts_with("Q\nq\n0 g\n0 G\n"));
}

#[test]
fn rejects_malformed_or_colliding_font_resources_but_creates_missing_ones() {
    for mutation in [
        "resources",
        "resources_ref",
        "font",
        "font_ref",
        "collision",
    ] {
        let pdf = mutate_template(valid_template(), |doc| {
            let page_id = doc.get_pages()[&1];
            let page = doc.get_dictionary_mut(page_id).unwrap();
            match mutation {
                "resources" => page.set("Resources", Object::Integer(7)),
                "resources_ref" => page.set("Resources", Object::Reference((9999, 0))),
                "font" => page.set("Resources", dictionary! { "Font" => 7 }),
                "font_ref" => page.set(
                    "Resources",
                    dictionary! { "Font" => Object::Reference((9999, 0)) },
                ),
                "collision" => page.set(
                    "Resources",
                    dictionary! { "Font" => dictionary! { "EBF1601C_F1" => 7 } },
                ),
                _ => unreachable!(),
            }
        });
        let error = render_1601c_pdf(&pdf, &xml(&[("txtMonth", "01")])).unwrap_err();
        assert!(
            error.to_string().contains("resource"),
            "{mutation}: {error}"
        );
    }

    let missing = mutate_template(valid_template(), |doc| {
        let page_id = doc.get_pages()[&1];
        doc.get_dictionary_mut(page_id)
            .unwrap()
            .remove(b"Resources");
    });
    render_1601c_pdf(&missing, &xml(&[("txtMonth", "01")])).unwrap();
}

#[test]
fn output_is_deterministic_preserves_art_and_appends_known_overlay() {
    let template = valid_template();
    let input = xml(&[
        ("txtTaxpayerName", "KNOWN TAXPAYER"),
        ("txtTax14", "123.45"),
    ]);
    let first = render_1601c_pdf(&template, &input).unwrap();
    let second = render_1601c_pdf(&template, &input).unwrap();
    assert_eq!(first, second);

    let output = Document::load_mem(&first).unwrap();
    assert_eq!(output.get_pages().len(), 2);
    let page1 = output.get_page_content(output.get_pages()[&1]);
    let content = String::from_utf8_lossy(&page1);
    assert!(content.contains("ORIGINAL-PAGE-1"));
    // Guided fields are emitted one glyph per character cell when they fit.
    assert!(content.contains("(K)"));
    assert!(content.contains("(R)"));
    assert!(!content.contains("KNOWN TAXPAYER"));
    assert!(content.contains("(123)"));
    assert!(!content.contains("123.45"));
    let page = output.get_dictionary(output.get_pages()[&1]).unwrap();
    assert!(page.get(b"Contents").unwrap().as_array().unwrap().len() >= 2);
}

#[test]
fn selected_checkbox_is_a_vector_x_not_text() {
    let output = render_1601c_pdf(&valid_template(), &xml(&[("AmendedRtn_1", "true")])).unwrap();
    let doc = Document::load_mem(&output).unwrap();
    let content = String::from_utf8_lossy(&doc.get_page_content(doc.get_pages()[&1])).into_owned();
    assert!(content.contains("184.5 813 m"));
    assert!(content.contains("195 823.5 l"));
    assert!(!content.contains("(X)"));
}

#[test]
fn rejects_exclusive_overlong_and_non_winansi_values() {
    let template = valid_template();
    let exclusive = xml(&[("AmendedRtn_1", "true"), ("AmendedRtn_2", "true")]);
    assert!(render_1601c_pdf(&template, &exclusive)
        .unwrap_err()
        .to_string()
        .contains("mutually exclusive"));

    let overlong = xml(&[("txtMonth", "TOO-LONG")]);
    assert!(render_1601c_pdf(&template, &overlong)
        .unwrap_err()
        .to_string()
        .contains("capacity"));

    let unsupported = xml(&[("txtTaxpayerName", "EMOJI &#x1F600;")]);
    assert!(render_1601c_pdf(&template, &unsupported)
        .unwrap_err()
        .to_string()
        .contains("unsupported WinAnsi"));
}

#[test]
fn calibrated_segmented_header_tin_payment_and_page2_coordinates() {
    let output = render_1601c_pdf(
        &valid_template(),
        &xml(&[
            ("txtMonth", "06"),
            ("txtYear", "2026"),
            ("txtTIN1", "123"),
            ("txtTIN2", "456"),
            ("txtTIN3", "789"),
            ("txtBranchCode", "00000"),
            ("txtAgency37", "BANK"),
            ("txtNumber37", "ABC"),
            ("txtDate37", "07/23/2026"),
            ("txtTaxAgentNo", "AGENT-123"),
            ("txtDateIssue", "01/02/2024"),
            ("txtDateExpiry", "01/02/2027"),
            ("txtPg2TIN1", "123"),
            ("txtPg2TIN2", "456"),
            ("txtPg2TIN3", "789"),
            ("txtPg2BranchCode", "00000"),
            ("txtPg2TaxpayerName", "NAME"),
            ("sched1:txtTotal1", "1,234.5"),
        ]),
    )
    .unwrap();

    let page1 = text_placements(&output, 1);
    assert_placement(&page1, "0", 50.85, 812.0);
    assert_placement(&page1, "6", 65.35, 812.0);
    assert_placement(&page1, "1", 238.77, 781.0);
    assert_placement(&page1, "BANK", 122.0, 137.0);
    assert_placement(&page1, "ABC", 193.0, 137.0);
    assert_placement(&page1, "07/23/2026", 279.0, 137.0);
    assert_placement(&page1, "AGENT-123", 134.0, 182.0);
    assert_placement(&page1, "01/02/2024", 286.0, 179.0);
    assert_placement(&page1, "01/02/2027", 452.0, 179.0);

    let page2 = text_placements(&output, 2);
    assert_placement(&page2, "1", 35.77, 831.0);
    assert_placement(&page2, "NAME", 226.0, 831.0);
    // Item 4 adjustment whole portion ends before x=491 and its normalized
    // cents occupy the two cells beginning at x=506.
    assert_placement(&page2, "1,234", 465.0, 637.0);
    assert_placement(&page2, "5", 510.6, 637.0);
    assert_placement(&page2, "0", 524.6, 637.0);
}

#[test]
fn guided_address_uses_cells_when_it_fits_and_whole_text_when_it_does_not() {
    let short = render_1601c_pdf(&valid_template(), &xml(&[("txtAddress", "AB CD")])).unwrap();
    let placements = text_placements(&short, 1);
    for (text, x) in [
        ("A", 22.3125),
        ("B", 36.7375),
        (" ", 51.1625),
        ("C", 65.5875),
        ("D", 80.0125),
    ] {
        assert_placement(&placements, text, x, 733.0);
    }
    assert!(!placements.iter().any(|(text, _, _)| text == "AB CD"));

    let long = "1234567890123456789012345678901234567890X";
    assert_eq!(long.len(), 41);
    let overflow = render_1601c_pdf(&valid_template(), &xml(&[("txtAddress", long)])).unwrap();
    let placements = text_placements(&overflow, 1);
    assert_placement(&placements, long, 19.5, 733.0);
}

#[test]
fn monetary_values_split_whole_and_cents_and_reject_bad_precision() {
    for (value, expected_whole, expected_cents) in [
        ("123", "123", ["0", "0"]),
        ("-1,234.5", "-1,234", ["5", "0"]),
        ("0.09", "0", ["0", "9"]),
    ] {
        let output = render_1601c_pdf(&valid_template(), &xml(&[("txtTax14", value)])).unwrap();
        let placements = text_placements(&output, 1);
        assert!(placements
            .iter()
            .any(|(text, x, y)| text == expected_whole && *x < 549.0 && (*y - 628.0).abs() < 0.02));
        assert_placement(&placements, expected_cents[0], 569.85, 628.0);
        assert_placement(&placements, expected_cents[1], 584.35, 628.0);
        assert!(!placements
            .iter()
            .any(|(text, _, y)| (*y - 628.0).abs() < 0.02 && text.contains('.')));
    }

    for bad in ["12.345", "1,23.00", "--1.00", "12.xx"] {
        let error = render_1601c_pdf(&valid_template(), &xml(&[("txtTax14", bad)])).unwrap_err();
        assert!(error.to_string().contains("invalid monetary"), "{error}");
    }
}

#[test]
#[ignore = "requires operator-supplied EBIRFORMS_1601C_PDF_TEMPLATE"]
fn renders_operator_supplied_official_template() {
    let path = std::env::var("EBIRFORMS_1601C_PDF_TEMPLATE")
        .expect("set EBIRFORMS_1601C_PDF_TEMPLATE to the January 2018 official PDF");
    let template = std::fs::read(path).unwrap();
    let output = render_1601c_pdf(
        &template,
        &xml(&[
            ("txtMonth", "06"),
            ("txtYear", "2026"),
            ("txtTaxpayerName", "VISUAL OVERLAY TEST"),
            ("AmendedRtn_1", "true"),
            ("txtTax14", "123,456.78"),
            ("sched1:txtTotal1", "123,456.78"),
        ]),
    )
    .unwrap();
    let document = Document::load_mem(&output).unwrap();
    assert_eq!(document.get_pages().len(), 2);
}
