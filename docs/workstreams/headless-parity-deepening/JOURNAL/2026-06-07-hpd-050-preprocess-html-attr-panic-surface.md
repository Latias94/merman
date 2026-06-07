# HPD-050 - Preprocess HTML Attribute Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After CRLF normalization, entity placeholder encoding, and style/classDef hash-color protection
stopped compiling preprocess regexes on public input, the last cached regex helpers in
`crates/merman-core/src/preprocess/mod.rs` were the HTML cleanup pass:

```rust
Regex::new(r"<(\w+)([^>]*)>").expect("preprocess regex must compile")
Regex::new("=\"([^\"]*)\"").expect("preprocess regex must compile")
```

Pinned Mermaid 11.15 source implements this in `packages/mermaid/src/preprocess.ts` as
`cleanupText(...)`: normalize CRLF, then rewrite matched HTML tag attributes from double quotes to
single quotes because Mermaid parsers reject double quotes in that position.

## Changes

- Removed the cached `re_tag` and `re_attr_eq_double_quoted` preprocess regex helpers.
- Added a direct scanner for Mermaid's `/<(\w+)([^>]*)>/g` tag shape.
- Added a direct scanner for Mermaid's `/="([^"]*)"/g` attribute replacement inside matched tags.
- Preserved source-shaped boundaries:
  - tag names use JavaScript ASCII `\w`, not Rust regex Unicode word matching;
  - the tag ends at the first `>`;
  - empty double-quoted values become empty single-quoted values;
  - non-ASCII tag names such as `<é ...>` are not rewritten.
- Added helper-level source-shape coverage and public `preprocess_diagram(...)` coverage.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core normalize_html_tag_attributes_matches_mermaid_cleanup_shape preprocess_rewrites_html_attributes_without_regex encode_entity_placeholders_matches_mermaid_ascii_word_shape preprocess_encodes_entities_without_entity_regex preprocess_normalizes_crlf_without_regex` -
  passed, `5` tests run.
- `cargo +1.95 nextest run -p merman-core detect flowchart` - passed, `118` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'cached_regex|OnceLock|Regex|regex::|re_tag|re_attr_eq_double_quoted|Regex::new' crates/merman-core/src/preprocess/mod.rs` -
  no preprocess regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No CRLF normalization, entity placeholder marker semantics, style/classDef hex protection,
frontmatter/directive parsing, detector order, semantic model, rendered output, SVG baseline, root
viewport formula, theme behavior, or Architecture residual classification changed. This slice only
removes two avoidable regex construction points from public preprocessing.
