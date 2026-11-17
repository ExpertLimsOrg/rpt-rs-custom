# QR codes from the `qr:` display-string convention

Field and text objects whose **resolved display string** is exactly `qr:` followed by a URL are rendered as a square QR image in the object's bounding box, one distinct PNG per printed row.

## Authoring in Crystal Reports

Keep the database column as a plain string URL. In the report, either:

- Add a **formula field**: `"qr:" + {YourTable.YourUrlField}`
- Or expose a SQL column: `'qr:' || url AS qr_payload`

Place the field in the detail section. Make the object **square** (equal width and height in twips).

## Rules (v1)

- Prefix is lowercase ASCII `qr:` only (`QR:` is not recognised).
- Outer whitespace on the whole value is trimmed; the URL payload is trimmed after the prefix.
- The entire value must match; `notes qr:http://…` does not trigger QR mode.
- Maximum payload length: 4096 UTF-8 bytes (same as `rpt-codes`).
- QR pixel size uses `min(width, height)` of the object at 300 DPI.

## Errors

If encoding fails, layout emits the usual image placeholder and records a warning diagnostic.

## See also

- `rpt-codes` CLI for standalone PNG generation
- `docs/superpowers/plans/2026-11-17-qr-prefix-field-text.md`