//! Layout tests for the `qr:` display-string → QR image convention.

use super::*;

const PNG_SIG: &[u8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

fn square_qr_field(name: &str, data_source: &str) -> ReportObject {
    let mut o = ReportObject::default();
    o.name = name.to_string();
    o.bounds = Rect {
        left: Twips(100),
        top: Twips(0),
        width: Twips(1440),
        height: Twips(1440),
    };
    let mut f = FieldObject::default();
    f.data_source = data_source.to_string();
    f.ref_kind = FieldRefKind::DatabaseField;
    f.value_type = FieldValueType::String;
    o.kind = ReportObjectKind::Field(Box::new(f));
    o
}

fn detail_report(obj: ReportObject) -> Report {
    let mut report = tiny_report(5000);
    report.report_definition.areas[1].sections[0].objects = vec![obj];
    report
}

#[test]
fn three_rows_emit_three_distinct_qr_assets() {
    let report = detail_report(square_qr_field("QrCell", "{t.url}"));
    let saved = saved_data(
        &[("t.url", FieldValueType::String)],
        &[
            &["qr:https://example.com/1"],
            &["qr:https://example.com/2"],
            &["qr:https://example.com/3"],
        ],
    );
    let doc = rendered(&report, &saved);
    let image_ids: Vec<_> = doc
        .pages
        .iter()
        .flat_map(|p| &p.ops)
        .filter_map(|op| match op {
            DrawOp::Image(i) => Some(i.image_id.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        image_ids
            .iter()
            .any(|id| id.starts_with("QrCell#")),
        "expected per-instance QR image ids, got {image_ids:?}"
    );
    let mut keys: Vec<_> = doc
        .assets
        .keys()
        .filter(|k| k.starts_with("QrCell#"))
        .collect();
    keys.sort();
    assert_eq!(keys.len(), 3, "one asset per detail row");
    assert!(
        doc.assets[keys[0]].bytes.starts_with(PNG_SIG),
        "asset should be PNG"
    );
    let b0 = &doc.assets[keys[0]].bytes;
    let b1 = &doc.assets[keys[1]].bytes;
    let b2 = &doc.assets[keys[2]].bytes;
    assert_ne!(b0, b1);
    assert_ne!(b1, b2);
    assert!(
        !doc.pages.iter().flat_map(|p| &p.ops).any(|op| {
            matches!(op, DrawOp::Text(t) if t.text.starts_with("qr:"))
        }),
        "QR rows must not emit literal qr: text"
    );
}

#[test]
fn plain_url_renders_text_not_qr() {
    let report = detail_report(square_qr_field("UrlText", "{t.url}"));
    let saved = saved_data(
        &[("t.url", FieldValueType::String)],
        &[&["https://only-text.example"]],
    );
    let doc = rendered(&report, &saved);
    assert!(
        doc.pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::Text(t) if t.text.contains("https://only-text"))),
        "expected text draw op"
    );
    assert!(
        !doc.assets.values().any(|a| a.bytes.starts_with(PNG_SIG)),
        "should not register QR PNG assets"
    );
}

#[test]
fn bare_qr_prefix_falls_back_to_text_path_without_panic() {
    let mut o = text_object("Bare", "qr:", 0);
    o.bounds.height = Twips(1440);
    o.bounds.width = Twips(1440);
    let report = detail_report(o);
    let doc = rendered(&report, &empty_saved());
    assert!(!doc.pages.is_empty());
    assert!(
        doc.pages[0]
            .ops
            .iter()
            .any(|op| matches!(op, DrawOp::Text(t) if t.text == "qr:")),
        "invalid qr: prefix should render as text"
    );
    assert!(
        !doc.assets.values().any(|a| a.bytes.starts_with(PNG_SIG)),
        "invalid qr: must not allocate QR assets"
    );
}

#[test]
fn payload_over_max_bytes_renders_as_text_not_qr() {
    let report = detail_report(square_qr_field("QrBig", "{t.url}"));
    let huge = format!("qr:{}", "x".repeat(5000));
    let saved = saved_data(
        &[("t.url", FieldValueType::String)],
        &[&[huge.as_str()]],
    );
    let doc = rendered(&report, &saved);
    assert!(
        !doc.assets.values().any(|a| a.bytes.starts_with(PNG_SIG)),
        "payload > 4096 bytes must not enter QR mode"
    );
    assert!(
        doc.pages[0].ops.iter().any(|op| {
            matches!(op, DrawOp::Text(t) if t.text.starts_with("qr:"))
        }),
        "oversized value should fall through to text rendering"
    );
}