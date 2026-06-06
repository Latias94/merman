# Merman 0.7 Architecture Deepening

Status: Closed
Last updated: 2026-06-06

## Why This Lane Exists

`merman` is still pre-1.0, FFI is not published, and the pinned Mermaid baseline is already broad
enough that shallow module seams are becoming release risk. The next 0.7.0 work should deepen the
headless render operation, diagram family facts, SVG parity output, render-side theme access, and
typed semantic ownership before those shapes become compatibility promises.

This lane turns the 2026-06-06 architecture review into an execution plan. The bias is deliberate:
delete accidental complexity while the public surface can still move, but preserve Mermaid parity
with fresh evidence after each slice.

## Relevant Authority

- ADRs:
  - `docs/adr/0001-upstream-baseline.md`
  - `docs/adr/0004-public-api-and-headless-output.md`
  - `docs/adr/0006-feature-flags-tiny-vs-full.md`
  - `docs/adr/0010-semantic-model-boundary.md`
  - `docs/adr/0011-semantic-model-versioning.md`
  - `docs/adr/0014-upstream-parity-policy.md`
  - `docs/adr/0020-sanitization-and-security-level.md`
  - `docs/adr/0050-svg-viewbox-parity.md`
  - `docs/adr/0052-normalized-upstream-fixtures.md`
  - `docs/adr/0057-headless-svg-text-bbox.md`
  - `docs/adr/0059-raster-output-strategy.md`
  - `docs/adr/0062-fixture-derived-overrides.md`
  - `docs/adr/0063-extensible-svg-output-pipeline.md`
  - `docs/adr/0064-host-styling-svg-postprocessors.md`
  - `docs/adr/0065-ascii-output-boundary.md`
  - `docs/adr/0066-ffi-binding-strategy.md`
  - `docs/adr/0068-render-side-presentation-theme-view.md`
- Existing docs:
  - `CONTEXT.md`
  - `docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md`
  - `docs/alignment/STATUS.md`
  - `docs/alignment/PARITY_HARDENING_PLAN.md`
  - `docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`
  - `docs/performance/FEARLESS_REFACTORING.md`
- Architecture review input:
  - Read-only architecture review run from 2026-06-06, promoted into this workstream's problem
    statement, target state, and task ledger.

## Problem

Five release-facing seams are too shallow:

- public adapters know the full parse/layout/SVG/postprocess ordering;
- diagram family facts are split across detector registries, parser registries, render registries,
  bindings metadata, fixture admission, and xtask command lists;
- SVG root, viewport, emitted bounds, and fixture-derived override behavior still leak into
  family renderers;
- renderers still read raw config and `themeVariables` fallback chains outside the
  `PresentationTheme` module accepted by ADR 0068;
- typed semantic models coexist with legacy JSON fallback paths in ways that make renderer
  dispatch and sanitization wider than necessary.

The common symptom is low locality. A Mermaid family or output contract change often requires edits
in several modules that each expose nearly as much interface as implementation.

## Target State

When this lane closes:

- there is one canonical Headless Render Operation module that owns parse, typed render model
  construction, layout, SVG emission, postprocess metadata, and pipeline ordering;
- CLI, Rust facade, bindings-core, FFI, UniFFI, WASM, ASCII/raster-facing entry points are thin
  adapters over canonical operations or explicitly documented exceptions;
- diagram family facts and admission state are represented by deeper modules that can project
  detector/parser/render metadata, supported diagram metadata, fixture admission, and xtask command
  coverage;
- SVG root/viewport behavior is owned under the SVG parity layer, with family renderers providing
  content bounds and bounded Mermaid family deltas;
- renderer theme access prefers `PresentationTheme` roles over raw config path chains;
- family modules own typed semantic sanitization and projection decisions where field knowledge is
  required;
- legacy JSON fallback is either deleted where evidence permits or fenced as an adapter for
  compatibility/custom/error/not-yet-typed paths.

## In Scope

- Internal refactors across `merman-core`, `merman-render`, `merman`, `merman-cli`,
  `merman-bindings-core`, `merman-ffi`, `merman-uniffi`, `merman-wasm`, and `xtask`.
- Public surface slimming before 0.7.0 where FFI/bindings compatibility is not yet committed.
- Documentation updates to `CONTEXT.md`, ADRs when contract decisions change, and alignment docs
  when admission semantics change.
- Focused tests around new interfaces plus parity gates for SVG/root/theme changes.
- Deleting redundant helpers, pass-through convenience methods, duplicate fallback paths, and stale
  compatibility code after evidence shows the replacement earns its keep.

## Out Of Scope

