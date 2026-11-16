//! Errors for QR and barcode generation.

/// A failure while decoding input or rendering a symbol.
#[derive(Debug, thiserror::Error)]
pub enum CodesError {
    /// The base64 payload could not be decoded.
    #[error("invalid base64: {0}")]
    Base64(String),
    /// The decoded bytes are not valid UTF-8 (QR URL).
    #[error("QR payload is not valid UTF-8")]
    Utf8,
    /// The QR URL is empty after decode.
    #[error("QR URL is empty")]
    EmptyUrl,
    /// The decoded QR URL exceeds the supported length.
    #[error("QR URL exceeds the maximum length of {max} bytes (got {got})")]
    QrUrlTooLong {
        /// Maximum allowed byte length after Base64 decode.
        max: usize,
        /// Actual decoded length.
        got: usize,
    },
    /// QR output must be square so modules stay scannable.
    #[error("QR output must be square (got {width_px} × {height_px} pixels)")]
    QrNonSquarePixels {
        /// Requested width in pixels.
        width_px: u32,
        /// Requested height in pixels.
        height_px: u32,
    },
    /// The QR payload is too large for the chosen version.
    #[error("QR encoding failed: {0}")]
    Qr(String),
    /// The value is not encodable as Code 39.
    #[error("Code 39 encoding failed: {0}")]
    Code39(String),
    /// PNG encoding failed.
    #[error("PNG write failed: {0}")]
    Png(String),
    /// Width or height in pixels rounded to zero.
    #[error("output size must be at least one pixel in each dimension")]
    ZeroSize,
}