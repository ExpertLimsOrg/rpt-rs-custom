# QR URL field object — design spec (C)

**Date:** 2026-11-17  
**Obsidian:** `notes/qr-url-field-design.md` (canonical for vault navigation)

## Summary

Introduce `ReportObjectKind::QrUrlField` bound to a database string field containing a per-row URL. At layout time, `rpt-layout` calls `rpt-codes::render_qr_png` and emits per-instance PNG assets into the Page IR, reusing the same PDF image path as blob fields.

## Non-goals (v1)

- Crystal binary record decoding for a native QR object (deferred).
- Formula-driven QR payloads.
- Code 39 (follow-up variant).

## Components

1. **`QrUrlFieldObject`** — `data_source: String` only (mirror `BlobFieldObject`).
2. **KDL** — `qr-url-field` node for v1 authoring and tests.
3. **`rpt-layout`** — new emitter; depends on `rpt-codes`.
4. **`rpt-query::used_fields`** — register bound field.

## Layout contract

- Resolve `data_source` per row via existing field resolution.
- Square QR: pixel size from `min(bounds.width, bounds.height)` and report DPI (default 300).
- Asset key: `{object_name}#{instance}`.
- Empty URL or encoding failure → placeholder + diagnostic (match blob/picture patterns).

## Limits

- `MAX_QR_URL_BYTES` (4096) from `rpt-codes`.

## Tests

Integration test: three rows, three URLs, three distinct PNG assets in output document.

## Implementation phases

1. Model + KDL + layout + test.  
2. User-facing docs + meridian example.  
3. `.rpt` reader support if wire format is identified.