# ADR 0065: ASCII Output Boundary

Date: 2026-05-28

## Status

Accepted

## Context

`merman` is a Rust, browser-free Mermaid implementation. The existing renderer work focuses on SVG
and raster output, while downstream library users may also need terminal-friendly diagrams for
logs, documentation, CLI previews, chat systems, and environments where SVG is unavailable.

The reference repository `repo-ref/mermaid-ascii` contains a useful MIT-licensed Go implementation
of Mermaid-like ASCII and Unicode rendering. Its graph renderer includes grid placement, path
routing, junction merging, box drawing, and separate ASCII/Unicode character sets. Its sequence
renderer includes a compact participant/message layout. The repository is cloned under
`repo-ref/`, which is intentionally gitignored, so any shipped attribution, license text, and test
fixtures must live in tracked `merman` paths.

The upstream implementation also contains parser and application concerns that should not become a
second Mermaid implementation inside `merman`.

## Decision

Model ASCII output as a first-class rendering target with its own crate:

1. Add `crates/merman-ascii` for terminal/text rendering.
2. Make `merman-ascii` consume typed models from `merman-core` instead of parsing Mermaid syntax.
3. Keep ASCII layout independent from `merman-render` SVG layout. Character-cell layout is a
   separate product target, not a quantized SVG export.
4. Expose the crate through an opt-in `ascii` feature in the top-level `merman` crate after the
   renderer has a tested public API.
5. Preserve third-party attribution in tracked files before any derived code or copied fixtures ship.

The initial product target is stable, readable, deterministic text output for flowchart and
sequence diagrams. Exact byte-for-byte parity with `mermaid-ascii` is useful for algorithm port
tests, but the public product boundary is Mermaid semantic compatibility plus stable ASCII output.

## Consequences

- `merman` gains a non-SVG output surface without weakening the existing SVG parity boundary.
- ASCII snapshots become user-visible behavior and must be treated as semver-sensitive output.
- Unsupported Mermaid features need explicit degradation or structured diagnostics; silently
  misrepresenting diagram meaning is not acceptable.
- The Go reference can guide algorithm shape, but parser duplication, CLI/web code, and local
  `repo-ref` references must not enter the shipped crate.
- License and fixture provenance become part of the workstream evidence, not an afterthought.

## Non-Goals

- Do not implement a second Mermaid parser for ASCII output.
- Do not use browser, SVG, or pixel layout as the source of truth for ASCII coordinates.
- Do not make ASCII output the default `merman` rendering mode.
- Do not claim Mermaid CLI visual parity for text output.
- Do not ship copied upstream fixtures or derived source without tracked MIT license notice and
  source commit provenance.
ASCII remains an independent model-to-grid adapter rather than a projection of SVG geometry. It
receives the same operation control, runtime context, and operation-owned resource policy as the
SVG target, while keeping its own graph, routing, canvas, character-width, and output rules.
`max_grid_cells` is a structured ASCII resource quota; cancellation/deadline is a separate
operation outcome. The public seam is one model-level backend entry, not per-family source
render helpers.

## 2026-08 viewport/report addendum

The ASCII request boundary now also accepts a provider-neutral terminal-cell viewport through
`AsciiViewportPolicy`. The policy is intentionally separate from `AsciiRenderOptions`: callers own
the requested width and choose `Allow`, `Fallback`, or `Error`, while the renderer owns display-cell
measurement, complete typed fallback selection, and terminal-safe diagnostics. `AsciiOutput` is the
canonical renderer/facade result; the named string helpers are projections of its `text` field.

`Fallback` is not clipping, ellipsis, source disclosure, or a resource recovery path. It makes at
most one typed-model fallback attempt and reports the stable `primary_overflow` reason when that
attempt is selected. It returns `FallbackUnavailable` when the family cannot
preserve the required fields within the bound. Resource and cancellation errors retain their
existing precedence and never become fallback reports. `AsciiLayoutProfile::Compact` is an explicit
opt-in candidate; canonical geometry and the Issue #53 pre-layout label wrapping remain the default.

## 2026-08-25 contract-hardening addendum