- Full ELK layout parity.
- Broad pixel-perfect tuning that is not backed by upstream Mermaid source or fixture evidence.
- Rewriting `dugong` or `manatee` algorithm internals as part of this lane.
- Changing the compatibility JSON public output contract without an explicit ADR.
- Host/product-specific styling policy inside the default parity renderer.
- Large unrelated performance rewrites already tracked under `docs/performance/`.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| FFI and related binding surfaces can still be reshaped before 0.7.0. | High | User instruction and ADR 0066. | Public adapter slimming must become additive/deprecated instead of deleting. |
| Mermaid `11.15.0` remains the pinned baseline for this lane. | High | `CONTEXT.md`, ADR 0001, `merman_core::baseline`. | Diagram family facts and admission inventory must be regenerated from the new baseline. |
| Public JSON output remains a compatibility surface even if JSON stops being a normal renderer input. | High | ADR 0004, ADR 0010. | JSON fallback deletion would need a new ADR and migration plan. |
| Headless Render Operation must land before broad public convenience deletion. | High | Architecture review deletion test. | Deleting helpers first would push workflow ordering to callers. |
| SVG root/viewport and theme work can proceed without changing Mermaid semantics. | Medium | ADR 0050, 0057, 0062, 0068. | If a family delta contradicts an ADR, split an ADR review task before code changes. |
| Diagram family facts and admission inventory can be built incrementally. | Medium | Existing detector/parser registries and xtask lists. | Start with read-only projection checks before replacing production call sites. |

## Architecture Direction

The lane deepens existing seams rather than inventing unrelated abstractions.

The first seam is the Headless Render Operation. It should be behavior-bearing: callers cross one
interface and get parse, layout, SVG, postprocess metadata, and pipeline ordering. It is not a pass
through helper over the same five parameters. Adapters choose input/output shape, error protocol,
and host options; they do not rebuild the render flow.

Diagram family facts should represent pinned-baseline facts once and project them outward. Detection
order, aliases, tiny/full feature profile, known-type side effects, parser adapters, render-model
adapters, supported diagrams, and fixture admission are one domain. Splitting them is useful only
when the split hides implementation behind a smaller interface.

SVG root/viewport and render-side theme access remain renderer-owned. Family renderers should own
family semantics and bounded upstream deltas, but not generic root serialization, override lookup,
or repeated theme fallback chains.

Typed semantic ownership should move field knowledge toward family modules. Engine-level orchestration
should not know which families have `title`, `accTitle`, or `accDescr` fields. Flowchart receives
special attention because it is the largest parity-risk family and currently carries duplicate JSON
and typed projection paths.

JSON fallback is the final deletion target, not the first. The lane must first prove which 0.7.0
families are admitted for typed render. Once that evidence exists, JSON can stay as public output
and compatibility adapter without remaining a second main renderer path.

## Source Coverage Audit

| Source | State | Evidence path | Impact | Required action |
| --- | --- | --- | --- | --- |
| User goal and constraints | COVERED | chat request on 2026-06-06 | Allows fearless refactor and deletion before FFI publish. | Keep compatibility notes in tasks. |
| Project context | COVERED | `CONTEXT.md` | Confirms baseline and architecture boundaries. | Update with lane terms. |
| ADRs | COVERED | ADR list above | Prevents re-litigating accepted boundaries. | Add ADR only for changed public contract. |
| Prior architecture audit | COVERED | `docs/quality/ARCHITECTURE_ISSUES_2026-06-01.md` | Confirms issues are not one-off observations. | Link tasks to issue IDs where useful. |
| Improve architecture report | COVERED | temp HTML report above | Provides candidate ordering and deletion tests. | Promote findings into this workstream. |
| Validation commands | COVERED | `EVIDENCE_AND_GATES.md` | Defines fresh evidence before completion. | Tighten per task as implementation starts. |
| Code evidence | COVERED | context manifest + explored files | Confirms shallow modules and duplicate paths. | Workers read task-local code before edits. |
| New ADR for public surface deletion | DEFERRED | none yet | Required only when task changes committed public contract. | M07A-040 decides. |

## Closeout Condition

This lane can close when:

- Headless Render Operation is canonical and adopted by public adapters;
- diagram family facts and admission inventory own the release-facing diagram surface;
- SVG root/viewport and PresentationTheme migrations remove the targeted leakage;
- typed semantic sanitization is no longer owned by an Engine-level field match;
- Flowchart has one semantic source or a documented load-bearing reason to keep its split;
- JSON fallback is deleted or fenced with explicit admission evidence;
- redundant public convenience, helper, and duplicate fallback paths are deleted where justified;
- final gates in `EVIDENCE_AND_GATES.md` pass or any residual risks are explicitly split into
  follow-on workstreams.

Status: Closed on 2026-06-06 via M07A-120. Final workspace, alignment, structural SVG parity,
selected root SVG parity, override no-growth, JSON ledger, formatting, and documentation gates
passed. Full `parity-root` remains an existing root-only residual surface owned by
`docs/workstreams/mermaid-11-15-root-viewport-residuals`.
