# Architecture Issues Audit - 2026-06-01

This document records the read-only architecture audit performed on 2026-06-01.
It is an issue ledger, not an implementation plan. The goal is to preserve
actionable findings with enough source locations to reopen each topic quickly.

Audit scope:

- `merman-core`: diagram detection, parser registries, semantic model seams.
- `merman-render`: layout, SVG parity rendering, root viewport, config/theme.
- `merman-ascii`: terminal rendering modules and test surfaces.
- Public facade, CLI, FFI/UniFFI/WASM/web bindings.
- `xtask`, fixtures, upstream parity tooling, benchmark and release gates.

No files were modified during the audit phase. This file is the first materialized
artifact from the findings.

## Locator Notes

Line numbers are a snapshot of the working tree observed on 2026-06-01. Some
files were already modified by concurrent work, so line numbers should be treated
as navigation anchors rather than immutable references.

Large-file size signals observed during the audit:

| File | Current line count |
| --- | ---: |
| `crates/manatee/src/algo/fcose/mod.rs` | 4444 |
| `crates/xtask/src/cmd/import/cypress.rs` | 3049 |
| `crates/merman-render/src/flowchart/layout.rs` | 2495 |
| `crates/merman-render/src/class.rs` | 2371 |
| `crates/merman-core/src/theme.rs` | 2184 |
| `crates/merman-render/src/state/layout.rs` | 1735 |
| `crates/xtask/src/svgdom.rs` | 1347 |
| `crates/merman-render/src/svg/parity/state/emitted_bounds.rs` | 1328 |
| `crates/merman-render/src/svg/parity.rs` | 1056 |
| `crates/merman-ascii/src/relation_graph.rs` | 873 |
| `crates/merman-core/src/lib.rs` | 735 |
| `crates/merman/src/render/mod.rs` | 677 |
| `crates/merman-bindings-core/src/lib.rs` | 467 |
| `crates/merman-ascii/src/sequence/render.rs` | 400 |
| `crates/merman-core/src/diagram/mod.rs` | 302 |
| `crates/merman-ascii/src/graph/routing.rs` | 186 |

## Priority Key

- P0: Cross-cutting correctness, baseline drift, public contract drift, or release gate risk.
- P1: High-leverage refactor that reduces repeated behavior across many diagrams or adapters.
- P2: Important cleanup or seam-deepening that should follow a broader boundary decision.

## Issues

### ARCH-001 - Repository lacks a root context entry point

Priority: P1

Locators:

- Missing root `CONTEXT.md`
- Existing scattered context: `docs/adr/*`, `docs/alignment/*`, `docs/workstreams/*`

Problem:

There is no root project context document that summarizes the current domain
language, active architecture boundaries, baseline version, and where to look
first. The codebase has strong ADR/workstream coverage, but the entry point is
spread across many files. This increases onboarding cost for future agents and
humans and makes it easy to miss active lanes such as root viewport residuals.

Impact:

Architecture decisions are traceable but not locally discoverable. This weakens
the "locality" of repository knowledge and makes broad refactors more likely to
start from stale assumptions.

Suggested direction:

Create a root context entry point that points to the authoritative ADRs,
alignment status, active workstreams, and current Mermaid baseline.

Status note 2026-06-05:

`CONTEXT.md` now exists at the repository root as the current-facing entry
point. It records the active Mermaid baseline, major ownership boundaries,
non-goals such as full ELK layout support, and the first documents to inspect.
ARCH-001 is closed as a discoverability gap, though the file should stay current
as future architecture lanes land.

Related decisions:

- ADR-0001 upstream baseline
- ADR-0014 upstream parity policy

### ARCH-002 - Mermaid baseline facts are split and stale

Priority: P0

Locators:

- `tools/upstreams/REPOS.lock.json:7` pins `mermaid@11.15.0`
- `docs/adr/0001-upstream-baseline.md:14` states `mermaid@11.15.0`
- `crates/merman-core/src/detect/mod.rs:103` uses `default_mermaid_11_12_2_full`
- `crates/merman-core/src/detect/mod.rs:183` uses `default_mermaid_11_12_2`
- `crates/merman-core/src/diagram/mod.rs:25` uses `default_mermaid_11_12_2`
- `crates/merman-render/src/generated/architecture_root_overrides_11_12_2.rs:1`
- `docs/alignment/ARCHITECTURE_MINIMUM.md:5` still says Mermaid `@11.12.3`
- `docs/adr/0047-layout-golden-snapshots.md:9` still says Mermaid `@11.12.3`
- `docs/adr/0052-normalized-upstream-fixtures.md:7` still says Mermaid `@11.12.3`

Problem:

The repository currently has more than one baseline story. The lock file and
ADR-0001 say the active baseline is `11.15.0`, while many code names, generated
artifact names, generated comments, alignment docs, and older ADRs still encode
`11.12.x` or `11.12.3`. Some generated files are documented as historical names,
but that history is now visible at production call sites.

Impact:

Baseline upgrades have poor locality. A future bump can miss report titles,
generated provenance, registry names, and comparison policies. The stale naming
also makes it harder to determine whether a drift is a real upstream difference
or just stale documentation.

Deletion test:

If a single baseline module were deleted, the current complexity would return to
xtask, generated files, renderer modules, docs, and registry constructors. That
means the baseline module is a real seam worth deepening.

Suggested direction:

Create a pinned baseline registry module or manifest used by import, generation,
reports, docs projections, detector/diagram registry naming, and generated
override provenance.

Status note 2026-06-05:

This issue has been partially narrowed. `crates/merman-core/src/baseline.rs`
now records the pinned Mermaid `11.15.0` constants and the legacy generated
suffix explicitly, while detector and parser registries expose
`for_pinned_mermaid_baseline` / `pinned_mermaid_baseline_*` constructors. The
remaining work is not to invent the baseline seam from scratch, but to pay down
stale `11.12.x` wording in historical docs/comments and remove production
call-site reliance on deprecated versioned names where compatibility allows.

Related decisions:

- ADR-0001 upstream baseline
- ADR-0014 upstream parity policy
- ADR-0062 fixture-derived overrides

### ARCH-003 - Diagram detection and registry seam is too shallow

Priority: P0

Locators:

- `crates/merman-core/src/detect/mod.rs:64`
- `crates/merman-core/src/detect/mod.rs:87`
- `crates/merman-core/src/detect/mod.rs:103`
- `crates/merman-core/src/detect/mod.rs:145`
- `crates/merman-core/src/detect/mod.rs:193`
- `crates/merman-core/src/diagram/mod.rs:25`
- `crates/merman-core/src/diagram/mod.rs:85`
- `crates/merman-core/src/diagram/mod.rs:192`
- `crates/merman-core/src/lib.rs:87`