Internal output seams may change during the alpha stabilization window. At this stage the merged
report vocabulary remained schema 1; the later encoding addendum below supersedes that transport
version. `AsciiOutput` owns one measured-candidate path for extent, display-cell,
grapheme, encoded-byte, and output admission observations. Fallback writers perform bounded local
checks while constructing a complete candidate, then commit its document admission once against the
render-wide resource ledger; failed candidates never become partial output.

`AsciiOutput::metadata()` is the canonical transport adapter input and `AsciiOutput::report()` is the
CLI report projection. Bindings, generated fixtures, Web/Playground metadata, and platform DTOs must
consume those fields rather than hand-copying report vocabulary. Projection and fallback capability
come from typed family capability records; output-string prefixes are not semantic classifiers.
Semantic fallback applies typed-model complexity preflight before family compatibility projection;
Flowchart's typed projection additionally uses a bounded, cancellation-aware JSON writer before
flattening, keeping intermediate materialization within the active ASCII output policy for bounded
profiles.

## 2026-08-26 layout, theme, and encoding addendum

The alpha contract now resolves one host request into narrower host, family-layout, visual-role, and
output policies. `AsciiRenderOptions` remains the public compatibility façade, but family renderers
consume resolved policy rather than interpreting a global density switch. Flowchart and Sequence
separately admit `AsciiLayoutProfile::Compact`; State, Class, ER, XYChart, and structured-text
families remain canonical-only until family-local evidence admits another profile. Flowchart Compact
uses a 24-cell ordinary-node wrap default and Sequence Compact uses three-cell participant spacing,
unless the caller explicitly overrides the corresponding family option. State does not inherit
Flowchart graph policy.

`AsciiColorMode::Ansi16` is the terminal-native styled profile. It keeps unstyled primary text at
terminal Reset and applies sparse named ANSI accents to semantic roles without detecting terminal
background polarity. Plain remains the deterministic default. Host environment signals such as TTY,
`NO_COLOR`, `TERM`, and `COLORTERM` are resolved by the CLI or another adapter before the renderer is
called.

`AsciiOutput` transport schema 2 adds explicit `encoding` identity for Plain, ANSI16, ANSI256,
TrueColor, and HTML results. Logical extent counts content rows; a final line terminator is encoded
output but not another logical row. CLI report mode is a machine-safe Plain channel and rejects an
explicit styled request. Viewport Fallback is likewise admitted only for Plain; styled fallback
requests fail capability preflight rather than returning ambiguous or partially styled structured
text. `Allow` and `Error` remain valid for every admitted primary encoding.

Family capability records are the request-preflight authority. In addition to semantic coverage and
projection, they publish admitted layout profiles, width profiles, primary encodings, and fallback
encodings. CLI, bindings-core, UniFFI, Web, Flutter, Python, Apple, and generated contract snapshots
must project those same source descriptors rather than maintaining transport-local admission tables.

## 2026-09-11 automatic layout selection addendum

Flowchart and Sequence additionally admit `AsciiLayoutProfile::Auto` with an explicit width bound.
Auto makes at most two primary attempts: Canonical followed by Compact on width overflow. It
selects the narrower valid diagram, keeping Canonical on ties, then applies the existing
Allow/Error/Fallback policy. Fixed Canonical and Compact requests retain their fixed selection.
Flowchart Compact now defaults to a three-cell horizontal rank gap, alongside its existing
24-cell node wrap; explicit family overrides win. Other family density policies are unchanged.

Primary work remains cumulative across attempts. Retained candidate text is included in storage
admission; resource/cancellation failures remain terminal. Canonical route errors are not converted
to width fallback. A route error encountered only in the optional Compact candidate can retain a
previously valid Canonical primary. The structured fallback remains Plain-only.

ASCII report and metadata schema 3 add `requested_layout_profile` and `compact_attempted`.
`layout_profile` and primary extent identify the selected primary geometry, including when the
final output is structured fallback. Compact selection is independent from fallback selection.
The options JSON schema remains version 2. Width fitting does not guarantee reduced height/area,
and it does not authorize edge-label truncation or automatic direction changes.

