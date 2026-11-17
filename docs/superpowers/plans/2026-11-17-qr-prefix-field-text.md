# Convención `qr:` en campos de texto — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a field or text object's resolved display string is exactly `qr:` + URL, render a per-row QR PNG in the object box instead of drawing text.

**Architecture:** Parse prefix in a small `rpt-layout` module; set `TextPlan.qr_url` during `text_plan`; emit via `emit_qr_url_field` mirroring `emit_blob_field` asset keys; depend on `rpt-codes::render_qr_png`. No new `ReportObjectKind` — Crystal-authored reports use formula/SQL prefix.

**Tech Stack:** Rust, `rpt-layout`, `rpt-codes`, `rpt-pages` Page IR, existing layout tests.

## Global Constraints

- Prefix is literal ASCII `qr:` (lowercase), case-sensitive on the prefix only.
- Entire resolved display string must match `qr:` + payload (trim whitespace on full string only); no substring match inside paragraphs (v1).
- Payload UTF-8 length ≤ `rpt_codes::MAX_QR_URL_BYTES` (4096).
- QR bitmap is square: pixel size from `min(bounds.width, bounds.height)` at **300 DPI** (constant `QR_RENDER_DPI = 300`).
- Asset id per instance: `{object_name}#{instance}` (same as blob fields).
- On failure: placeholder image op + layout diagnostic (same pattern as blob decode failure).
- Do not change rendering for strings without the prefix.

---

## File map

| File | Role |
|------|------|
| `crates/rpt-layout/src/qr_url.rs` | Parse `qr:` payload, twips→px, encode PNG |
| `crates/rpt-layout/src/lib.rs` | `mod qr_url`; `TextPlan.qr_url` field |
| `crates/rpt-layout/src/paginate.rs` | Early return in `text_plan`; `section_is_blank` treats QR as content |
| `crates/rpt-layout/src/place.rs` | `emit_qr_url_field`; branch in `emit_object` |
| `crates/rpt-layout/Cargo.toml` | `rpt-codes` dependency |
| `crates/rpt-layout/src/tests/qr_url.rs` | Integration tests (new module) |
| `docs/rendering/qr-url-prefix.md` | Author docs (Crystal + SQL) |
| `notes/qr-url-field-design.md` | Point to prefix convention (Obsidian) |
| `CHANGELOG.md` | Added entry under Unreleased |

---

### Task 1: Parse helper + unit tests

**Files:**
- Create: `crates/rpt-layout/src/qr_url.rs`
- Modify: `crates/rpt-layout/src/lib.rs` (add `mod qr_url;`)
- Test: inline `#[cfg(test)]` in `qr_url.rs`

**Interfaces:**
- Produces: `pub(crate) fn qr_payload_from_display(raw: &str) -> Option<&str>`
- Produces: `pub(crate) fn qr_side_px_from_bounds(width: Twips, height: Twips, dpi: u32) -> u32`
- Produces: `pub(crate) fn try_render_qr_png(url: &str, side_px: u32) -> Result<Vec<u8>, rpt_codes::CodesError>`

- [ ] **Step 1: Write failing tests**

```rust
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
    use rpt_model::Twips;
    assert!(qr_side_px_from_bounds(Twips(2880), Twips(1440), 300) > 0);
}
```

- [ ] **Step 2: Run tests (expect FAIL)**

```bash
cargo test -p rpt-layout qr_payload -- --nocapture
```

Expected: module/function not found.

- [ ] **Step 3: Implement minimal parse + twips→px**

```rust
//! `qr:` display-string convention → QR PNG bytes.
use rpt_model::Twips;

pub(crate) const QR_PREFIX: &str = "qr:";
pub(crate) const QR_RENDER_DPI: u32 = 300;

pub(crate) fn qr_payload_from_display(raw: &str) -> Option<&str> {
    let s = raw.trim();
    let rest = s.strip_prefix(QR_PREFIX)?;
    let url = rest.trim();
    if url.is_empty() || url.len() > rpt_codes::MAX_QR_URL_BYTES {
        return None;
    }
    Some(url)
}

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
```

- [ ] **Step 4: Add `rpt-codes` to `crates/rpt-layout/Cargo.toml`**

```toml
rpt-codes = { workspace = true }
```

- [ ] **Step 5: Run tests (expect PASS)**

```bash
cargo test -p rpt-layout qr_payload qr_side -- --nocapture
```

- [ ] **Step 6: Commit** (only if user requested commits)

```bash
git add crates/rpt-layout/Cargo.toml crates/rpt-layout/src/lib.rs crates/rpt-layout/src/qr_url.rs
git commit -m "feat(layout): add qr: prefix parsing and QR sizing helpers"
```

---

### Task 2: `TextPlan` + pagination

**Files:**
- Modify: `crates/rpt-layout/src/lib.rs` (`TextPlan`)
- Modify: `crates/rpt-layout/src/paginate.rs`

**Interfaces:**
- Consumes: `qr_payload_from_display`
- Produces: `TextPlan { qr_url: Option<String>, ... }` — `Some` skips text wrap

- [ ] **Step 1: Extend `TextPlan`**

```rust
pub(crate) struct TextPlan {
  // existing fields...
  /// When set, emit QR for this URL instead of text lines.
  pub(crate) qr_url: Option<String>,
}
```

Update every `TextPlan { ... }` literal in the crate (search `TextPlan {`) to add `qr_url: None` until set in `text_plan`.

- [ ] **Step 2: Short-circuit in `text_plan` after `raw` is computed**

Immediately after the `match` that sets `raw` (before font substitution / wrap loop):

```rust
if let Some(url) = crate::qr_url::qr_payload_from_display(&raw) {
    return Some(TextPlan {
        lines: Vec::new(),
        color,
        kind,
        currency,
        qr_url: Some(url.to_string()),
    });
}
```