Problem:

Diagram-type facts are split across `DetectorRegistry`, `DiagramRegistry`,
`RenderDiagramRegistry`, `RenderSemanticModel`, `fast_detect_by_leading_keyword`,
and known-type parse side effects. `fast_detect_by_leading_keyword` can return a
diagram id before the registered detector order, feature availability, or
config-sensitive detection logic has had a chance to run.

Impact:

Adding or changing a diagram requires editing many places. Tiny/full feature
rules and detection order are harder to reason about because fast-path logic is
not derived from the same source as the registries.

Deletion test:

Deleting the fast path or a registry today would not remove complexity; it would
move diagram facts into other callers. That indicates the module interface is
too shallow.

Suggested direction:

Deepen the diagram-type definition module so detection, feature availability,
JSON parser adapter, typed render parser adapter, aliases, and known-type
behavior derive from one source.

Related decisions:

- ADR-0006 feature flags
- ADR-0012 tiny scope
- ADR-0014 upstream parity policy

### ARCH-004 - Parser common syntax is duplicated across diagram implementations

Priority: P1

Locators:

- `crates/merman-core/src/diagrams/class/parse.rs:10`
- `crates/merman-core/src/diagrams/class/fast.rs:10`
- `crates/merman-core/src/diagrams/sequence/parse.rs:36`
- `crates/merman-core/src/diagrams/flowchart/accessibility.rs:1`
- `crates/merman-core/src/diagrams/architecture.rs:504`
- `crates/merman-core/src/diagrams/xychart.rs:304`

Problem:

Common Mermaid syntax such as `title`, `accTitle`, `accDescr`, inline comments,
line splitting, and header handling is implemented in multiple diagram modules.
The architecture allows diagram-local implementations, but the shared token,
span, error, and common syntax behavior is not deep enough.

Impact:

Upstream syntax fixes can drift between diagrams. New diagram implementations
must rediscover common Mermaid syntax instead of reusing a clear parser seam.

Suggested direction:

Centralize Mermaid common parser terminals and accessibility/header/comment
rules. Keep fast parsers as internal adapters constrained by the same conformance
tests, not as independent behavior.

Related decisions:

- ADR-0002 parser strategy
- ADR-0022 parsing library selection

### ARCH-005 - Semantic model seam leaks into `Engine`

Priority: P0

Locators:

- `crates/merman-core/src/diagram/mod.rs:85`
- `crates/merman-core/src/lib.rs:331`
- `crates/merman-core/src/lib.rs:361`
- `crates/merman-core/src/lib.rs:404`
- `crates/merman-core/src/lib.rs:427`
- `crates/merman-core/src/lib.rs:436`
- `crates/merman-core/src/diagrams/flowchart.rs:67`
- `crates/merman-core/src/diagrams/state/parse.rs:7`

Problem:

The externally visible parse path can produce both compatibility JSON and typed
`RenderSemanticModel`. `Engine` must match over all typed variants to sanitize
models. Some diagrams also have JSON and typed paths that duplicate parsing or
model construction.

Impact:

The semantic model boundary is not local to diagram implementations. JSON and
typed projections can drift, and sanitization behavior requires broad edits when
a diagram model changes.

Suggested direction:

Each diagram should produce one semantic source and project it to compatibility
JSON and typed render models. Sanitization should live at that semantic seam.

Status note 2026-06-02:

HPD-060 landed the first bounded pilot for this direction in Sequence.
`SequenceDb::into_model(...)` now delegates through the typed
`SequenceDiagramRenderModel` and projects compatibility JSON via
`to_compat_json(...)`, removing the parser DB's second manual JSON master path.
This narrows ARCH-005 for Sequence only; it does not complete the repo-wide
semantic seam cleanup.

Related decisions:

- ADR-0004 public API and headless output
- ADR-0010 semantic model boundary
- ADR-0020 sanitization and security level

### ARCH-006 - `Engine` and `ParseOptions` carry too many pipeline stages

Priority: P1

Locators:

- `crates/merman-core/src/lib.rs:40`
- `crates/merman-core/src/lib.rs:73`
- `crates/merman-core/src/lib.rs:241`
- `crates/merman-core/src/lib.rs:326`
- `crates/merman-core/src/lib.rs:652`
- `crates/merman-core/src/lib.rs:703`
- `crates/merman-core/src/error.rs:5`

Problem:

`Engine` handles preprocessing, detection, parse dispatch, known-type parsing,
typed render parsing, timing, runtime date hooks, config merge, sanitization, and
error suppression. `ParseOptions::suppress_errors` has documented behavior that
does not fully line up with ADR language or the split between detection failure
and parse failure.

Impact:

Callers must understand implicit stage order and lenient/strict behavior. Testing
error behavior requires exercising the whole engine rather than a focused
parsing pipeline interface.

Suggested direction:

Make the parsing pipeline stages explicit in an internal module. Keep `Engine` as
a facade over a deeper pipeline implementation.

Status note 2026-06-11:

The first Parse Pipeline extraction has landed. `Engine` metadata, semantic JSON,
known-type semantic JSON, and typed render-model entrypoints now delegate to
`crates/merman-core/src/parse_pipeline.rs`, which owns preprocessing,
detection/known-type metadata projection, runtime date hooks, lenient parse
failure handling, timing diagnostics, and common DB sanitization. ARCH-006 is
narrowed but not fully closed: the public `ParseOptions::suppress_errors` name
and documented lenient/strict semantics remain unchanged for compatibility and
can be reassessed separately if the public Interface is revised before a stable
release.

Related decisions:

- ADR-0004 public API and headless output
- ADR-0007 error and diagnostics
- ADR-0009 logging

### ARCH-007 - Core tests over-focus on compatibility JSON

Priority: P1

Locators:

- `crates/merman-core/tests/snapshots.rs:60`
- `crates/merman-core/tests/snapshots.rs:202`
- `crates/merman-core/src/tests/class.rs:20`
- `crates/merman-core/src/diagrams/class/tests.rs:23`
- `crates/merman-core/src/tests/detect.rs:17`

Problem:

Many parser tests call `Engine::parse_diagram` and assert JSON paths or snapshots
after normalization. That proves compatibility JSON shape, but it does not
directly prove semantic invariants, typed projection, sanitization, or feature
dependent detector behavior.

