---
type: "Work Progress"
title: "Mermaid 12 container micro layout and missing sections"
description: "Parent sizing lifecycle and Flowchart missing-section rendering verified; FFI web and golden deliverables made explicit."
timestamp: 2026-09-20T10:12:11Z
record_id: "4a45d432eb464cf0a1bf9b4118ef2e92"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "746bfcc29"
supersedes: "37c5db64031849829fe210f69a874869"
---

# Summary

Full U1-U15 / R1-R12 goal remains active and incomplete. This continuation committed:

- `5a21dda67`: explicit FFI, Web WASM, built Playground and golden deliverables in U13/U15.
- `5a1fcf441`: parent-provider container micro layout and sizing lifecycle.
- `746bfcc29`: Flowchart straight clipped edges and midpoint labels when ELK emits no sections.

Release scope remains alpha.7 = unreleased main + Mermaid12 alignment, compared with alpha.6.
Keep package development versions at alpha.6 until final coordinated projections/changelog.
Production selected defaults/bundle remain11.17.2; staged target references are not yet admitted.
No publication or goal completion. The legacy wiki rollups remain unadopted; do not migrate them.

# Details

## Container micro layout

The EPL source crate exports `inside_top_center_micro_layout(current, minimum, label)` for the
reachable single inside-top-center title. Active dimensions are replaced by the symmetric title
grid/minimum; fixed dimensions remain fixed, with title overhang returned as margins. This is
not a general ELK option framework. Force, Stress, MrTree, Radial, Rectpacking and Layered parents
now consume Radial's retained constraints. Radial consumes fixed-title overhang margins.

Rectpacking's whitespace expansion moves child content before its second NodeMicroLayout.
The second operation restores active dimensions but retains the content translation; its early
Box branch instead clears constraints. `Rectangle.micro_layout_size` carries the restore size.
Containers without titles/minima must not be shrunk to zero or restored to pre-expansion sizes.

Actual elkjs inputs/results: `target/mermaid-12-admission/{container-micro,container-tall-label,
container-expansion}-oracle.{cjs,json}`. Committed adapter tests compare eight parent algorithms
with Radial active constraints and Stress fixed constraints, plus Rectpacking expansion. An
initial test used output array position instead of node ID and accidentally inspected an outer
sibling; it was corrected to ID lookup, without changing production code.

## Flowchart missing sections

Keep adapter `points=[]` as the truthful provider output. SVG route preparation recognizes
that state, connects semantic endpoint centers and reuses the existing shape endpoint cutter.
True two-point routes remain on the routed/rounded branch. Missing-section routes use linear
curves, skip redundant cluster clipping and historical degenerate group-route collapse, and
still receive the normal downstream marker path offset. Main-label anchor and bounds use the
same clipped endpoint midpoint. Route work accounting includes generated points.

Source: Mermaid12 `rendering-util/layout-algorithms/elk/render.ts`, missing-section branch near
1798-1832. The SVG test exercises explicit Box/Rectpacking containers, empty raw points,
rectangle border endpoints, label midpoint and linear path commands. Independent reviewer
`/root/review_missing_sections` found no blocking issue. Existing incomplete group-frame
sanitization remains an explicit follow-up; shape/group edge-case tests belong with that work.

## Checks

- Full `merman-elk-layered` + `merman-layout-elk` nextest:459 passed, run
  `09bd729e-11ad-4638-908e-65bb4bf2039b`.
- Renderer `flowchart_svg_test`, filter `test(flowchart_elk_)`, feature `layout-elk`:4 passed,
  run `e503c657-0f8c-4e9a-8e16-59e7e94261ba`.
- `cargo fmt -p merman-elk-layered -p merman-layout-elk -p merman-render --check`,
  `git diff --check`, `python scripts/verify-third-party-licenses.py`: passed.
- No broad fixture regeneration or final feature/distribution validation was attempted.

## Public surface clarification

The user specifically asked about FFI, Web WASM, Playground and goldens. U13 now names their
actual owners and requires ABI/ownership/error preservation, actual built WASM consumption,
updated web declarations/catalogs and a focused built Playground browser smoke. U15 explicitly
requires affected semantic/model/layout/SVG/theme/config/binding/editor goldens and metadata to
be refreshed by their existing generators, with authentic target provenance. Historical corpus
identities remain unchanged. Avoid multiplying test matrices or rebuilding unchanged profiles.

# Next Action

Continue source-backed renderer integration and hierarchy routing. Flowchart missing sections
now work; Class and ER equivalents remain. Root dispatch still only admits `elk`; register the
six additional Mermaid root names only when their family paths are functional. Do not register
root `elk.layered` or `elk.radial`. Cross-provider boundary edges still produce the explicit
unsupported error; implement source behavior rather than silently substituting Layered.

Preserve the full remaining scope: U2 default/theme promotion and obsolete bridge removal;
State/Requirement/Mindmap actual ELK paths; U5 group-frame/degenerate/marker/line-hop rules;
U9/U10 existing-family geometry and appearance; native Agentflow and Usecase; U13 editor,
Tree-sitter, FFI/Web WASM/Playground; U14 default features, lean fallback and EPL distribution;
U15 authentic fixture admission, required integration, size/performance and final review.
Do not mark the full goal complete based on the scoped tests above.

# Citations

- `docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md`
- Previous immutable checkpoint: record `37c5db64031849829fe210f69a874869`.
- Mermaid12 `98a0945418c76238f15df2afaddbba4272656c3b`; ELK0.9.1
  `62d5909f96fad541bc101ad52dabaece6b7eab7e`; elkjs0.9.3
  `a8304cf79fde75bc2ab1a89d28320f53f8637436`.
