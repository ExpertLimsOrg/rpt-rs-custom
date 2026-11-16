//! `rpt-codes` — generate QR (URL in base64) or Code 39 barcode PNGs, without a `.rpt` file.

use rpt_codes::{decode_qr_url_base64, mm_to_pixels, render_code39_png, render_qr_png};
use std::io::{self, Write};
use std::process::ExitCode;

const VERSION: &str = concat!("rpt-codes ", env!("CARGO_PKG_VERSION"));

const USAGE: &str = concat!(
    "rpt-codes ",
    env!("CARGO_PKG_VERSION"),
    " — QR or Code 39 barcode → PNG

Generate a standalone PNG (no Crystal report). Exactly one mode:

  -qr, --qr <BASE64>     QR code; BASE64 is standard alphabet (A–Za–z0–9+/) decoding to the URL UTF-8
  -code, --code          Code 39 barcode (requires -value / --value)

OPTIONS
    -value, --value <TEXT>    payload for Code 39 (digits, A–Z, space, and - . $ / + %)
    --width-mm <N>            output width in millimetres (required)
    --height-mm <N>           output height in millimetres (required; for QR must equal --width-mm)
    --dpi <N>         pixels per inch when converting mm (default: 300)
    -o, --output <path>  write PNG here; '-' or omit writes stdout (binary)

    -h, --help        show this help
    -V, --version     show version and exit

EXAMPLES
    rpt-codes -o qr.png --width-mm 40 --height-mm 40 \\
        --qr $(printf '%s' 'https://example.com' | base64)
    rpt-codes -o bar.png --width-mm 80 --height-mm 25 --code --value 'SAMPLE-001'
"
);

#[derive(Debug)]
enum Mode {
    Qr { b64: String },
    Code39 { value: String },
}

#[derive(Debug)]
struct Cli {
    mode: Mode,
    width_mm: f64,
    height_mm: f64,
    dpi: u32,
    output: Option<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_args(args.into_iter()) {
        Ok(Some(c)) => c,
        Ok(None) => return ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("rpt-codes: {msg}\n");
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("rpt-codes: {msg}");
            ExitCode::from(1)
        }
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<Cli>, String> {
    let mut qr: Option<String> = None;
    let mut code = false;
    let mut value: Option<String> = None;
    let mut width_mm: Option<f64> = None;
    let mut height_mm: Option<f64> = None;
    let mut dpi: u32 = 300;
    let mut output: Option<String> = None;

    let mut args = args;
    while let Some(arg) = args.next() {
        let mut take = |flag: &str| -> Result<String, String> {
            args.next()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("{VERSION}");
                return Ok(None);
            }
            "-qr" | "--qr" => {
                if qr.is_some() {
                    return Err("only one --qr value allowed".into());
                }
                qr = Some(take("--qr")?);
            }
            "-code" | "--code" => code = true,
            "-value" | "--value" => value = Some(take("--value")?),
            "--width-mm" => {
                width_mm = Some(parse_positive_mm(&take("--width-mm")?)?);
            }
            "--height-mm" => {
                height_mm = Some(parse_positive_mm(&take("--height-mm")?)?);
            }
            "--dpi" => {
                dpi = take("--dpi")?
                    .parse::<u32>()
                    .map_err(|_| "--dpi must be a positive integer".to_string())?;
                if dpi == 0 {
                    return Err("--dpi must be greater than zero".into());
                }
            }
            "-o" | "--output" => output = Some(take("--output")?),
            other if other.starts_with('-') => return Err(format!("unknown option {other:?}")),
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }

    if qr.is_some() && value.is_some() {
        return Err("--value is only used with --code".into());
    }

    let mode = match (qr, code) {
        (Some(b64), false) => Mode::Qr { b64 },
        (None, true) => {
            let value = value.ok_or_else(|| "--code requires --value".to_string())?;
            if value.is_empty() {
                return Err("--value must not be empty".into());
            }
            Mode::Code39 { value }
        }
        (Some(_), true) => return Err("choose either --qr or --code, not both".into()),
        (None, false) => {
            return Err("one of --qr or --code is required".into());
        }
    };

    let width_mm = width_mm.ok_or_else(|| "--width-mm is required".to_string())?;
    let height_mm = height_mm.ok_or_else(|| "--height-mm is required".to_string())?;

    if matches!(mode, Mode::Qr { .. }) && (width_mm - height_mm).abs() > f64::EPSILON {
        return Err(format!(
            "QR output requires equal --width-mm and --height-mm (got {width_mm} × {height_mm})"
        ));
    }

    Ok(Some(Cli {
        mode,
        width_mm,
        height_mm,
        dpi,
        output,
    }))
}

fn parse_positive_mm(s: &str) -> Result<f64, String> {
    let n: f64 = s
        .parse()
        .map_err(|_| format!("{s:?} is not a valid millimetre size"))?;
    if !n.is_finite() || n <= 0.0 {
        return Err(format!("millimetre size must be positive, got {s:?}"));
    }
    Ok(n)
}

fn run(cli: &Cli) -> Result<(), String> {
    let w = mm_to_pixels(cli.width_mm, cli.dpi);
    let h = mm_to_pixels(cli.height_mm, cli.dpi);

    let png = match &cli.mode {
        Mode::Qr { b64 } => {
            let url = decode_qr_url_base64(b64).map_err(|e| e.to_string())?;
            render_qr_png(&url, w, h).map_err(|e| e.to_string())?
        }
        Mode::Code39 { value } => render_code39_png(value, w, h).map_err(|e| e.to_string())?,
    };

    match cli.output.as_deref() {
        None | Some("-") => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(&png)
                .map_err(|e| format!("could not write stdout: {e}"))?;
            stdout.flush().map_err(|e| format!("could not flush stdout: {e}"))?;
        }
        Some(path) => std::fs::write(path, &png).map_err(|e| format!("could not write {path}: {e}"))?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_without_value_exits_parse_error() {
        let err = parse_args(
            [
                "--code".to_string(),
                "--width-mm".to_string(),
                "10".to_string(),
                "--height-mm".to_string(),
                "10".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("--value"));
    }

    #[test]
    fn qr_and_code_together_rejected() {
        let err = parse_args(
            [
                "--qr".to_string(),
                "dGVzdA==".to_string(),
                "--code".to_string(),
                "--width-mm".to_string(),
                "1".to_string(),
                "--height-mm".to_string(),
                "1".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("not both"));
    }

    #[test]
    fn qr_rejects_mismatched_mm() {
        let err = parse_args(
            [
                "--qr".to_string(),
                "dGVzdA==".to_string(),
                "--width-mm".to_string(),
                "40".to_string(),
                "--height-mm".to_string(),
                "30".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("equal --width-mm"));
    }

    #[test]
    fn qr_rejects_orphan_value() {
        let err = parse_args(
            [
                "--qr".to_string(),
                "dGVzdA==".to_string(),
                "--value".to_string(),
                "X".to_string(),
                "--width-mm".to_string(),
                "10".to_string(),
                "--height-mm".to_string(),
                "10".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("only used with --code"));
    }

    #[test]
    fn qr_mode_parses() {
        let cli = parse_args(
            [
                "--qr".to_string(),
                "dGVzdA==".to_string(),
                "--width-mm".to_string(),
                "40".to_string(),
                "--height-mm".to_string(),
                "40".to_string(),
            ]
            .into_iter(),
        )
        .expect("parse")
        .expect("some");
        assert!(matches!(cli.mode, Mode::Qr { .. }));
    }
}