Impact:

Refactors around semantic models or typed rendering will hit noisy snapshot
failures and still may not catch true semantic drift.

Suggested direction:

Keep public JSON parity tests, but move parser, semantic, sanitization, and
detector invariants to the modules that own those interfaces.

Related decisions:

- ADR-0010 semantic model boundary
- ADR-0011 semantic model versioning
- ADR-0014 upstream parity policy

### ARCH-008 - Typed render dispatch and JSON fallback are parallel systems

Priority: P0

Locators:

- `crates/merman-render/src/lib.rs:141`
- `crates/merman-render/src/lib.rs:150`
- `crates/merman-render/src/lib.rs:168`
- `crates/merman-render/src/lib.rs:329`
- `crates/merman-render/src/lib.rs:339`
- `crates/merman-render/src/svg/parity.rs:154`
- `crates/merman-render/src/svg/parity.rs:284`
- `crates/merman-render/src/svg/parity.rs:446`
- `crates/merman-render/src/svg/parity.rs:652`

Problem:

Typed layout dispatch and JSON layout fallback both contain full diagram-type
matching. SVG parity rendering also has several entry points: raw JSON,
`MermaidConfig`, and typed render model. Public paths still preserve JSON-first
surfaces for many diagrams.

Impact:

Changing one diagram often requires touching core registry, render enum match,
JSON fallback match, SVG raw match, SVG typed match, and sometimes ASCII. This
is high duplication at a major architectural seam.

Suggested direction:

Deepen diagram-family render modules so each family owns parse projection,
layout, SVG adapter, and compatibility JSON fallback behavior. The JSON fallback
should be an adapter, not a parallel master path.

Status note 2026-06-02:

The HPD-060 Sequence pilot applies this adapter rule on the core projection side:
compatibility JSON is now projected from `SequenceDiagramRenderModel` rather
than rebuilt from `SequenceDb`. Render dispatch is still broader than desired,
so ARCH-008 remains open for renderer-side family ownership and JSON fallback
consolidation.

Related decisions:

- ADR-0004 public API and headless output
- ADR-0010 semantic model boundary

### ARCH-009 - `SvgRenderOptions` and SVG render entry points are too broad

Priority: P1

Locators:

- `crates/merman-render/src/svg/parity.rs:79`
- `crates/merman-render/src/svg/parity.rs:108`
- `crates/merman-render/src/svg/parity.rs:129`
- `crates/merman-render/src/svg/parity.rs:265`
- `crates/merman-render/src/svg/parity.rs:427`

Problem:

`SvgRenderOptions` mixes parity output switches, debug switches, math rendering,
time overrides, root override behavior, and diagram id metadata. The render
entry points then pass this wide interface through most diagram-specific render
modules.

Impact:

Render modules learn about options they do not own. Public and internal option
surfaces are harder to evolve without accidental behavioral changes.

Suggested direction:

Keep public options stable, but prepare narrower per-stage or per-diagram render
settings before entering each diagram renderer.

Related decisions:

- ADR-0063 extensible SVG output pipeline
- ADR-0064 host styling SVG postprocessors

### ARCH-010 - Root viewport rules are scattered across renderers and tooling

Priority: P0

Locators:

- `crates/merman-render/src/svg/parity/root_svg.rs:26`
- `crates/merman-render/src/svg/parity/root_svg.rs:68`
- `crates/merman-render/src/svg/parity/util.rs:361`
- `crates/merman-render/src/svg/parity/flowchart/document.rs:75`
- `crates/merman-render/src/svg/parity/state/render.rs:371`
- `crates/merman-render/src/svg/parity/state/render.rs:645`
- `crates/merman-render/src/svg/parity/architecture/viewport.rs:42`
- `crates/xtask/src/cmd/compare/all.rs:286`
- `.github/workflows/ci.yml:59`
- `docs/alignment/STATUS.md:130`

Problem:

Root SVG writing exists, but the deeper rules are scattered: viewBox, max-width,
f32 lattice, padding, `useMaxWidth`, root override lookup, accepted residuals,
disabled-root audit, no-growth budget, and CI gating. The active residual lane
also shows remaining root-only drift.

Impact:

Root viewport fixes touch many modules and reports. It is difficult to tell
whether a residual is a renderer bug, a measurement approximation, a generated
override, or an accepted policy exception.

Suggested direction:

Create a root viewport parity module that owns computed bounds, text/bbox union
inputs, fixture-derived overrides, accepted residual policy, diagnostic disable,
report projections, and renderer lookup adapters.

Related decisions:

- ADR-0050 release quality gates / SVG viewBox parity
- ADR-0057 headless SVG text bbox
- ADR-0062 fixture-derived overrides

### ARCH-011 - Headless SVG emitted-bounds adapter is hidden under State

Priority: P1

Locators:

- `crates/merman-render/src/svg/parity/state/emitted_bounds.rs:45`
- `crates/merman-render/src/svg/parity/state/emitted_bounds.rs:49`
- `crates/merman-render/src/svg/parity/state/render.rs:577`
- `crates/merman-render/src/svg/parity/architecture/viewport.rs:42`
- `crates/merman-render/src/svg/parity/gitgraph.rs:779`

Problem:

`svg_emitted_bounds_from_svg` is a cross-diagram headless DOM bbox approximation,
but it physically lives under the State renderer. Architecture and GitGraph also
depend on it.

Impact:

The module location communicates the wrong ownership. Future bbox fixes may be
made in State without realizing they affect other diagrams and root viewport
parity.

Suggested direction:

Move emitted SVG geometry bounds into an independent parity module. Keep the
ADR-0057 split between geometry layer and opt-in text layer.

Status note 2026-06-05:

This ownership move has landed. The emitted-bounds implementation now lives at
`crates/merman-render/src/svg/parity/emitted_bounds.rs`, and State,
Architecture, and GitGraph consume it through the SVG parity layer rather than a
State-owned module. ARCH-011 is closed as an ownership-location issue; future
work belongs under ARCH-010/ARCH-017 if the root viewport policy itself needs to
be deepened.

Related decisions:

- ADR-0057 headless SVG text bbox

### ARCH-012 - SVG string building and postprocessor scanning are duplicated

Priority: P1

Locators:

