# HPD-050 - Preprocess Entity Placeholder Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After CRLF normalization stopped compiling a regex on public preprocessing input, the next narrow
preprocess regex boundary was Mermaid entity placeholder encoding:

```rust
Regex::new(r"#\w+;").expect("preprocess regex must compile")
Regex::new(r"^\+?\d+$").expect("preprocess regex must compile")
```

Pinned Mermaid 11.15 source implements this as `encodeEntities(...)` in
`packages/mermaid/src/utils.ts`, using `/#\w+;/g` and then choosing the numeric marker from the
matched inner text.

## Changes

- Removed the cached `re_entity` and `re_int` preprocess regex helpers.
- Added a direct ASCII byte scanner for Mermaid `#\w+;` placeholders.
- Preserved numeric versus nonnumeric marker output:
  - `#77653;` -> `ﬂ°°77653¶ß`;
  - `#there;` -> `ﬂ°there¶ß`.
- Preserved the source-shaped non-match boundary for non-ASCII or non-word sequences such as
  `#é;`, `#+123;`, and `#has-dash;`.
- Added helper-level source-shape coverage and public `preprocess_diagram(...)` coverage.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core encode_entity_placeholders_matches_mermaid_ascii_word_shape preprocess_encodes_entities_without_entity_regex preprocess_normalizes_crlf_without_regex` -
  passed, `3` tests run.
- `cargo +1.95 nextest run -p merman-core detect flowchart` - passed, `117` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 're_entity|re_int|cached_regex!\(re_entity|cached_regex!\(re_int|Regex::new\(r"#\\w\+;"|Regex::new\(r"\^\\\+\?\\d\+\$"' crates/merman-core/src/preprocess/mod.rs` -
  no entity or integer preprocess regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No style/classDef hex protection, HTML attribute rewrite behavior, frontmatter/directive parsing,
detector order, semantic model, rendered output, SVG baseline, root viewport formula, theme
behavior, or Architecture residual classification changed. This slice only removes two avoidable
regex construction points from public preprocessing.
