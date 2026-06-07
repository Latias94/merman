# HPD-050 - Sanitizer Attribute Whitespace Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`sanitize_text(...)` validates URL-like attributes through a DOMPurify-like URI policy. DOMPurify
first removes a narrow attribute-whitespace character class from the parsed attribute value before
testing `IS_ALLOWED_URI` and `IS_SCRIPT_OR_DATA`.

The Rust port still compiled that fixed character class on first public sanitizer use:

```rust
Regex::new(r"[\u{0000}-\u{0020}\u{00A0}\u{1680}\u{180E}\u{2000}-\u{2029}\u{205F}\u{3000}]")
    .expect("valid regex")
```

Pinned DOMPurify 3.4.0 defines the source rule in
`repo-ref/dompurify/dist/purify.cjs.js`:

```js
const ATTR_WHITESPACE = seal(/[\u0000-\u0020\u00A0\u1680\u180E\u2000-\u2029\u205F\u3000]/g);
```

## Changes

- Removed the cached sanitizer attribute-whitespace regex helper from
  `crates/merman-core/src/sanitize.rs`.
- Replaced it with `remove_dompurify_attr_whitespace(...)`, a source-shaped scanner that returns a
  borrowed value when no cleanup is needed.
- Preserved the exact DOMPurify 3.4.0 character class:
  `U+0000..U+0020`, `U+00A0`, `U+1680`, `U+180E`, `U+2000..U+2029`, `U+205F`, and `U+3000`.
- Preserved the cleanup timing before both URI allowlist validation and the
  `ALLOW_UNKNOWN_PROTOCOLS` script/data guard.
- Added helper-level boundary coverage and public `sanitize_text(...)` coverage proving
  `java\u00A0script:` is collapsed before rejecting an unsafe `href`.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core sanitize` - passed, `33` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'dompurify_attr_whitespace_regex|fn dompurify_attr_whitespace_regex|Regex::new\(r"\[\\u\{0000\}-\\u\{0020\}|Regex::new\(r"\[\\u0000-\\u0020' crates/merman-core/src/sanitize.rs` -
  no sanitizer attribute-whitespace regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No DOMPurify generated allowlists, data/ARIA attribute-name policy, URI allowlist regex semantics,
script/data URL regex semantics, minimal HTML entity decoding, tag policy, semantic parsing,
rendered output, SVG baseline, root viewport formula, theme behavior, or Architecture residual
classification changed. This slice only removes one avoidable regex construction point from the
sanitizer's pre-URI attribute whitespace cleanup.