- `crates/merman-render/src/svg/parity/util.rs:434`
- `crates/merman-render/src/svg/parity/util.rs:515`
- `crates/merman-render/src/svg/parity/er.rs:925`
- `crates/merman-render/src/svg/parity/er.rs:1093`
- `crates/merman-render/src/svg/parity/er.rs:1526`
- `crates/merman-render/src/svg/fallback.rs:24`
- `crates/merman-render/src/svg/fallback.rs:359`
- `crates/merman-render/src/svg/pipeline/builtin/util.rs:18`
- `crates/merman-render/src/svg/pipeline/builtin/scoped_css.rs:61`

Problem:

Renderers and postprocessors both hand-roll XML/SVG string scanning, attribute
escaping, text escaping, root tag cursoring, style insertion, and lightweight
attribute parsing.

Impact:

Safety and parity rules can drift between renderer output and postprocessor
rewrites. Fixes to escaping or root insertion may need to be duplicated.

Suggested direction:

Create an internal SVG fragment/string substrate for tag writing, attribute
escaping, root tag cursoring, style insertion, and lightweight attribute reads.
Keep the public postprocessor interface as `String`/`Cow`.

Related decisions:

- ADR-0043 headless rendering
- ADR-0063 extensible SVG output pipeline
- ADR-0064 host styling SVG postprocessors

### ARCH-013 - Effective config and theme access is scattered

Priority: P1

Locators:

- `crates/merman-core/src/theme.rs:1`
- `crates/merman-render/src/config.rs:22`
- `crates/merman-render/src/config.rs:34`
- `crates/merman-render/src/architecture.rs:35`
- `crates/merman-render/src/class.rs:21`
- `crates/merman-render/src/class.rs:735`
- `crates/merman-render/src/flowchart/layout.rs:26`
- `crates/merman-render/src/flowchart/layout.rs:711`
- `crates/merman-render/src/state/config.rs:14`
- `crates/merman-render/src/svg/parity/util.rs:9`
- `crates/merman-render/src/svg/parity/flowchart/render_config.rs:33`
- `crates/merman-render/src/svg/parity/class/settings.rs:21`

Problem:

`effective_config` is passed as raw JSON through layout, render, labels, styles,
and SVG emission. Many modules define local `config_string`, `config_bool`, or
config fallback logic even though `merman-render/src/config.rs` already contains
some shared helpers.

Impact:

Mermaid config precedence and default behavior can drift between diagrams.
Theme-variable changes become broad renderer edits instead of focused config
module changes.

Suggested direction:

Create a deeper effective config/theme view module. Diagram renderers should
consume prepared settings in diagram terms rather than raw JSON paths.

Status note 2026-06-05:

The recent frontmatter/config and `look` pass narrowed this issue but did not
close it. Top-level frontmatter diagram namespaces are now mapped into
`config.<diagram>` where supported, and Flowchart, Class, ER, State, Mindmap,
Requirement, and Kanban section SVG paths now have regression coverage for
`look=neo` DOM/style consumption. Sequence is intentionally documented as a
CSS/theme consumer rather than a diagram-wide `data-look` DOM contract. The
first follow-up centralized render-side `look` interpretation in
`DiagramLook` / `config_diagram_look`, replacing repeated raw JSON default/trim
logic across SVG parity renderers. A second follow-up centralized the common
`themeVariables.fontSize` then root `fontSize` pixel fallback in
`config_theme_or_root_font_size_px`, plus moved Block/Requirement's matching
font-family fallback onto `config_font_family_or_first_array_css` without
dropping their array-first compatibility. A third follow-up separated and
centralized the SVG root font-size rule where `themeVariables.fontSize` accepts
CSS px strings while root `fontSize` stays numeric-only, via
`config_theme_font_size_css_or_root_number_px`, and reused
`config_font_family_css` for the shared info-like CSS root font-family fallback.
Timeline render text now also uses the shared theme/root px font-size fallback,
while its root-only layout probe remains local. Flowchart, Class, and Sequence
still keep diagram-specific font precedence rules. The remaining ARCH-013 work
is broader: replace scattered raw JSON config lookups with narrow per-family
presentation/config views.

Status note 2026-06-11:

Sequence now has the first renderer-side family config view under
`crates/merman-render/src/sequence/config.rs`. Sequence layout settings and SVG
parity render settings are projected from `effective_config` before the layout
and SVG renderers consume them, removing the previous local
`effective_config["sequence"]` lookups from those call sites. This narrows
ARCH-013 for Sequence only. The distinction between layout numeric-string
compatibility and SVG numeric-only settings is preserved with focused tests.

Class now follows the same direction with
`crates/merman-render/src/class/config.rs`. The Class layout path and SVG parity
settings consume a family-owned config view, which localizes the `flowchart ??
class` namespace precedence, HTML-label wrap-mode rules, and the intentional
difference between layout numeric-string compatibility and SVG render numeric
boundaries. ARCH-013 remains open for the other diagram families and shared
theme/config surfaces.

Flowchart now joins that renderer-side config-view lane under
`crates/merman-render/src/flowchart/config.rs`. Flowchart layout and SVG parity
render settings consume `FlowchartConfigView`, centralizing Dagre spacing
fallbacks, node/state padding, wrapping widths, subgraph title margins,
font-family/font-size precedence, and asymmetric node-vs-edge HTML-label rules.
The fixed 200px edge/subgraph-title wrap behavior and legacy leading-integer
font-size parsing are covered by focused tests. ARCH-013 remains open for the
remaining diagram families and broader shared theme/config surfaces.

State now also consumes a family-owned config view from
`crates/merman-render/src/state/config.rs`. State layout settings and SVG render
settings project Dagre spacing, flowchart HTML-label wrap mode, label wrapping
width, state padding, title margin, look, hand-drawn seed, text style, and
security-level flags before layout/render code uses them. ARCH-013 remains open
for the remaining diagram families and shared theme/style config surfaces.

ER now also consumes a family-owned config view from
`crates/merman-render/src/er/config.rs`. ER layout settings and SVG render
settings project Dagre spacing, entity measurement padding, min width, label
wrapping width, relationship HTML-label precedence, font, title-margin
semantics, look, max-width, and hand-drawn seed settings before layout/render
code uses them. ER theme color and gradient generation still intentionally
remain in the SVG parity layer with the broader shared theme/style config
surface.

Block now also consumes a family-owned config view from
`crates/merman-render/src/block/config.rs`. Block layout settings project
padding and text style before node sizing, grid sizing, and edge-label
measurement consume them. Block SVG theme CSS still belongs to the shared SVG
parity theme layer rather than the layout config view.

