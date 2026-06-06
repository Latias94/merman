# Merman 0.7 Architecture Deepening — Handoff

Status: Active
Last updated: 2026-06-06

## Current State

The workstream is open and M07A-C1 and M07A-C2 are complete. M07A-020 introduced a private
Headless Render Operation module and routed existing Rust facade SVG helpers through it. M07A-030
migrated CLI Mermaid input rendering to call `render_svg_with_pipeline_sync` with CLI-owned
postprocess pipeline policy. M07A-040 removed shallow uncommitted `HeadlessRenderer` convenience
methods while keeping documented readable/resvg-safe preset helpers. M07A-050 added core Diagram
Family Facts projections for detector/parser/render/metadata surfaces. M07A-060 added xtask
admission inventory projections consumed by `compare-all-svgs` and `check-alignment`. M07A-070
added shared root viewport planning and migrated `treeView` as the proof family for canonical root
`viewBox` / `width` / `height` / responsive `style` emission. M07A-075 closed the PDF output
size-budget gap so vector PDF conversion validates intrinsic SVG size limits before fit/page
wrapping and only allows oversized trusted exports through explicit unbounded options. M07A-076
split raw SVG raster/PDF input from Mermaid-generated SVG postprocessing: raw SVG now starts from a
`resvg_safe` boundary before CLI background/CSS postprocessors and is documented as trusted input.
M07A-077 exposed fixed local-time controls through the CLI and fixed the typed render-model parse
path so Gantt render output uses the same `Engine::with_fixed_today` and
`Engine::with_fixed_local_offset_minutes` context as semantic JSON parsing. M07A-078 exposed the
same fixed-time controls through Rust headless renderer facades and the shared binding
`options_json` contract, so existing C, UniFFI, WASM, Python, Android, Apple, Flutter, and Web paths
inherit the capability without ABI growth. M07A-079 aligned Flowchart `nodeSpacing=0` and
`rankSpacing=0` with Mermaid's dagre source semantics (`|| 50`) while preserving
`diagramPadding=0` through the SVG viewBox path (`?? 8`). M07A-080 migrated XyChart and
QuadrantChart visible theme role resolution into renderer-facing `PresentationTheme` roles.
M07A-090 moved typed render-model common DB sanitization out of `Engine` and into
`RenderSemanticModel` plus family-owned model methods, closing typed path gaps for Flowchart
`accTitle` / `accDescr` and Treemap `title`. M07A-100 collapsed Flowchart semantic JSON and typed
render-model parsing around one internal `FlowchartSemanticSource` while preserving FlowDB ordering
traces such as `vertexCalls`. M07A-110 fenced `RenderSemanticModel::Json` to the built-in `error`
diagram and custom render-model adapters, while registry coverage now requires every pinned
non-error built-in semantic parser to have a typed render parser. The admission alignment gate also
no longer depends on ignored local `fixtures/_deferred` investigation directories.

## Read First

- `docs/workstreams/merman-0-7-architecture-deepening/DESIGN.md`
- `docs/workstreams/merman-0-7-architecture-deepening/TODO.md`
- `docs/workstreams/merman-0-7-architecture-deepening/EVIDENCE_AND_GATES.md`
- `CONTEXT.md`
- ADR 0004, 0006, 0010, 0011, 0012, 0014, 0020, 0050, 0057, 0062, 0063, 0064, 0066,
  0068 for completed C1/C2/C3 and typed semantic ownership context

## Next Action

Next planner action is M07A-120 final verification and closeout:

- run the final gate set from `EVIDENCE_AND_GATES.md`, broadening to workspace gates only if the
  current machine budget allows it;
- reconcile TODO.md, TASKS.jsonl, CAMPAIGNS.jsonl, and this handoff before closing or splitting
  residual follow-ons.

## Known Risks

- Diagram Family Facts intentionally preserve the existing bindings supported-diagram metadata
  surface; treeView/ishikawa/eventmodeling admission is recorded in xtask inventory, not newly
  published as binding metadata.
- Admission inventory currently owns primary SVG matrix and root-deferred projections. It no longer
  requires ignored local `fixtures/_deferred` directories for `check-alignment`, but per-family
  compare command dispatch still remains explicit in `compare-all-svgs`.
- Root viewport planning is shared, but only `treeView` has migrated to the canonical plan in
  M07A-070. Other families still use their existing root emitters and should migrate in separate
  family-scoped slices.
- Raw external SVG input is still routed through the SVG postprocess/raster/PDF path and has not
  been upgraded into a general-purpose untrusted-SVG sanitizer in this lane. M07A-076 narrowed the
  CLI raster boundary and documented trusted-input semantics, but arbitrary uploaded SVG still needs
  host-side trust decisions.
- PDF size limits now cover intrinsic SVG dimensions before vector conversion; this does not address
  every possible SVG parser complexity risk.
- CLI and shared binding Gantt parse/render determinism now have fixed today/offset controls. Typed
  per-platform option builders remain a follow-on convenience, not a low-level ABI requirement.
- New typed render models that expose top-level `title`, `accTitle`, or `accDescr` must add a
  family-owned `sanitize_common_db_fields` method; `Engine` should remain orchestration-only.
- Flowchart typed/JSON convergence now has one internal semantic source, but any future Flowchart
  parser changes still need to preserve FlowDB ordering traces and SVG parity evidence.
- JSON render-model fallback is now intentionally narrow: built-in non-error families must stay on
  typed render parsers, while `error` and custom adapters may still use JSON. Future built-in family
  additions must update family facts and typed render parser coverage together.

## Working Tree Notes

At workstream open, the repo had unrelated untracked local directories:

- `.claude/`
- `.playwright-cli/`
- `.playwright/`
- `test-results/`

Do not remove, restore, or commit them as part of this lane.
