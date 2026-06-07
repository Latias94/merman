# HPD-050 - Markdown Line Assembly Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

`mermaid_markdown_to_lines(...)` maintained its current output line through a local `line_idx`
invariant. Normal control flow kept `out` and `line_idx` aligned, but two production append sites
still depended on that invariant staying panic-safe:

- `flush_word(...)` used `unwrap_or_else(|| unreachable!("line exists"))` after `out.get_mut(...)`;
- raw HTML tag token emission wrote through `out[line_idx]` directly.

This is not a user-facing semantic mismatch by itself, but it was an avoidable panic-bearing
renderer text boundary in the same release-hardening class as the recent invariant cleanup slices.

## Changes

- Added a local `line_mut(...)` helper that extends the markdown line vector before returning the
  requested line.
- Routed both word flushing and raw HTML tag emission through that helper.
- Added focused coverage for raw HTML tags after an explicit newline so the line-assembly behavior
  remains locked while the invariant guard changes.

## Verification

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render html_tags_after_newline_stay_on_current_markdown_line markdown` -
  passed, `22` tests run.
- `rg -n 'line exists|unreachable!|unwrap_or_else\(\|\| unreachable|out\[line_idx\]' crates/merman-render/src/text/markdown.rs` -
  reports only the guarded helper-internal `&mut out[line_idx]` after `resize_with(...)`.
- `rg -n 'unwrap_or_else|unreachable!|panic!|expect\(|unwrap\(' crates/merman-render/src/text/markdown.rs` -
  no matches.
- `git diff --check` - passed.

## Boundary

This is a local Markdown text-tokenization panic-surface cleanup. It does not change Markdown
delimiter semantics, raw HTML token handling, flowchart label measurement, SVG baselines, root
viewport formulas, parser behavior, sanitizer policy, or Mermaid parity residual classification.
