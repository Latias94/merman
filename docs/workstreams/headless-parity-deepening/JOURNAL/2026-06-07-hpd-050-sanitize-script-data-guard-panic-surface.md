# HPD-050 - Sanitizer Script/Data Guard Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

When `dompurifyConfig.ALLOW_UNKNOWN_PROTOCOLS` is true, DOMPurify still blocks script-like and
`data:` URI values with `IS_SCRIPT_OR_DATA` after removing `ATTR_WHITESPACE`.

The Rust port still compiled that fixed guard regex on first public sanitizer use:

```rust
Regex::new(r"(?i)^(?:\w+script|data):").expect("valid regex")
```

Pinned DOMPurify 3.4.0 defines the source rule in
`repo-ref/dompurify/dist/purify.cjs.js`:

```js
const IS_SCRIPT_OR_DATA = seal(/^(?:\w+script|data):/i);
```

## Changes

- Removed the cached `IS_SCRIPT_OR_DATA` regex helper from
  `crates/merman-core/src/sanitize.rs`.
- Replaced it with `is_dompurify_script_or_data_uri(...)`, a source-shaped scanner over the URI
  prefix before the first colon.
- Preserved JavaScript regex `\w` semantics as ASCII `[A-Za-z0-9_]`.
- Preserved the source boundary that `data:` matches directly, while `\w+script:` requires at
  least one ASCII word character before `script`.
- Added helper-level boundary coverage for case-insensitive script/data forms, ASCII word prefixes,
  `script:` non-match, non-word hyphen/non-ASCII prefixes, and missing-colon input.
- Added public `sanitize_text(...)` coverage proving `ALLOW_UNKNOWN_PROTOCOLS` keeps an unknown
  `foo:` URI but still removes `javascript:` and `data:` href values.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `35` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'dompurify_is_script_or_data_regex|fn dompurify_is_script_or_data_regex|Regex::new\(r"\(\?i\)\^\(\?:\\w\+script\|data\):' crates/merman-core/src/sanitize.rs` -
  no sanitizer script/data guard regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No DOMPurify generated allowlists, data/ARIA attribute-name policy, URI allowlist regex semantics,
attribute-whitespace cleanup, minimal HTML entity decoding, tag policy, semantic parsing, rendered
output, SVG baseline, root viewport formula, theme behavior, or Architecture residual
classification changed. This slice only removes one avoidable regex construction point from the
sanitizer's `ALLOW_UNKNOWN_PROTOCOLS` script/data URI guard.
