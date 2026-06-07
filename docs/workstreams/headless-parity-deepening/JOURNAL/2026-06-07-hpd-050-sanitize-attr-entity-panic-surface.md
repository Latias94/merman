# HPD-050 - Sanitizer Attribute Entity Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`sanitize_text(...)` validates URL-bearing attributes through a DOMPurify-like policy. Browser
DOMPurify sees parsed DOM attribute values, where character references have already been decoded.
The Rust port bridges that gap with `decode_attr_html_entities_minimally(...)` before URI
validation.

That bridge still compiled five fixed regexes on first public sanitize use:

```rust
Regex::new(r"(?i)&colon;").expect("valid regex")
Regex::new(r"(?i)&newline;").expect("valid regex")
Regex::new(r"(?i)&tab;").expect("valid regex")
Regex::new(r"(?i)&#0*58;?").expect("valid regex")
Regex::new(r"(?i)&#x0*3a;?").expect("valid regex")
```

## Changes

- Removed the five cached regex helpers from `decode_attr_html_entities_minimally(...)`.
- Replaced named entity replacement with an ASCII case-insensitive literal scanner.
- Replaced decimal and hex colon replacement with source-shaped scanners that preserve the old
  optional-semicolon and prefix-match behavior.
- Preserved the existing replacement order: named colon, newline, tab, decimal colon, hex colon.
- Added helper-level coverage for named, decimal, hex, and non-match cases.
- Added public `remove_script(...)` coverage proving numeric colon references are decoded before
  unsafe `javascript:` URLs are rejected.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core decode_attr_entities_matches_minimal_dompurify_url_subset_without_regex remove_script_decodes_colon_entities_before_url_validation_without_regex sanitize` -
  passed, `30` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'colon_entity_regex|newline_entity_regex|tab_entity_regex|numeric_colon_dec_regex|numeric_colon_hex_regex|Regex::new\(r"\(\?i\)&colon;|Regex::new\(r"\(\?i\)&newline;|Regex::new\(r"\(\?i\)&tab;|Regex::new\(r"\(\?i\)&\#0\*58|Regex::new\(r"\(\?i\)&\#x0\*3a' crates/merman-core/src/sanitize.rs` -
  no sanitizer minimal-entity regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No DOMPurify-like allowed tag/attribute policy, URI allowlist semantics, script/data URL checks,
broad HTML entity decoding, semantic parsing, rendered output, SVG baseline, root viewport formula,
theme behavior, or Architecture residual classification changed. This slice only removes five
avoidable regex construction points from the sanitizer's minimal URL-attribute entity bridge.