Sankey now also consumes a family-owned config view from
`crates/merman-render/src/sankey/config.rs`. Sankey layout settings and SVG
render settings project dimensions, node geometry, node alignment, value-label
visibility, max-width behavior, label prefix/suffix, link color, outlined-label
mode, node color overrides, and `$ref` fallback semantics before layout/render
code uses them.

Event Modeling now consumes a family-owned config view from
`crates/merman-render/src/eventmodeling/config.rs`. Its layout settings project
diagram padding and max-width behavior before bounds computation and SVG root
emission consume the layout.

TreeView now consumes a family-owned config view from
`crates/merman-render/src/tree_view/config.rs`. Its layout settings project row
indentation, padding, line thickness, max-width behavior, and theme-provided
label font size before tree layout and measurement consume them.

Packet now consumes a family-owned config view from
`crates/merman-render/src/packet/config.rs`. Its layout and SVG style settings
project show-bits behavior, row and bit geometry, packet row wrapping, and packet
CSS role defaults before layout and SVG emission consume them.

Venn now consumes a family-owned config view from
`crates/merman-render/src/venn/config.rs`. Its layout settings project canvas
size, padding, max-width behavior, and debug-layout switching before the Venn
layout kernel and SVG emission consume them.

Ishikawa now consumes a family-owned config view from
`crates/merman-render/src/ishikawa/config.rs`. Its layout and SVG render settings
project diagram padding, max-width behavior, numeric layout font size, and SVG
CSS font-size spelling before layout and SVG emission consume them.

Related decisions:

- ADR-0005 configuration strategy
- ADR-0019 generated default config
- ADR-0064 host styling SVG postprocessors

### ARCH-014 - Architecture/Cytoscape/FCoSE boundary is not clean

Priority: P1

Locators:

- `crates/merman-render/src/architecture.rs:375`
- `crates/merman-render/src/architecture.rs:429`
- `crates/merman-render/src/architecture.rs:1217`
- `crates/merman-render/src/architecture.rs:1348`
- `crates/merman-render/src/architecture.rs:1385`
- `crates/manatee/src/graph/mod.rs:77`
- `crates/manatee/src/algo/fcose/mod.rs:1405`
- `crates/manatee/src/algo/fcose/mod.rs:1418`
- `crates/manatee/src/algo/fcose/mod.rs:1729`
- `crates/merman-render/src/svg/parity/architecture/geometry.rs:144`

Problem:

Architecture business layout, Cytoscape `boundingBox()` approximation, label
extras, compound padding, FCoSE relocation, and root viewBox parity are
intertwined. `manatee` is at risk of absorbing Mermaid/Architecture-specific
baseline rules.

Impact:

The reusable layout engine boundary weakens. Fixing Architecture parity can
accidentally alter generic FCoSE behavior or vice versa.

Suggested direction:

Keep Mermaid/Cytoscape parity adaptation in `merman-render`, and keep `manatee`
as a reusable layout module. Use an explicit adapter layer for mapping diagram
model/settings to layout input, bounds extras, relocation calibration, and
viewport metadata.

Related decisions:

- ADR-0053 Cytoscape layout ports
- ADR-0058 Manatee compound parent metadata

### ARCH-015 - RoughJS parity adaptation is scattered

Priority: P2

Locators:

- `crates/roughr/src/core.rs:62`
- `crates/roughr/src/core.rs:98`
- `crates/roughr/src/renderer.rs:535`
- `crates/roughr/src/renderer.rs:1141`
- `crates/merman-render/src/svg/parity/roughjs_common.rs:31`
- `crates/merman-render/src/svg/parity/roughjs_common.rs:127`
- `crates/merman-render/src/svg/parity/flowchart/render/node/roughjs.rs:111`
- `crates/merman-render/src/svg/parity/state/roughjs.rs:78`
- `crates/merman-render/src/svg/parity/class/rough.rs:16`

Problem:

`roughr` is a reusable generator module, but Mermaid/RoughJS parity rules such as
seed behavior, `roughness=0` RNG consumption, path formatting, bounds, cache, and
small numeric calibration are spread across renderers and common helpers.

Impact:

Flowchart, State, and Class can drift in subtle rough path or bounds behavior.

Suggested direction:

Create a dedicated "roughr output to Mermaid parity SVG path/bounds" adapter in
`merman-render`. Do not move Mermaid DOM output rules into `roughr`.

### ARCH-016 - Large modules are acting as complexity sinks

Priority: P1

Locators:

- `crates/manatee/src/algo/fcose/mod.rs` - 4444 lines
- `crates/xtask/src/cmd/import/cypress.rs` - 3049 lines
- `crates/merman-render/src/flowchart/layout.rs` - 2495 lines
- `crates/merman-render/src/class.rs` - 2371 lines
- `crates/merman-core/src/theme.rs` - 2184 lines
- `crates/merman-render/src/state/layout.rs` - 1735 lines
- `crates/xtask/src/svgdom.rs` - 1347 lines

Problem:

Several files are large enough that unrelated responsibilities are difficult to
separate by inspection. Some are generated or algorithmic, but others combine
config parsing, layout, parity calibration, and family-specific behavior.

Impact:

Future refactors and parity fixes have high conflict risk and weak locality.

Suggested direction:

Use the more specific issues in this document to split by domain responsibility,
not by arbitrary file size alone.

### ARCH-017 - Root/text override footprint and policy are not local

Priority: P0

Locators:

- `crates/merman-render/src/generated/*_root_overrides_11_12_2.rs`
- `crates/merman-render/src/generated/*_text_overrides_11_12_2.rs`
- `crates/merman-render/src/svg/parity/util.rs:361`
- `docs/alignment/ROOT_VIEWPORT_OVERRIDES.md:122`
- `docs/alignment/STATUS.md:130`
- `crates/xtask/src/cmd/compare/all.rs:286`

Problem:

ADR-0062 allows fixture-derived overrides, but the current inventory, generated
lookup functions, accepted residual policy, disabled-root audit, and no-growth
gate are not owned by one module. Production renderers know about generated
lookups directly.

Impact:

It is hard to tell whether an override is current, stale, diagnostic, accepted,
or release-blocking. Override paydown and growth control become process-heavy.

Suggested direction:

Fold root/text override inventory and residual policy into the root viewport
parity module proposed in ARCH-010.

Related decisions:

- ADR-0062 fixture-derived overrides

### ARCH-018 - Fixture parity inventory is fragmented

Priority: P0

Locators:

