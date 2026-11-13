//! Optional local `test2` dump (`.tmp-test2.json` at repo root): footer must not paint past margins.

use rpt_data::{build_dataset, EmptySource};
use rpt_reader::model::Report;
use rpt_pages::DrawOp;
use rpt_render::{
    render_backend, render_dataset_with, CosmicLayout, FontProvider, FontSource, Locale,
    PdfBackend, PdfOptions,
};
use rpt_test_support::workspace_root;
use serde_json::Value;

#[test]
fn test2_json_footer_pdf_ink_stays_inside_content_width() {
    let path = workspace_root().join(".tmp-test2.json");
    if !path.exists() {
        eprintln!("skip: {}", path.display());
        return;
    }
    let raw = std::fs::read_to_string(&path).expect("read test2 json");
    let v: Value = serde_json::from_str(&raw).expect("parse test2 json");
    let report: Report = serde_json::from_value(v["model"].clone()).expect("report model");
    let content_w = report.print_options.content_width.0;
    let ds = build_dataset(&EmptySource, &report.data_definition);
    let doc = render_dataset_with(
        &report,
        &ds,
        Box::new(CosmicLayout::new(FontProvider::bundled())),
        Locale::default(),
        None,
        None,
    );
    for op in &doc.pages[0].ops {
        let DrawOp::Text(t) = op else { continue };
        let Some(src) = t.source.as_ref() else { continue };
        if src.section != "PageFooterSection1" {
            continue;
        }
        assert!(
            t.bounds.left.0 + t.bounds.width.0 <= content_w,
            "{:?} exceeds content width",
            src.object_name
        );
        if let Some(m) = &t.metrics {
            assert!(
                m.advance.0 <= t.bounds.width.0 + 64,
                "{:?} layout advance {} wider than box {}",
                src.object_name,
                m.advance.0,
                t.bounds.width.0
            );
        }
    }
    let pdf = render_backend(
        &doc,
        &PdfBackend,
        &PdfOptions {
            fonts: FontSource::Bundled,
            ..Default::default()
        },
    );
    let out = workspace_root().join(".tmp-test2-fixed.pdf");
    std::fs::write(&out, &pdf).expect("write pdf");
    assert!(pdf.starts_with(b"%PDF"));
}
