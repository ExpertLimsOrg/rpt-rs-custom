//! `qr:` display-string convention → QR PNG bytes for placement.

use rpt_model::Twips;

pub(crate) const QR_PREFIX: &str = "qr:";
pub(crate) const QR_RENDER_DPI: u32 = 300;

/// When the whole resolved display string is `qr:` + payload, return the payload (URL).
pub(crate) fn qr_payload_from_display(raw: &str) -> Option<&str> {
    let s = raw.trim();
    let rest = s.strip_prefix(QR_PREFIX)?;
    let url = rest.trim();
    if url.is_empty() || url.len() > rpt_codes::MAX_QR_URL_BYTES {
        return None;
    }
    Some(url)
}

/// Square QR pixel size from the object's bounds at `dpi`.
pub(crate) fn qr_side_px_from_bounds(width: Twips, height: Twips, dpi: u32) -> u32 {
    let side_twips = width.0.min(height.0).max(1) as f64;
    let mm = side_twips / 1440.0 * 25.4;
    rpt_codes::mm_to_pixels(mm, dpi)
}

pub(crate) fn try_render_qr_png(
    url: &str,
    side_px: u32,
) -> Result<Vec<u8>, rpt_codes::CodesError> {
    rpt_codes::render_qr_png(url, side_px, side_px)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpt_model::Twips;

    #[test]
    fn accepts_qr_prefix_with_https_url() {
        assert_eq!(
            qr_payload_from_display("qr:https://example.com/x"),
            Some("https://example.com/x")
        );
    }

    #[test]
    fn trims_outer_whitespace_only() {
        assert_eq!(
            qr_payload_from_display("  qr:https://a.b  "),
            Some("https://a.b")
        );
    }

    #[test]
    fn rejects_without_prefix() {
        assert_eq!(qr_payload_from_display("https://x"), None);
        assert_eq!(qr_payload_from_display("QR:https://x"), None);
    }

    #[test]
    fn rejects_empty_payload() {
        assert_eq!(qr_payload_from_display("qr:"), None);
        assert_eq!(qr_payload_from_display("qr:   "), None);
    }

    #[test]
    fn rejects_embedded_prefix_in_sentence() {
        assert_eq!(qr_payload_from_display("see qr:http://x"), None);
    }

    #[test]
    fn side_px_uses_min_dimension() {
        assert!(qr_side_px_from_bounds(Twips(2880), Twips(1440), 300) > 0);
    }

    #[test]
    fn render_fails_when_qr_payload_exceeds_symbol_capacity() {
        let payload = "x".repeat(3500);
        let err = try_render_qr_png(&payload, 64).expect_err("dense payload should not encode");
        assert!(matches!(err, rpt_codes::CodesError::Qr(_)));
    }
}