- `crates/merman-core/tests/snapshots.rs:183`
- `crates/merman-render/tests/layout_snapshots_test.rs:174`
- `crates/xtask/src/cmd/snapshots.rs:154`
- `crates/xtask/src/cmd/upstream_svg_policy.rs:10`
- `docs/alignment/ARCHITECTURE_UPSTREAM_TEST_COVERAGE.md:26`

Problem:

Fixture status is determined by filename conventions, manually maintained
coverage docs, import defer rules, SVG skip policy, and whether semantic/layout
or SVG goldens exist. Semantic goldens can cover a broad corpus with
`suppress_errors`, while layout goldens often run only where a golden exists.

Impact:

The test surface is both too broad and too narrow. New upstream fixtures require
multiple manual updates and may silently lack layout/SVG coverage.

Suggested direction:

Create a fixture parity inventory module with raw/normalized/parser-only status,
semantic/layout/SVG baseline state, defer/skip reason, provenance, and report
projection.

Related decisions:

- ADR-0052 normalized upstream fixtures

### ARCH-019 - SVG DOM signature policy is too implicit

Priority: P1

Locators:

- `crates/xtask/src/svgdom.rs:15`
- `crates/xtask/src/svgdom.rs:142`
- `crates/xtask/src/svgdom.rs:200`
- `crates/xtask/src/svgdom.rs:222`
- `crates/xtask/src/svgdom.rs:425`
- `crates/xtask/src/svgdom.rs:548`
- `crates/xtask/src/svgdom.rs:643`
- `crates/xtask/src/svgdom.rs:703`
- `crates/xtask/src/svgdom.rs:1057`

Problem:

`dom_signature(svg, mode, decimals)` looks like a simple comparison API, but its
implementation contains many policies: numeric normalization, root viewBox/style
snapping, path/points masking, identifier masking, class normalization,
diagram-specific DOM tolerance, and root handling differences.

Impact:

Compare results are difficult to interpret. A pass may mean true parity, or it
may mean a policy masked a known surface.

Suggested direction:

Make DOM mode and diagram-specific normalization policies explicit and include
the applied mask/normalize surfaces in compare reports.

Related decisions:

- ADR-0050 SVG viewBox parity

### ARCH-020 - `xtask compare-*` commands duplicate the same harness

Priority: P1

Locators:

- `crates/xtask/src/cmd/compare/diagrams/architecture.rs:66`
- `crates/xtask/src/cmd/compare/diagrams/class.rs:47`
- `crates/xtask/src/cmd/compare/diagrams/flowchart.rs:117`
- `crates/xtask/src/cmd/compare/diagrams/sequence.rs:89`
- `crates/xtask/src/cmd/compare/diagrams/state.rs:86`
- `crates/xtask/src/cmd/compare/diagrams/pie.rs:66`
- `crates/xtask/src/cmd/compare/all.rs:149`

Problem:

There are many per-diagram compare commands that repeat fixture scanning,
upstream/local SVG loading, parse/layout/render, DOM comparison, report writing,
and root reporting. Special cases are embedded in each command.

Impact:

Adding a diagram or changing comparison behavior has poor locality and high copy
paste risk.

Suggested direction:

Build one compare harness with per-diagram adapters for fixture policy, render
target, skips, root reporting, and extra diagnostics.

Related decisions:

- ADR-0014 upstream parity policy

### ARCH-021 - Benchmark scenario coverage is not explicit

Priority: P2

Locators:

- `crates/merman/benches/pipeline.rs:6`
- `crates/merman/benches/pipeline.rs:210`
- `crates/merman/tests/pipeline_bench_fixtures.rs:28`
- `docs/performance/BENCHMARKING.md:113`
- `docs/adr/0060-benchmarking-strategy.md:22`

Problem:

Benchmark fixture sets are hardcoded in bench/test files. Criterion paths can
emit skip messages. Stage spotchecks cover only a small default set.

Impact:

The benchmark surface does not clearly map to the parity corpus, and failures can
be underreported as skips.

Suggested direction:

Create a benchmark scenario inventory that records fixture, stage coverage,
must-run policy, and relation to parity corpus. Gate only scenario availability
and no silent skips in CI, not machine-sensitive performance thresholds.

Related decisions:

- ADR-0060 benchmarking strategy

### ARCH-022 - Dagre reference layout adapter is duplicated

Priority: P2

Locators:

- `tools/dagre-harness/run.mjs:60`
- `tools/dagre-harness/run.mjs:82`
- `crates/xtask/src/cmd/debug/dagre.rs:271`
- `crates/xtask/src/cmd/debug/dagre.rs:437`
- `crates/xtask/src/cmd/debug/dagre.rs:477`

Problem:

The JS harness and Rust debug command both handle reference layout invocation
and compound-edge normalization. The tool is useful, but it is shaped as a State
debug helper rather than a reusable upstream layout baseline adapter.

Impact:

Dagre-backed diagrams cannot easily share the same reference layout machinery.

Suggested direction:

Create a reusable Dagre reference adapter for input schema, endpoint
normalization, JS invocation, Rust/JS diff, and optional golden output.

HPD-050 status:

An initial adapter extraction has landed in
`crates/xtask/src/cmd/debug/dagre_reference.rs`. `compare-dagre-layout` remains a
State-only graph producer for now; non-State producers should be added only when a
source-backed Dagre residual audit needs them.

Related decisions:

- ADR-0044 Dugong parity and testing
- ADR-0045 Dugong graphlib API

### ARCH-023 - Headless render pipeline is duplicated across facade, CLI, bindings, and raster

Priority: P0

Locators:

- `crates/merman/src/render/mod.rs:186`
- `crates/merman/src/render/mod.rs:230`
- `crates/merman/src/render/mod.rs:299`
- `crates/merman/src/render/mod.rs:323`
- `crates/merman/src/render/raster.rs:45`
- `crates/merman-cli/src/main.rs:535`
- `crates/merman-cli/src/main.rs:584`
- `crates/merman-bindings-core/src/lib.rs:130`

Problem:

The operation pipeline from parse to layout to SVG to postprocess to bytes is
assembled in several places. `render_svg_sync` and `render_svg_with_pipeline_sync`
are very similar, and raster callers assemble SVG plus raster-safe processing in
multiple paths.

Impact:

Changing metadata, postprocessor order, resvg-safe behavior, no-diagram handling,
or raster defaults can drift across adapters.

Suggested direction:

Create a headless operation pipeline module. CLI, FFI, UniFFI, WASM, and facade
APIs should be adapters that choose input/output shape rather than rebuilding the
pipeline.

Related decisions:

