# HPD-050 - Sanitizer Data/ARIA Attribute Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`sanitize_text(...)` mirrors DOMPurify's default attribute-name policy for `data-*` and `aria-*`
attributes before falling back to the generated default attribute allowlist.

The Rust port still compiled two fixed regexes on first public sanitizer use:

```rust
Regex::new(r"^data-[\-\w.\u{00B7}-\u{FFFF}]+$").expect("valid regex")
Regex::new(r"^aria-[\-\w]+$").expect("valid regex")
```

Pinned DOMPurify 3.4.0 defines the equivalent source rules in
`repo-ref/dompurify/dist/purify.cjs.js`:

```js
const DATA_ATTR = seal(/^data-[\-\w.\u00B7-\uFFFF]+$/);
const ARIA_ATTR = seal(/^aria-[\-\w]+$/);
```

## Changes

- Removed the two cached data/ARIA attribute-name regex helpers from
  `crates/merman-core/src/sanitize.rs`.
- Replaced them with source-shaped scanners for DOMPurify's `DATA_ATTR` and `ARIA_ATTR` rules.
- Preserved the current validation order and configuration behavior:
  - `ALLOW_DATA_ATTR` plus `!FORBID_ATTR` gates source-shaped `data-*` names;
  - `ALLOW_ARIA_ATTR` gates source-shaped `aria-*` names before the default allowlist fallback.
- Added helper-level boundary coverage for ASCII word, hyphen, dot, U+00B7, U+FFFF, empty suffix,
  colon, dot-in-ARIA, and non-BMP non-match cases.
- Expanded public `sanitize_text(...)` coverage so valid `data-*` / `aria-*` names survive while
  invalid source-shape neighbors and unknown attributes are removed.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `31` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'dompurify_(data|aria)_attr_regex|fn dompurify_.*attr_regex|Regex::new\(r"\^data-|Regex::new\(r"\^aria-' crates/merman-core/src/sanitize.rs` -
  no sanitizer data/ARIA attribute-name regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No DOMPurify generated allowlists, URI allowlist semantics, whitespace cleanup, script/data URL
checks, minimal HTML entity decoding, tag policy, semantic parsing, rendered output, SVG baseline,
root viewport formula, theme behavior, or Architecture residual classification changed. This slice
only removes two avoidable regex construction points from sanitizer `data-*` / `aria-*`
attribute-name validation.
