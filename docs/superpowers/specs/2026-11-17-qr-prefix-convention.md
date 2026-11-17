# QR via `qr:` display-string convention

**Supersedes:** `2026-11-17-qr-url-field-design.md` (QrUrlField / KDL-first)

## Convention

Resolved display string of a **Field** or **Text** object:

- Trim outer whitespace.
- Must start with ASCII `qr:` (lowercase).
- Remainder after `qr:` (trimmed) is the QR payload (typically an https URL).
- Max payload length: `rpt_codes::MAX_QR_URL_BYTES` (4096).

Entire string must match; no in-paragraph detection (v1).

## Crystal authoring

- Formula: `"qr:" + {Table.url}`
- Or SQL column: `'qr:' || url`

## Rendering

`rpt-layout` sets `TextPlan.qr_url`, calls `rpt_codes::render_qr_png`, emits `ImageOp` with per-instance asset id `{name}#{instance}`.

## Plan

`docs/superpowers/plans/2026-11-17-qr-prefix-field-text.md`