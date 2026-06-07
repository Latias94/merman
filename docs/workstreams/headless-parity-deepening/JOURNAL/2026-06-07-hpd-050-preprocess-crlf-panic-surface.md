# HPD-050 - Preprocess CRLF Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the detector comment cleanup removed the last detector-registry regex construction point,
`cleanup_text(...)` still used the cached preprocess regex helper for CRLF normalization:

```rust
Regex::new(r"\r\n?").expect("preprocess regex must compile")
```

The pattern is a static literal, so this is not a user-input syntax panic. It is still avoidable
regex construction on a public preprocessing path for input containing `\r`, and the behavior is a
simple Mermaid-shaped line-ending normalization.

## Changes

- Removed the `re_crlf` cached regex from preprocess.
- Added `normalize_crlf(...)`, a direct scanner that maps both `\r\n` and CR-only line endings to
  `\n`.
- Kept normalization before frontmatter, directive, detector, and comment cleanup handling.
- Added focused helper and public preprocess regressions for CRLF / CR-only input.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core normalize_crlf_matches_mermaid_line_ending_cleanup preprocess_normalizes_crlf_without_regex preprocess_strips_mermaid_comment_at_eof_without_regex` -
  passed, `3` tests run.
- `cargo +1.95 nextest run -p merman-core detect` - passed, `20` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 're_crlf|cached_regex!\(re_crlf|Regex::new\(r"\\r\\n\?' crates/merman-core/src/preprocess/mod.rs` -
  no CRLF regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No entity encoding, HTML attribute rewrite behavior, frontmatter/directive parsing, detector order,
semantic model, rendered output, SVG baseline, root viewport formula, theme behavior, or
Architecture residual classification changed. This slice only removes an avoidable regex
construction point from public preprocessing.
