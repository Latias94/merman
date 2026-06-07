# HPD-050 - Preprocess Style Hex Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After entity placeholder encoding stopped compiling `#\w+;` / integer regexes on public
preprocessing input, the same `encodeEntities(...)` port still used cached regex helpers for the
two upstream hash-color protection passes:

```rust
Regex::new(r"style.*:\S*#.*;").expect("preprocess regex must compile")
Regex::new(r"classDef.*:\S*#.*;").expect("preprocess regex must compile")
```

Pinned Mermaid 11.15 source applies those as line-local JavaScript regex replacements before the
entity placeholder pass.

## Changes

- Removed the cached `re_style_hex` and `re_classdef_hex` preprocess regex helpers.
- Added a line-local scanner for the upstream `style.*:\S*#.*;` and `classDef.*:\S*#.*;` source
  shapes.
- Preserved the greedy final-semicolon behavior. On a same-line span such as
  `style a fill:#fff; style b fill:#000;`, only the final semicolon is removed before entity
  placeholder encoding, so the earlier `#fff;` is still encoded.
- Preserved the upstream non-match boundary when whitespace appears between `:` and `#`.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core encode_entity_placeholders_matches_mermaid_ascii_word_shape preprocess_encodes_entities_without_entity_regex` -
  passed, `2` tests run.
- `cargo +1.95 nextest run -p merman-core detect flowchart` - passed, `117` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 're_style_hex|re_classdef_hex|cached_regex!\(re_style_hex|cached_regex!\(re_classdef_hex|Regex::new\(r"style\.\*|Regex::new\(r"classDef\.\*' crates/merman-core/src/preprocess/mod.rs` -
  no style/classDef hex-protection regex helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed.

## Boundary

No entity placeholder marker semantics, HTML attribute rewrite behavior, frontmatter/directive
parsing, detector order, semantic model, rendered output, SVG baseline, root viewport formula,
theme behavior, or Architecture residual classification changed. This slice only removes two
avoidable regex construction points from public preprocessing.
