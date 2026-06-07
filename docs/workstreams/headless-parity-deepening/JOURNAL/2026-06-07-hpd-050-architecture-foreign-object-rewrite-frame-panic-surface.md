# HPD-050 - Architecture foreignObject Rewrite Frame Panic Surface

Task: HPD-050 release-boundary panic-surface hardening

## Context

Architecture service `iconText` normalization already uses explicit frame stacks for parsing,
namespace rewriting, and serialization of XHTML/SVG-like foreignObject fragments. The rewrite loop
still had one production `expect("rewrite frame should exist")` on the final stack pop. That pop is
guarded by the preceding `stack.last_mut()` check in normal control flow, but it is still a library
panic on an SVG/HTML normalization boundary if the explicit-stack invariant ever drifts.

## Changes

- Replaced the production `expect("rewrite frame should exist")` in
  `crates/merman-render/src/svg/parity/architecture/foreign_object.rs` with a defensive
  `let Some(frame) = stack.pop() else { return Vec::new(); }` branch.
- Preserved normal namespace rewriting, HTML-child splitting from SVG-only parents, and fragment
  serialization behavior.
- Left Architecture parsing, FCoSE layout, group/service geometry, root-bounds formulas, SVG
  baselines, theme CSS, and sanitizer policy unchanged.

## Verification

- `cargo +1.95 fmt -p merman-render` - passed.
- `cargo +1.95 nextest run -p merman-render normalize_xhtml_fragment_handles_deep_nested_html_with_small_stack architecture_svg_handles_deep_icon_text_xhtml_fragment` -
  passed, `2` tests run.
- `rg -n 'rewrite frame should exist' crates/merman-render/src/svg/parity/architecture/foreign_object.rs` -
  no matches.
- `git diff --check` - passed with the existing `CONTEXT.jsonl` LF/CRLF conversion warning.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `860`
  lines parsed.

## Boundary

This is a renderer panic-surface guard in an already stack-based rewrite loop. It does not claim an
Architecture root residual fix and does not change normal `iconText` output.