- [ ] **Step 3: Fix `section_is_blank` for text/field objects**

In the arm that checks `p.lines.iter().all(|l| l.text.trim().is_empty())`, treat QR as non-blank:

```rust
plans.get(i).and_then(Option::as_ref).map(|p| {
    p.qr_url.is_some()
        || p.lines.iter().all(|l| l.text.trim().is_empty())
})
```

(Negate logic: section is blank only when NOT qr and lines empty — read existing predicate carefully and invert so `qr_url` ⇒ object has content.)

- [ ] **Step 4: Compile**

```bash
cargo test -p rpt-layout --no-run
```

Fix any missed `TextPlan` initializers.

- [ ] **Step 5: Commit** (if requested)

---

### Task 3: Emit QR image in `place.rs`

**Files:**
- Modify: `crates/rpt-layout/src/place.rs`

**Interfaces:**
- Consumes: `TextPlan.qr_url`, `try_render_qr_png`, `qr_side_px_from_bounds`
- Produces: `Formatter::emit_qr_url_field` (private)

- [ ] **Step 1: Write failing integration test** (Task 4 file first is OK — TDD order: add test skeleton that fails on missing ImageOp)

- [ ] **Step 2: Add `emit_qr_url_field`**

Mirror `emit_blob_field` (lines ~640–680): build `rect`, `image_id = format!("{}#{}", obj.name, at.instance)`, call `try_render_qr_png`, insert `ImageAsset` with `image/png`, `DrawOp::Image` with `ImageFit::Contain`, else placeholder `DrawOp::Image` keyed by `obj.name` and `Diagnostic` with kind e.g. `UnsupportedObject` or reuse existing image warning pattern.

Use `QR_RENDER_DPI` from `qr_url.rs`.

- [ ] **Step 3: Branch at start of `emit_text_object` OR in `emit_object` match**

Preferred: at top of `emit_object`, after building `Placement`:

```rust
if let Some(plan) = plan {
    if let Some(url) = &plan.qr_url {
        self.emit_qr_url_field(obj, url, at);
        return;
    }
}
```

Then existing `Field|Text|FieldHeading => emit_text_object` unchanged.

- [ ] **Step 4: Ensure field headings never set `qr_url`** (only field/text paths in `text_plan`; headings use static text — no change).

- [ ] **Step 5: Compile**

```bash
cargo test -p rpt-layout --no-run
```

---

### Task 4: Layout integration tests

**Files:**
- Create: `crates/rpt-layout/src/tests/qr_url.rs`
- Modify: `crates/rpt-layout/src/tests/mod.rs` (`mod qr_url;`)

**Interfaces:**
- Consumes: `layout`, `Dataset`, helpers from `tests/mod.rs`

- [ ] **Step 1: Test three rows → three distinct image assets**

```rust
// Build minimal Report: one detail section, one FieldObject bound to string field.
// Dataset with 3 rows: "qr:https://a/1", "qr:https://a/2", "qr:https://a/3"
// Square field bounds 1440×1440 twips.
// After layout(&report, &dataset, &registry):
// - Count DrawOp::Image in detail for object name
// - Assert document.assets has keys FieldName#0, #1, #2 with distinct bytes
// - PNG magic 0x89 'P' 'N' 'G'
```

- [ ] **Step 2: Test plain URL without prefix still renders Text op**

One row `"https://only-text.example"` → at least one `DrawOp::Text`, no QR asset.

- [ ] **Step 3: Test `qr:` with empty payload → placeholder, no panic**

Row value `"qr:"` after format → no QR asset (parser returns None) → empty text lines; assert no panic.

- [ ] **Step 4: Run full layout tests**

```bash
cargo test -p rpt-layout qr_url --
cargo test -p rpt-layout
```

Expected: all pass.

- [ ] **Step 5: Commit** (if requested)

---

### Task 5: Documentation + Obsidian + CHANGELOG

**Files:**
- Create: `docs/rendering/qr-url-prefix.md`
- Modify: `notes/qr-url-field-design.md`, `notes/rpt-rs.md`
- Modify: `CHANGELOG.md` (Unreleased → Added)

- [ ] **Step 1: Author doc**

Sections:
- Convención `qr:https://…`
- Crystal: formula `"qr:" + {Table.url}` or SQL `'qr:' || url`
- Cuadrado el objeto en el diseñador
- Límites y errores
- No aplica a texto mezclado (v1)

- [ ] **Step 2: Update Obsidian note** — supersede diseño C; link `[[rpt-rs]]`; estado "implementado" when code lands.

- [ ] **Step 3: CHANGELOG bullet**

```markdown
- Field and text objects whose resolved value is `qr:` followed by a URL render as a QR code (per-row PNG via `rpt-codes`) instead of literal text.
```

- [ ] **Step 4: Link from `docs/rendering/01-pipeline.md` or README rendering index** (one line cross-link).

- [ ] **Step 5: Commit** (if requested)

---

## Spec self-review

| Requirement | Task |
|-------------|------|
| `qr:` prefix, lowercase | Task 1 tests |
| Full-string match only | Task 1 `rejects_embedded_prefix` |
| Crystal field + formula path | Task 5 docs |
| Per-row PNG | Task 3 asset `#instance` |
| No change without prefix | Task 4 second test |
| 4096 byte limit | Task 1 parse |
| Square QR from box | Task 1 `side_px`, Task 3 |
| Placeholder on failure | Task 3 |

No placeholders remain in plan steps.

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-11-17-qr-prefix-field-text.md`.

**1. Subagent-Driven (recommended)** — one subagent per task, review between tasks.

**2. Inline Execution** — same session, checkpoints after Tasks 2 and 4.

Which approach?