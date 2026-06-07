# HPD-050 - Detector Comment Cleanup Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

After the C4 detector lazy-regex cleanup, `DetectorRegistry` still compiled a comment-stripping
regex every time a detector registry was constructed:

```rust
Regex::new(r"(?m)\s*%%.*\n").unwrap()
```

That pattern is a static literal, so the unwrap is not a user-input parse panic, but it is still a
fixed regex construction cost on the public auto-detect boundary. Mermaid 11.15 already exposes the
source shape for this cleanup in `packages/mermaid/src/diagram-api/comments.ts::cleanupComments`.

## Changes

- Added `crate::utils::cleanup_mermaid_comments(...)`, a small line scanner matching Mermaid 11.15
  comment cleanup semantics:
  - remove lines whose first non-whitespace bytes are `%%`, not `%%{`, and have a non-newline
    comment body after the marker;
  - preserve `%%{...}%%` directive/init lines until directive processing;
  - trim leading blank/comment lines;
  - remove final comment lines with no trailing newline.
- Removed `DetectorRegistry::any_comment_re` and the eager `Regex::new(...).unwrap()` from
  registry construction.
- Routed both `DetectorRegistry::detect_type(...)` and `preprocess_diagram(...)` through the
  shared helper, removing the duplicate local preprocess comment scanner.
- Added focused regressions for detector comment stripping, preprocess EOF comment stripping, and
  the shared helper's Mermaid-shaped behavior.

## Verification

- `cargo +1.95 fmt -p merman-core` - passed.
- `cargo +1.95 nextest run -p merman-core cleanup_mermaid_comments_matches_mermaid_line_comment_shape detector_registry_strips_mermaid_comment_lines_without_regex preprocess_strips_mermaid_comment_at_eof_without_regex detector_registry_strips_deep_frontmatter_with_small_stack auto_detect_common_headers_with_deep_config_small_stack` -
  passed, `5` tests run.
- `cargo +1.95 nextest run -p merman-core detect` - passed, `19` tests run.
- `cargo +1.95 fmt --check -p merman-core` - passed.
- `git diff --check` - passed.
- `rg -n 'any_comment_re|cleanup_comments\(|Regex::new\(r"\(\?m\)\\s\*%%' crates/merman-core/src -S` -
  no detector comment-regex or duplicate local cleanup helper matches.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed,
  `794` lines.

## Boundary

No detector order, supported-family profile, known-type parse side effect, semantic model, SVG
output, baseline, root viewport formula, theme behavior, or Architecture residual classification
changed. This slice only removes a detector-registry regex construction point and consolidates
Mermaid comment cleanup semantics across detection and preprocessing.