## Terminal layer ordering and coordinate ownership

Flowchart, State, and Swimlane retain Dugong's rank seed. Cross-axis placement uses bounded
forward/backward barycenter ordering and integer layer compaction, allowing both branches and
merges to influence alignment. State side constraints remain part of the ordering. Compound
placement continues to enforce containment and local direction before frame allocation.

For scenes without containers, terminal coordinates and node dimensions follow each node's own
text and shape demand. A coarse projection is not authoritative after this compaction. Coarse
route candidates are admitted only while node coordinates and dimensions still match that
projection; actual-contact orthogonal candidates and terminal search serve the resulting geometry.
Finite candidate paths admit their retained cells before allocation. The final scene owns the
two-dimensional grid admission, so a discarded projection or unselected exterior candidate cannot
reject a fitting diagram by charging an imaginary canvas.

Non-aligned scenes identify feedback connections before route admission. Short direct and exterior
channels are compared before terminal search. Candidate labels receive valid host positions before
scoring, and scene growth includes those positions. A growing scene can trigger bounded refinement
even without a port conflict. Optional repairs share one scene-wide expansion allowance, while a
search for the first valid path remains subject to the operation budget. Predetermined parallel
self-loops retain deferred label placement until their complete nested batch is known. Exhausting
the optional allowance cannot flush that batch. Every candidate must carry its visible edge label;
geometry that drops a condition is not admissible. These changes replace the sequential node-slot
placement mechanism. General port/channel negotiation remains separate work;
this ordering change alone does not establish the terminal presentation comparison floor.

## Reversible route contributions

If ordinary routing finishes without a valid path for a non-loop edge, scene routing may give
that edge priority over one earlier contributor. A scene shares a four-proposal allowance and
the existing optional terminal-search expansion allowance. Each proposal replays the retained
contributors into a provisional occupancy, admits the blocked edge, and replans the displaced
edge. Both routes must materialize their markers and labels before the proposal replaces the
incumbent. Routine proposal failure restores the original route order without allocating or
replaying the old scene. Cancellation and resource failures restore geometry and propagate;
consumed work is never refunded.

Replaying preserves canonical edge identities while remapping dense occupancy indices. Active
cells, exact materialized markers, label-to-host reservations, and original terminal candidate
claims are distinct contributions. A relocated marker must not create a new candidate window
when its route is replayed. Provisional occupancy retains and admits the original scene's route
capacity, including replacement and later edges.

This is a bounded one-contributor repair, not joint port/channel negotiation. It runs after
ordinary required search and the coarse fallback finish without a route; it does not recover
from a search that has already terminated the operation through cancellation or resource limits.
No-route results remain heuristic failures, not certified spatial impossibility. Changing that
stage order and improving shared-trunk presentation require separate whole-scene evidence.

## Compatible common-source channels

Flat forward branches may allocate their source contact, common channel, and private target
contacts as a group before route selection. The group retains every canonical edge identity;
target markers and labels remain independent. The first implementation admits distinct targets
with distinct cross-axis intercepts, open source markers, and compatible strokes and styles.
Parallel, reciprocal, self-loop, container, incompatible, or insufficient-space groups continue
through ordinary route planning. Complete label measurements and authored minimum main-axis
spans are checked before an allocation is offered.

Each admitted contributor owns its allocated geometry, not just a sharing identifier. A valid
joint channel takes precedence over shorter per-edge alternatives. Optional searches cannot
dismantle it, and a one-contributor reorder must re-admit the same channel or reject the proposal.
Its compatibility identity allows shared trunk cells without a congestion penalty. Unrelated
contributors may cross transversely but cannot merge collinearly into that trunk and imply new
source/target combinations. Conditions require a private label host; placing clear text beside
an already shared host is insufficient.

This channel allocation removes independent wrapping wires in ordinary compatible fan-outs. It
does not compact node outlines or cross-axis spacing, allocate shared target markers, negotiate
whole conflict components, or replace the full-field width fallback. Those are separate remaining
parts of terminal presentation planning.
