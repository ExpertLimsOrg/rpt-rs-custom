//! Generate QR codes and Code 39 barcodes as PNG bytes.
//!
//! Used by the [`rpt-codes`](../../apps/rpt-codes-cli) CLI; no Crystal report is required.

mod error;

pub use error::CodesError;

use barcoders::sym::code39::Code39;
use base64::Engine;
use image::imageops::FilterType;
use image::{DynamicImage, GrayImage, Luma};
use qrcode::QrCode;

/// Maximum UTF-8 byte length of a QR URL after Base64 decode.
pub const MAX_QR_URL_BYTES: usize = 4096;

/// Decode a standard Base64 string into the UTF-8 QR URL.
///
/// # Errors
///
/// [`CodesError::Base64`], [`CodesError::Utf8`], or [`CodesError::EmptyUrl`].
pub fn decode_qr_url_base64(s: &str) -> Result<String, CodesError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|e| CodesError::Base64(e.to_string()))?;
    if bytes.len() > MAX_QR_URL_BYTES {
        return Err(CodesError::QrUrlTooLong {
            max: MAX_QR_URL_BYTES,
            got: bytes.len(),
        });
    }
    let url = String::from_utf8(bytes).map_err(|_| CodesError::Utf8)?;
    if url.is_empty() {
        return Err(CodesError::EmptyUrl);
    }
    Ok(url)
}

/// Convert millimetres to pixels at `dpi` dots per inch (minimum 1).
pub fn mm_to_pixels(mm: f64, dpi: u32) -> u32 {
    let px = (mm * f64::from(dpi) / 25.4).round();
    px.max(1.0) as u32
}

/// Render `url` as a QR code PNG scaled to `width_px` × `height_px`.
///
/// # Errors
///
/// [`CodesError::Qr`], [`CodesError::ZeroSize`], or [`CodesError::Png`].
pub fn render_qr_png(url: &str, width_px: u32, height_px: u32) -> Result<Vec<u8>, CodesError> {
    if width_px == 0 || height_px == 0 {
        return Err(CodesError::ZeroSize);
    }
    if width_px != height_px {
        return Err(CodesError::QrNonSquarePixels {
            width_px,
            height_px,
        });
    }
    let code = QrCode::new(url.as_bytes()).map_err(|e| CodesError::Qr(e.to_string()))?;
    let qr = code
        .render::<Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(1, 1)
        .build();
    let scaled = image::imageops::resize(&qr, width_px, height_px, FilterType::Nearest);
    encode_png_luma(&scaled)
}

/// Render `value` as a Code 39 barcode PNG scaled to `width_px` × `height_px`.
///
/// # Errors
///
/// [`CodesError::Code39`], [`CodesError::ZeroSize`], or [`CodesError::Png`].
pub fn render_code39_png(
    value: &str,
    width_px: u32,
    height_px: u32,
) -> Result<Vec<u8>, CodesError> {
    if width_px == 0 || height_px == 0 {
        return Err(CodesError::ZeroSize);
    }
    let barcode = Code39::new(value).map_err(|e| CodesError::Code39(e.to_string()))?;
    let bars = barcode.encode();
    let module_px = 2u32;
    let width_native = bars.len() as u32 * module_px;
    let height_native = height_px.max(32);
    let mut img = GrayImage::from_pixel(width_native, height_native, Luma([255u8]));
    for (i, &bar) in bars.iter().enumerate() {
        if bar == 1 {
            for dx in 0..module_px {
                let x = i as u32 * module_px + dx;
                for y in 0..height_native {
                    if let Some(p) = img.get_pixel_mut_checked(x, y) {
                        *p = Luma([0u8]);
                    }
                }
            }
        }
    }
    let scaled = image::imageops::resize(&img, width_px, height_px, FilterType::Nearest);
    encode_png_luma(&scaled)
}

fn encode_png_luma(img: &GrayImage) -> Result<Vec<u8>, CodesError> {
    let dyn_img = DynamicImage::ImageLuma8(img.clone());
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut out);
    dyn_img
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| CodesError::Png(e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_known_base64_url() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"https://example.com");
        assert_eq!(
            decode_qr_url_base64(&b64).expect("decode"),
            "https://example.com"
        );
    }

    #[test]
    fn decode_rejects_invalid_base64() {
        assert!(matches!(
            decode_qr_url_base64("%%%"),
            Err(CodesError::Base64(_))
        ));
    }

    #[test]
    fn decode_rejects_empty_url() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"");
        assert!(matches!(decode_qr_url_base64(&b64), Err(CodesError::EmptyUrl)));
    }

    #[test]
    fn mm_to_pixels_default_dpi() {
        assert_eq!(mm_to_pixels(25.4, 300), 300);
        assert_eq!(mm_to_pixels(0.01, 300), 1);
    }

    #[test]
    fn qr_png_starts_with_signature() {
        let png = render_qr_png("x", 64, 64).expect("qr");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn qr_rejects_non_square_pixels() {
        assert!(matches!(
            render_qr_png("x", 64, 32),
            Err(CodesError::QrNonSquarePixels { .. })
        ));
    }

    #[test]
    fn decode_rejects_oversized_url() {
        let big = vec![b'a'; MAX_QR_URL_BYTES + 1];
        let b64 = base64::engine::general_purpose::STANDARD.encode(&big);
        assert!(matches!(
            decode_qr_url_base64(&b64),
            Err(CodesError::QrUrlTooLong { .. })
        ));
    }

    #[test]
    fn qr_png_dimensions_match_request() {
        let w = 120u32;
        let png = render_qr_png("https://example.com", w, w).expect("qr");
        let img = image::load_from_memory(&png).expect("png");
        assert_eq!(img.width(), w);
        assert_eq!(img.height(), w);
    }

    #[test]
    fn code39_png_dimensions_match_request() {
        let (w, h) = (200u32, 80u32);
        let png = render_code39_png("TEST", w, h).expect("code39");
        let img = image::load_from_memory(&png).expect("png");
        assert_eq!(img.width(), w);
        assert_eq!(img.height(), h);
    }

    #[test]
    fn decode_trims_base64_whitespace() {
        let inner = base64::engine::general_purpose::STANDARD.encode(b"https://x.example");
        let padded = format!("  {inner}  ");
        assert_eq!(
            decode_qr_url_base64(&padded).expect("decode"),
            "https://x.example"
        );
    }

    #[test]
    fn code39_png_starts_with_signature() {
        let png = render_code39_png("TEST-1", 200, 80).expect("code39");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn code39_rejects_unencodable() {
        assert!(matches!(
            render_code39_png("\0", 10, 10),
            Err(CodesError::Code39(_))
        ));
    }
}