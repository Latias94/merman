# Resvg-Safe SVG Output Pipeline - Handoff

Status: Complete
Last updated: 2026-05-28

## Current State

The workstream is implemented and verified. SVG output now has an explicit `SvgPipeline` with
`Parity`, `Readable`, and `ResvgSafe` presets. Default `render_svg_sync` remains parity output,
while readable, raster, and CLI raster paths opt in to the shared pipeline.

## Current Task

- Task ID: RSO-080
- Owner: codex
- Files:
  - `crates/merman-render/src/svg`
  - `crates/merman/src/lib.rs`
- `crates/merman/src/render/raster.rs`
- `crates/merman-cli/src/main.rs`
- Goal: Close verified pipeline lane.
- Validation:
  - `cargo nextest run -p merman-render`
  - `cargo nextest run -p merman --features raster`
  - `cargo nextest run -p merman-cli`
  - `cargo fmt -p merman-render -p merman -- --check`
- `cargo clippy -p merman-render -p merman --all-targets -- -D warnings`
- Status: DONE

## Decisions

- Keep parity output as the default.
- Build consumer cleanup as an explicit pipeline.
- Expose a string/Cow custom postprocessor first; keep event-stream internals private.
- Do not copy Zed GPL implementation.
- `SvgPipeline::resvg_safe()` strips `<foreignObject>` only after fallback overlays are inserted.
- Product-specific theme/accent semantics remain host-owned custom postprocessors.

## Next Step

No required next task remains in this workstream. Follow-up candidates, if needed:

- Add more structured XML/CSS parsing internally if string postprocessing profiles as a bottleneck.
- Add host-specific examples once an integration requests theme/accent pass guidance.