- ADR-0063 extensible SVG output pipeline
- ADR-0064 host styling SVG postprocessors

### ARCH-024 - Options JSON contract has no single source

Priority: P0

Locators:

- `docs/bindings/OPTIONS_JSON.md:15`
- `crates/merman-bindings-core/src/lib.rs:84`
- `crates/merman-bindings-core/src/lib.rs:93`
- `crates/merman-bindings-core/src/lib.rs:98`
- `crates/merman-bindings-core/src/lib.rs:106`
- `crates/merman-bindings-core/src/lib.rs:218`
- `platforms/web/src/index.ts:1`
- `platforms/web/src/index.ts:17`
- `crates/merman-cli/src/main.rs:449`
- `crates/merman-cli/src/main.rs:461`

Problem:

The options contract is maintained in Markdown docs, private Rust serde structs,
TypeScript interfaces, and CLI flag mapping code. Defaults, enum names,
validation messages, and feature-gated behavior can drift.

Impact:

Adding one option requires multiple manual updates. Deleting or changing docs or
TypeScript types does not necessarily fail Rust tests.

Suggested direction:

Create a binding options contract module or generated contract source used by
docs, Rust parsing, TypeScript wrappers, and adapter tests.

Related decisions:

- ADR-0066 FFI binding strategy

### ARCH-025 - Binding diagnostics are inconsistent across adapters

Priority: P0

Locators:

- `crates/merman-bindings-core/src/lib.rs:15`
- `crates/merman-bindings-core/src/lib.rs:40`
- `crates/merman-bindings-core/src/lib.rs:53`
- `crates/merman-bindings-core/src/lib.rs:200`
- `crates/merman-wasm/src/lib.rs:141`
- `crates/merman-wasm/src/lib.rs:148`
- `crates/merman-ffi/src/lib.rs:222`
- `crates/merman-ffi/src/android_jni.rs:135`
- `platforms/flutter/lib/src/merman_ffi.dart:274`
- `platforms/apple/Sources/Merman/MermanEngine.swift:120`

Problem:

`merman-bindings-core` provides status and message, but richer diagnostics are
not a stable cross-adapter interface. WASM render errors become `"CODE: message"`
strings while validation has a structured result. JNI and platform wrappers wrap
payloads differently.

Impact:

Clients see different error shapes for the same underlying failure. Tests must
assert adapter-specific strings instead of one contract.

Suggested direction:

Define a shared diagnostic payload contract with status code, code name, message,
optional span/line/column where available, and fallback rules. Platform adapters
should only translate that payload to native exceptions/results.

Related decisions:

- ADR-0007 error and diagnostics
- ADR-0066 FFI binding strategy

### ARCH-026 - Async public entry points are currently a shallow runtime abstraction

Priority: P2

Locators:

- `crates/merman/src/render/mod.rs:299`
- `crates/merman/src/render/mod.rs:323`
- `crates/merman/src/ascii.rs:40`

Problem:

The async entry points wrap CPU-bound sync implementations and do not currently
perform I/O, yield, or express an asynchronous resource seam.

Impact:

Callers learn two APIs but do not get runtime leverage. If real async resource
loading appears later, the current shape may constrain the wrong abstraction.

Suggested direction:

Treat sync as the canonical headless interface for now. Keep async as a
compatibility adapter or reopen the async API decision when a real async seam
exists.

Related decisions:

- ADR-0008 async and runtime

### ARCH-027 - Binding tests are adapter smoke tests, not a contract matrix

Priority: P1

Locators:

- `crates/merman-bindings-core/src/lib.rs:482`
- `crates/merman-bindings-core/src/lib.rs:490`
- `crates/merman-bindings-core/src/lib.rs:498`
- `crates/merman-ffi/src/lib.rs:394`
- `crates/merman-uniffi/tests/bindgen_smoke.rs:98`
- `platforms/flutter/example/smoke.dart:7`
- `platforms/android/examples/MermanSmoke.kt:8`
- `platforms/web/package.json:25`

Problem:

Tests cover happy paths and a few error conditions, but they do not form a
single matrix proving that C, UniFFI, WASM, Flutter, Apple, Android, and web
adapters all implement the same contract.

Impact:

Adapter drift can remain invisible until a platform-specific user sees it.

Suggested direction:

Create shared binding contract fixtures: source, options JSON, expected status,
payload shape, and output invariants. Each adapter should prove conformance to
the same fixture set.

Related decisions:

- ADR-0066 FFI binding strategy

### ARCH-028 - ASCII graph routing seam is still shallow

Priority: P1

Locators:

- `crates/merman-ascii/src/graph/draw.rs:57`
- `crates/merman-ascii/src/graph/draw.rs:103`
- `crates/merman-ascii/src/graph/draw.rs:136`
- `crates/merman-ascii/src/graph/routing.rs:59`
- `crates/merman-ascii/src/graph/routing.rs:123`
- `crates/merman-ascii/src/graph/routing.rs:127`
- `crates/merman-ascii/src/graph/routing/cell.rs:9`

Problem:

ASCII graph routing has a seam, but plan, paint, label placement, style delta,
and output transform logic remain split between draw and routing modules. Some
style behavior is inferred by canvas state rather than carried by the route plan.

Impact:

Routing, labels, colors, junctions, and direction transforms are hard to test
independently. Broader graph route parity work will likely touch multiple modules.

Suggested direction:

Deepen the routing module so planned output carries route intent, label anchors,
edge roles/colors, junction semantics, and transform-aware character meaning.

Related decisions:

- ADR-0065 ASCII output boundary
- ADR-0067 ASCII color role API

### ARCH-029 - ASCII `RelationGraph` does not yet own enough Class/ER relation behavior

Priority: P1

Locators:

- `crates/merman-ascii/src/relation_graph.rs:20`
- `crates/merman-ascii/src/relation_graph.rs:435`
- `crates/merman-ascii/src/relation_graph.rs:483`
- `crates/merman-ascii/src/relation_graph.rs:503`
- `crates/merman-ascii/src/class/render.rs:564`
- `crates/merman-ascii/src/class/render.rs:613`
- `crates/merman-ascii/src/class/render.rs:625`
- `crates/merman-ascii/src/class/render.rs:690`
- `crates/merman-ascii/src/er/render.rs:408`
- `crates/merman-ascii/src/er/render.rs:459`
- `crates/merman-ascii/src/er/render.rs:479`
- `crates/merman-ascii/src/er/render.rs:546`

Problem:

