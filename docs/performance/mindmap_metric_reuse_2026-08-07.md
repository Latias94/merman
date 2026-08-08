# Mindmap operation metric reuse disposition — 2026-08-07

## Decision

Status: **rejected-superseded**.

The historical Mindmap hypothesis in `650aee080e54bfab3fe1d3c161278caaddb30dd0`
has no admissible current-base work left to remove. Its useful idea was to avoid a plain-label
measurement and one width measurement per non-empty styled run before the HTML line planner
recomputed the widths that determine the result. U8 now takes a stricter operation-owned route:
qualified built-in HTML measurement enters the final inline planner before either discarded
premeasurement can execute, while opaque custom and host routes retain their complete observable
callback trace.

The exact current-base upper bound for applying the old candidate is therefore **zero removable
measurement requests per Mindmap label**. No production candidate, adjacent timing pair, or
candidate-only test is warranted. The historical branch remains unmerged and no Mindmap-specific
cache, prepared field, or fallback bypass is retained.

## Reference and revision boundary

The semantic reference is Mermaid 11.16.1 at
`repo-ref/mermaid@7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`.

| Role | Commit | Meaning |
|---|---|---|
| historical hypothesis | `650aee080e54bfab3fe1d3c161278caaddb30dd0` | Moved the plain-label and per-run width probes after the no-inline-icon HTML fast return. |
| historical base | `2d2e891c77a924e78702302bd22ca48bee6dff02` | Parent of the unmerged hypothesis. |
| U8 built-in owner | `9f6af1843919104defa01be1acfdcb46813c2b2e` | Added the private built-in operation carrier and entered the final inline planner before the discarded preliminary probes. |
| U8 inactive-path completion | `685dd3ce1f6d37c85e24a572063ccae7b2926633` | Avoided inactive built-in break planning after the natural width already proves the result. |
| U8 opaque completion | `da1378b70f12987e5ca5aba7e584a61f1fb556fd` | Linearized opaque planning without reducing callback count, request text, order, or failure position. |
| U8 decision | `2484eb5793b39a5b872cc6046c9b4d36f961cd3e` | Accepted the shared rich-inline planning owner and recorded its structural contract. |
| reviewed current base | `00bfebcb8d37896f3a896e26a06910a08d41bbe4` | Current `perf/headless-performance-hardening` revision for this disposition. |

`650aee080` is not an ancestor of the reviewed current base. Its only containing local branch is
the isolated `perf/mindmap-inline-metrics` experiment branch.

## Source-backed semantic boundary

Mermaid 11.16.1 routes Mindmap Markdown labels through `labelHelper(...)` and `createText(...)`.
For HTML labels, `addHtmlSpan(...)` first obtains the `display: table-cell; white-space: nowrap`
client rectangle. When that width equals the configured limit, it switches the same label to
`display: table; white-space: break-spaces` with a fixed width and measures again. `labelHelper`
then uses that resulting rectangle as the node's label dimensions.

Merman preserves that semantic sequence while separating browser measurement from headless layout:

1. `mindmap_label_bbox_px(...)` converts the Markdown label to the same HTML projection and makes
   one complete `HtmlLike` measurement request per non-math node.
2. `measure_html_with_inline_styles(...)` grants a private carrier only when the routed wrap
   operation resolves directly to a concrete built-in profile. Host routes stay opaque even when
   their fallback profile is built-in.
3. For a qualified built-in label without inline FontAwesome content, the branch at
   `crates/merman-render/src/text/metrics.rs:2519` returns through
   `finish_inline_html_layout(...)` before the plain-label request at line 2549 and the per-run
   width pass at line 2564 are reachable.
4. For an opaque custom or host measurer, those requests intentionally remain before the final
   planner. Their count, text, style, order, stateful return values, failure position, and report
   provenance are observable Merman contracts; removing them would not be a semantics-preserving
   optimization.
5. Mindmap layout stores the measured result in `LayoutNode.width`, `height`, `label_width`, and
   `label_height`. The Mindmap SVG emitter reads those fields and emits XHTML without owning or
   invoking a `TextMeasurer`, so there is no layout-to-SVG duplicate measurement for this
   hypothesis to carry.

This is semantic and structural convergence with the pinned source, not a claim that headless font
metrics reproduce browser `getBoundingClientRect()` floating-point values exactly.

## Exact upper-bound check

Let `L` be the number of non-empty styled runs in one eligible HTML label.

On the historical base, the preliminary work targeted by `650aee080` was:

```text
discarded_requests_old = 1 plain-label request + L per-run requests
```

The final inline planner then performed the requests that actually determine natural width,
min-content width, wrapping, and line count. The old hypothesis moved the preliminary `1 + L`
term after the early return.

On the reviewed current base:

```text
qualified_builtin_discarded_requests = 0
mindmap_svg_remeasurement_requests = 0
admissible_opaque_request_reduction = 0
incremental_requests_removed_by_650aee080 = 0
```

The first zero follows directly from the carrier-qualified return preceding both preliminary
passes. The second follows from the measured dimensions being carried in the existing layout
artifact and consumed by SVG emission. The third is a contract boundary: opaque requests still execute,
but none is legally removable by this candidate. Consequently the safe current-base upper bound
is zero for every label count, repetition rate, and fixture size.

Math labels and labels containing inline icons are outside the old candidate's early-return
condition. Their remaining work is not evidence of an incomplete `650aee080` port and would require
a separately registered, source-backed hypothesis.

## Evidence disposition

U9 admits Requirement and Mindmap production work only when the family-local upper-bound check
qualifies. This one does not. Constructing an adjacent A/B pair would have only two possible
outcomes:

- an executable-equivalent duplicate of the accepted U8 built-in route; or
- a callback-reducing opaque route that fails the host/custom-measurer semantic gate before
  measurement.

No timing or memory claim is made, and no ignored experiment ledger was created. The decision is a
pre-admission upper-bound rejection, not an inconclusive benchmark result.

Read-only verification performed for this receipt:

- confirmed Mermaid package version `11.16.1` and reference commit
  `7ecca0cd7f1658ef74f4e7e91f925724ef403bbf`;
- inspected the full `650aee080` production and test diff against its parent;
- confirmed `650aee080` is not an ancestor of the reviewed branch;
- traced carrier qualification, built-in early return, opaque callback preservation, Mindmap
  layout dimension storage, and SVG dimension consumption; and
- searched the Mindmap SVG owner for `TextMeasurer` or measurement calls and found none.

No Cargo command was run because this disposition changes no production or test code. The shared
U8 semantic and structural verification remains recorded in
`docs/performance/rich_inline_html_planning_2026-08-07.md`.

## Residual boundary

The current built-in planner still performs the natural-width, min-content, wrap-boundary, and
line-count work required to model Mermaid's HTML label layout. Those are result-producing probes,
not discarded premeasurements. Browser font shaping, `foreignObject`, and client-rectangle floats
remain bounded parity residuals. Any future Mindmap text candidate must identify a new owner-local
term, preserve the complete built-in/host distinction, and pass a fresh upper-bound gate; this
closed historical branch is not such a candidate.
