# HPD-050 - Sanitizer Line Break Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the preprocess regex cleanup, the next source-backed public boundary was Mermaid common text
sanitization. `sanitize_text(...)` uses `break_to_placeholder(...)` before non-loose HTML escaping
so user-authored line breaks survive as `<br/>` instead of being escaped with other tags.

The Rust port still compiled this regex lazily on first sanitize use:

```rust
Regex::new(r"(?i)<br\s*/?>").expect("valid regex")
```

Pinned Mermaid 11.15 source defines the matching rule in
`packages/mermaid/src/diagrams/common/common.ts`:

```ts
export const lineBreakRegex = /<br\s*\/?>/gi;
```

## Changes

- Removed the cached line-break regex helper from `crates/merman-core/src/sanitize.rs`.
- Replaced it with a direct scanner for Mermaid's source-shaped line break tag:
  - `<br`, case-insensitive for ASCII `b` / `r`;
  - JavaScript regex whitespace after `br`;
  - optional `/`;
  - immediate closing `>`.
- Preserved non-match behavior for malformed variants such as `<br / >`, `<brx>`, `</br>`, and
  `< br>`.
- Added helper-level source-shape coverage and public `sanitize_text(...)` coverage.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core break_to_placeholder_matches_mermaid_line_break_regex_shape sanitize_text_preserves_mermaid_line_break_tags_without_regex sanitize` -
  passed, `28` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'fn line_break_regex|line_break_regex\(\)|Regex::new\(r"\(\?i\)<br\\s\*/\?>"' crates/merman-core/src/sanitize.rs` -
  no sanitizer line-break regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No DOMPurify-like allowed tag/attribute policy, URI validation, script/data URL checks, Mermaid
entity decoding, semantic parsing, rendered output, SVG baseline, root viewport formula, theme
behavior, or Architecture residual classification changed. This slice only removes one avoidable
regex construction point from public text sanitization.
