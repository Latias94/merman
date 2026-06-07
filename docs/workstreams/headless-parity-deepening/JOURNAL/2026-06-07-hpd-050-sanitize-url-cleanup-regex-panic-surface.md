# HPD-050 - sanitize_url Cleanup Regex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`sanitize_url(...)` ports Mermaid's `@braintree/sanitize-url` dependency for public URL
formatting. Its cleanup loop still compiled two static regexes on first use:

```rust
Regex::new(r"(?i)&(newline|tab);").expect("valid regex")
Regex::new(r"(?i)(\\|%5c)((%(6e|72|74))|[nrt])").expect("valid regex")
```

The installed Mermaid CLI dependency resolves `@braintree/sanitize-url` to 7.1.2 while Mermaid
declares `^7.1.1`. The installed source defines:

```ts
export const htmlCtrlEntityRegex = /&(newline|tab);/gi;
export const whitespaceEscapeCharsRegex =
  /(\\|%5[cC])((%(6[eE]|72|74))|[nrt])/g;
```

## Changes

- Removed `regex::Regex` from `crates/merman-core/src/utils.rs`.
- Replaced the named HTML control entity regex helper with a source-shaped byte scanner for
  `&newline;` and `&tab;`.
- Replaced the whitespace escape regex helper with a source-shaped byte scanner for backslash or
  `%5c` / `%5C` followed by `%6e` / `%6E`, `%72`, `%74`, or lowercase literal `n` / `r` / `t`.
- Preserved the cleanup-loop order: decode URI, decode HTML characters, remove named control
  entities, remove Unicode control characters, remove whitespace escapes, trim, decode URI again,
  and repeat while any cleanup shape remains.
- Added helper-level tests for named control entities, whitespace escape branches, and the
  source's lowercase literal `[nrt]` boundary.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize_url` - passed, `3` tests run.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `39` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `rg -n 'html_ctrl_entity_regex|whitespace_escape_chars_regex|Regex|regex::' crates/merman-core/src/utils.rs` -
  no sanitize-url regex dependency or cleanup regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `829`
  lines parsed.

## Boundary

This is a source-backed public URL sanitizer panic-surface cleanup. It does not change
DOMPurify-like `sanitize_text(...)`, DOMPurify generated allowlists, data/ARIA attribute-name
policy, URI allowlist semantics, attribute-whitespace cleanup, script/data guard semantics,
preprocessing, semantic parsing, rendered output, SVG baselines, root viewport formulas, or
Architecture residual classification.
