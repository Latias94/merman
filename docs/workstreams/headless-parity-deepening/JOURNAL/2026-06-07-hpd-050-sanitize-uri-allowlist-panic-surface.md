# HPD-050 - Sanitizer URI Allowlist Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`sanitize_text(...)` validates URL-like attributes through DOMPurify's default URI allowlist before
considering data-URI tag exceptions or `ALLOW_UNKNOWN_PROTOCOLS`.

The Rust port still compiled that allowlist regex on first public sanitizer use:

```rust
Regex::new(r"(?i)^(?:(?:(?:f|ht)tps?|mailto|tel|callto|sms|cid|xmpp):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))")
    .expect("valid regex")
```

Pinned DOMPurify 3.4.0 defines the source rule in
`repo-ref/dompurify/dist/purify.cjs.js`:

```js
const IS_ALLOWED_URI = seal(/^(?:(?:(?:f|ht)tps?|mailto|tel|callto|sms|cid|xmpp|matrix):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i);
```

The pinned source includes `matrix:`. The previous Rust regex did not.

## Changes

- Removed the cached `IS_ALLOWED_URI` regex helper and the final `regex::Regex` dependency from
  `crates/merman-core/src/sanitize.rs`.
- Replaced it with `is_dompurify_allowed_uri(...)`, a source-shaped scanner over the sanitized
  attribute value.
- Preserved DOMPurify's default safe schemes and aligned the pinned `matrix:` scheme:
  `http`, `https`, `ftp`, `ftps`, `mailto`, `tel`, `callto`, `sms`, `cid`, `xmpp`, and `matrix`.
- Preserved the source fallback branches:
  - values starting with a non-ASCII-letter / non-letter byte are allowed as relative-like values;
  - ASCII scheme-like prefixes made from `[A-Za-z+.-]+` are allowed only when followed by a
    non-scheme / non-colon byte or end-of-string.
- Added helper-level source-boundary coverage and public `sanitize_text(...)` coverage proving
  `matrix:` survives while default unknown `foo:` remains stripped.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `37` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'Regex|regex::|dompurify_is_allowed_uri_regex|fn dompurify_is_allowed_uri_regex' crates/merman-core/src/sanitize.rs` -
  no sanitizer regex dependency or URI allowlist regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

This is a source-backed URI allowlist convergence and panic-surface cleanup. It intentionally aligns
the default sanitizer with pinned DOMPurify 3.4.0 by allowing `matrix:`. It does not change
DOMPurify generated allowlists, data/ARIA attribute-name policy, attribute-whitespace cleanup,
script/data guard semantics, minimal HTML entity decoding, tag policy, semantic parsing, rendered
output, SVG baseline, root viewport formula, theme behavior, or Architecture residual
classification.