`relation_graph` is a useful seam, but it still exposes low-level helpers such
as lane offsets, relation character writes, and centered text writes. Class and
ER duplicate layered relation drawing flows.

Impact:

Dense, cyclic, spanning, and parallel relation improvements will likely require
editing both Class and ER renderers.

Suggested direction:

Move terminal relation route planning and painting deeper into `relation_graph`.
Keep Class/ER adapters responsible only for Mermaid semantics such as markers,
cardinality, labels, line kinds, and diagnostics.

Related decisions:

- ADR-0065 ASCII output boundary

### ARCH-030 - ASCII styled cells and color roles leak through multiple layers

Priority: P2

Locators:

- `crates/merman-ascii/src/canvas.rs:16`
- `crates/merman-ascii/src/canvas.rs:49`
- `crates/merman-ascii/src/canvas.rs:53`
- `crates/merman-ascii/src/canvas.rs:137`
- `crates/merman-ascii/src/text.rs:6`
- `crates/merman-ascii/src/text.rs:32`
- `crates/merman-ascii/src/text.rs:67`
- `crates/merman-ascii/src/graph/routing.rs:123`
- `crates/merman-ascii/src/graph/routing.rs:127`
- `crates/merman-ascii/src/relation_graph.rs:157`

Problem:

Renderers and intermediate modules still know about direct colors versus role
colors, and plain/color output branches appear in several places. The role-aware
canvas exists, but the finalization boundary is not deep enough.

Impact:

Future support for CJK/emoji width, background/fill, or additional output modes
will need broad edits.

Suggested direction:

Deepen styled terminal cell finalization so family renderers produce semantic
styled cells and output adapters decide plain, ANSI, HTML, true color, and future
background behavior.

Related decisions:

- ADR-0067 ASCII color role API

### ARCH-031 - ASCII Sequence `EventPlan` is more state bag than row plan

Priority: P1

Locators:

- `crates/merman-ascii/src/sequence/plan.rs:8`
- `crates/merman-ascii/src/sequence/plan.rs:13`
- `crates/merman-ascii/src/sequence/plan.rs:46`
- `crates/merman-ascii/src/sequence/plan.rs:93`
- `crates/merman-ascii/src/sequence/render.rs:120`
- `crates/merman-ascii/src/sequence/render.rs:142`
- `crates/merman-ascii/src/sequence/render.rs:391`

Problem:

`SequenceEventPlan` tracks activation, visibility, and control state, but
`sequence/render.rs` still decides row insertion, create/destroy visibility,
notes, boxes, and control overlay timing.

Impact:

Nested control blocks, lifecycle visibility, activations, and participant boxes
remain high-risk changes.

Suggested direction:

Deepen the sequence plan into a terminal row-intent plan before painting. The
renderer should paint rows rather than decide event lifecycle semantics.

Related decisions:

- ADR-0065 ASCII output boundary

### ARCH-032 - ASCII tests rely heavily on final string snapshots

Priority: P1

Locators:

- `crates/merman-ascii/tests/graph_fixture.rs:332`
- `crates/merman-ascii/tests/flowchart_model.rs:278`
- `crates/merman-ascii/tests/class_model.rs:197`
- `crates/merman-ascii/tests/er_model.rs:149`
- `crates/merman-ascii/tests/sequence_model.rs:353`
- `crates/merman-ascii/src/graph/routing/plan/tests.rs:22`

Problem:

Public ASCII snapshots are important, but many tests compress semantic adapter,
layout model, routing, painting, color, and output encoding into final string
assertions. Some routing plan tests exist, but coverage is still uneven across
relations, sequence planning, and styled-cell finalization.

Impact:

Failures are harder to localize, and internal refactors can be blocked by broad
string diffs even when the user-visible output should remain unchanged.

Suggested direction:

Keep public snapshots as compatibility gates, and add more internal seam tests
for graph adapters, layout/routing plans, relation plans, sequence row plans, and
styled-cell finalization.

Related decisions:

- ADR-0065 ASCII output boundary
- ADR-0067 ASCII color role API

### ARCH-033 - ASCII-specific directive preprocessing sits outside core parsing

Priority: P2

Locators:

- `crates/merman/src/ascii.rs:136`
- `crates/merman/src/ascii.rs:138`

Problem:

ASCII rendering applies local preprocessing for padding directives before core
parsing. This may be intentional, but it is a boundary smell: user input syntax
preprocessing normally belongs near parser/config handling, not in one output
adapter.

Impact:

ASCII-only input behavior can drift from parse/render contracts and may surprise
binding or CLI callers.

Suggested direction:

Re-evaluate whether these directives are truly ASCII output options, Mermaid
directives, or core parse/config concerns.

### ARCH-034 - Documentation and workstream state can contradict active gates

Priority: P1

Locators:

- `docs/alignment/STATUS.md:104`
- `docs/alignment/STATUS.md:130`
- `docs/alignment/STATUS.md:131`
- `docs/alignment/STATUS.md:608`
- `docs/workstreams/mermaid-11-15-root-viewport-residuals/TODO.md`
- `docs/workstreams/mermaid-11-15-root-viewport-residuals/HANDOFF.md`

Problem:

Historical notes say the global `parity-root` gate is green in some places, while
newer status text says root viewport parity is not enforced by `xtask verify` and
is tracked by an active residual lane.

Impact:

The release state can be misread. Agents can close or skip work based on older
green statements even though a newer lane is active.

Suggested direction:

Make current state summaries generated or clearly separated from historical
progress notes. Link active gates and workstreams from one authoritative status
section.

### ARCH-035 - Public layout/render APIs expose implementation-era seams

Priority: P1

Locators:

- `crates/merman/src/render/mod.rs:146`
- `crates/merman/src/render/mod.rs:186`
- `crates/merman/src/render/mod.rs:230`
- `crates/merman/src/render/mod.rs:497`
- `crates/merman-render/src/lib.rs:329`
- `crates/merman-render/src/lib.rs:339`

Problem:

Some public or near-public APIs expose the historical split between JSON parse,
typed render parse, layout, SVG, and pipeline stages. This is useful for testing,
but the canonical user-facing path is not clearly separated from migration-era
compatibility paths.

Impact:

New adapters may choose the wrong path and miss typed-first behavior, metadata,
pipeline policy, or raster-safe processing.

Suggested direction:

Define the canonical headless operation path and keep lower-level parse/layout
functions available as explicit expert/debug APIs.

Related decisions:

- ADR-0004 public API and headless output
- ADR-0063 extensible SVG output pipeline
