---
title: "ASCII Terminal Layout, Visual Hierarchy, and Theme Architecture Research"
type: "research"
date: "2026-08-26"
status: "implemented, verified, and independently reviewed"
scope: "merman-ascii terminal and agent-CLI output"
---

# ASCII Terminal Layout, Visual Hierarchy, and Theme Architecture Research

## Executive conclusion

Merman has completed the bounded architectural refactor recommended by this research for the first
two evidence families. The implementation preserves the agent-CLI foundations—terminal-cell
measurement, pre-layout Flowchart label wrapping, typed viewport outcomes, complete-output-or-error
behavior, and a Plain deterministic default—while separating host, family layout, visual role, and
output policy internally.

The implemented split separates three concerns that were adjacent but not cleanly independent:

1. **Layout policy**: family-owned spacing, wrapping, route-lane, and density decisions.
2. **Visual semantics**: a role-based hierarchy for text, structure, labels, groups, and emphasis.
3. **Terminal capability**: charset, display-width profile, color capability, TTY/`NO_COLOR`, and
   host-owned viewport selection.

Grok Mermaid is useful prior art, especially for a bounded cell canvas, explicit width gating,
loss-aware fallback, and pager-level terminal affordances. It is not a suitable semantic or visual
oracle for Merman: its parser and renderer intentionally cap/truncate labels, its fallback exposes
raw source, and its five-style-class palette is optimized for one pager/web product. Mermaid upstream
is the semantic authority, but its spacing and wrapping values are pixel/font/browser concepts. They
must be translated into terminal-cell policies rather than copied literally.

**Conclusion:** keep canonical geometry and Plain output as the compatibility floor, but allow an
alpha schema break when machine truthfulness requires it. Schema 2 was necessary to identify styled
text encoding. Flowchart and Sequence now have separately admitted opt-in Compact policies and the
renderer has a terminal-native ANSI16 profile. Compact and styled output remain explicit choices;
neither becomes the default from screenshot preference alone.

## Evidence boundary and revisions

This report uses repository-local source as the primary evidence. Reference repositories are cloned
under `repo-ref/` and are not product dependencies. The revisions examined were:

| Source | Revision / version | Relevant evidence |
| --- | --- | --- |
| Merman baseline | `acf689eff` (`origin/main`, merge of PR #100) | Starting ASCII options, color roles, output report, viewport contract, CLI adaptation, and ADRs |
| Merman implementation | `1faa86ea2` through the review closeout on `research/ascii-terminal-ux` | Bounded fallback writer, resolved family/output policies, ANSI16 roles, Flowchart/Sequence Compact admission, schema-2 report transport, CLI contract-5 capability/error discovery, State isolation, evidence matrix, and exact work accounting |
| Grok Build | `c2ad97f87aea4303b6000a2c22128bc91ee76c9b` | `xai-grok-markdown` Mermaid cell renderer and pager/theme integration |
| Mermaid | `41646dfd43ac83f001b03c70605feb036afae46d` (`mermaid@11.15.0`) | Flowchart spacing/wrapping and theme-variable resolution |
| Simon Willison tool | fetched 2026-08-26 | `tools.simonwillison.net/grok-mermaid` wrapper behavior and user-facing width/fallback policy |

The research deliberately distinguishes source-backed behavior from design inference. A proposed
role or profile below is not claimed to exist in upstream Mermaid or Grok.

The immediate product context is [Issue #53](https://github.com/Latias94/merman/issues/53), with the
pre-layout label-wrapping work in [PR #88](https://github.com/Latias94/merman/pull/88) and the
viewport/output contract in [PR #100](https://github.com/Latias94/merman/pull/100). Those changes are
treated as the regression floor for this research, not as work to repeat.

## Starting Merman baseline

### Contracts that are already correct

The merged ASCII work gives us a strong base:

- `AsciiRenderOptions` has explicit charset, terminal width profile (`Unicode`/`Cjk`), canonical
  and opt-in compact layout profiles, color mode/theme, and family-specific sizing knobs
  ([`options.rs`](../../crates/merman-ascii/src/options.rs)).
- Flowchart labels are measured and wrapped in terminal cells before node sizing and route planning.
  This is the correct order for Issue #53 and must remain true for every future profile.
- `AsciiViewportPolicy` separates host-requested maximum width and overflow behavior from renderer
  resource limits. `Allow`, `Fallback`, and `Error` are materially different outcomes
  ([`output.rs`](../../crates/merman-ascii/src/output.rs)).
- `AsciiOutput` reported primary/emitted extents, projection, overflow, fallback metadata, width
  profile, layout profile, and lossiness. The implementation retained that vocabulary and added an
  explicit schema-2 encoding identity; callers still do not infer state from string prefixes.
- The renderer has semantic color roles rather than sprinkling direct colors through family code:
  text, muted text, node/group borders, edge line/arrow/label, sequence roles, chart axes, and
  series roles ([`color.rs`](../../crates/merman-ascii/src/color.rs)).
- The CLI owns environment-sensitive `auto` color selection. It checks `NO_COLOR`, destination
  (TTY versus file), `TERM`, and `COLORTERM`, then resolves to a deterministic library color mode
  ([`invocation.rs`](../../crates/merman-cli/src/invocation.rs)). The library does not need to probe
  the host terminal.
- ADR 0065 correctly keeps ASCII as an independent model-to-grid adapter, not a quantized SVG
  export ([`0065-ascii-output-boundary.md`](../adr/0065-ascii-output-boundary.md)).

These decisions are more important than matching any particular screenshot. They are the semantic
and operational advantages of Merman's implementation.

### Tensions that justify another refactor

The `origin/main` starting shape exposed three architectural pressures:

1. **One options record owns too many policy domains.** Charset, width convention, layout density,
   family geometry, color encoding, color palette, and diagnostic toggles all travel in
   `AsciiRenderOptions`. This is convenient for alpha development but makes it difficult to add a
   terminal-native profile without accidentally changing family geometry or fallback behavior.
2. **The role vocabulary is structurally good but visually shallow.** It distinguishes border, line,
   arrow, label, and text, but it has no first-class surface/background role, section/title hierarchy,
   active/status emphasis, or accessibility-oriented semantic state. A theme can therefore change
   colors without fully expressing visual hierarchy.
3. **Density was not yet evidence-driven across families.** `Compact` changed a small set of
   shared defaults (graph padding, Flowchart wrap width, sequence spacing), while XYChart, Class, ER,
   and structured-text families have different density constraints. A single global compact switch is
   a useful experiment handle, not a finished layout architecture.

The implementation addressed these pressures with resolved policy records, expanded semantic roles,
and family-local Compact defaults. `AsciiRenderOptions` remains the alpha compatibility façade; it no
longer mutates shared options to apply Compact. This is a bounded deepening rather than replacement
of the model-to-grid boundary.

## Grok Build comparison

### What Grok does well

The core implementation in
[`xai-grok-markdown/src/mermaid.rs`](../../repo-ref/grok-build/crates/codegen/xai-grok-markdown/src/mermaid.rs)
has several ideas worth borrowing:

- It lays out into a bounded terminal-cell `Canvas` with explicit width/height and a hard cell cap.
- It separates the canvas's semantic classes (`Border`, `Text`, `Edge`, `EdgeLabel`) from the final
  `ratatui::Style`, allowing the same geometry to produce styled and plain lines.
- It applies the maximum-width gate after normal layout. An over-wide diagram does not silently
  reshape itself; it becomes an explicit fallback. This is a useful model for Merman's
  provider-neutral viewport contract.
- It keeps parser/layout/render caps explicit (`MAX_NODES`, `MAX_EDGES`, groups, nesting, and canvas
  cells). The cap is observable in tests rather than being an accidental allocator failure.
- Its pager has a separate `render_mermaid` preference (`auto | on | off`) and a separate image/open
  affordance layer ([`render_mermaid.rs`](../../repo-ref/grok-build/crates/codegen/xai-grok-pager-render/src/appearance/render_mermaid.rs)).
  This is a strong example of keeping product interaction policy outside the cell renderer.
- The pager's terminal-default theme uses `Color::Reset` and sparse named ANSI accents instead of
  guessing the terminal background. That is a practical polarity-safe strategy for a TTY product
  ([`terminal_default.rs`](../../repo-ref/grok-build/crates/codegen/xai-grok-pager-render/src/theme/terminal_default.rs)).

The Simon Willison page demonstrates the same layering in a smaller wrapper: an editable source
area, explicit width choices (fit/80/100/120/160/unlimited), copy actions, and a policy that lets a
wide diagram scroll in the browser rather than changing its semantics. The wrapper also exposes a
small role palette (`border`, `node`, `edge`, `edge label`, `title`) and uses the Grok WASM renderer
unchanged.

### What Merman should not copy

Several Grok choices conflict with Merman's complete semantic-output contract:

- Grok wraps to a fixed width and truncates after a fixed number of lines with an ellipsis. Its own
  tests intentionally assert truncation for very long labels. Merman must preserve authored labels
  or return a typed unavailable/error result; an ellipsis is semantic loss for the core renderer.
- Grok's over-wide fallback is a framed raw Mermaid source listing plus a hint. Merman's current
  boundary correctly treats raw source disclosure as host policy, not a renderer-owned semantic
  fallback.
- Grok's five visual classes are enough for a compact pager but do not model groups, surfaces,
  active/status emphasis, sequence activation, chart series, or accessibility diagnostics across
  Merman's broader family set.
- Grok's parser and layout implementation are intentionally self-contained. Copying it into
  `merman-ascii` would violate ADR 0065 and create a second Mermaid semantic implementation.

The useful lesson is **separation and boundedness**, not byte-for-byte output or parser reuse.

## Mermaid upstream comparison

Mermaid's Flowchart configuration exposes `nodeSpacing`, `rankSpacing`, `padding`, `diagramPadding`,
and `wrappingWidth`. The current upstream Flowchart renderer defaults node/rank spacing to 50 and
passes those values to the Dagre layout path. `wrappingWidth` is defined as the width at which text
continues on a new line. These values are pixel/font-oriented and are resolved alongside SVG text
measurement; they are not terminal-cell constants.

Mermaid's theme layer also demonstrates the right semantic granularity. The resolved theme variables
distinguish, among other roles, node background/border/text, cluster background/border, default link
color, edge-label background, title color, sequence actor/line/activation colors, and state-specific
colors. The theme compiler applies defaults and directives in layers.

The implication for Merman is a **semantic translation table**, not a direct config mirror:

| Mermaid/browser concept | Terminal translation |
| --- | --- |
| `nodeSpacing`, `rankSpacing` | Family-local cell gaps, validated against route occupancy and width corpus |
| `wrappingWidth` | Grapheme/display-cell label budget before layout; never post-render slicing |
| `nodeBkg` / `nodeBorder` / `nodeTextColor` | Node surface, node border, and primary text roles (surface may be omitted in plain mode) |
| `clusterBkg` / `clusterBorder` | Group surface/border plus title/section emphasis |
| `defaultLinkColor` / arrow colors | Edge line and marker roles, with plain-mode glyph distinction for dotted/thick edges |
| `edgeLabelBackground` | Optional label-clearance/surface role; in plain mode, spacing or framing rather than color |
| font size/family | Not copied; terminal output uses display-cell width profiles and fixed glyph policy |

Mermaid's browser theme precedence remains useful as a conceptual model (base theme → explicit
variables/directives → source-local overrides), but Merman must keep host terminal capability and
viewport outside Mermaid semantic configuration.

## Terminal and accessibility constraints

### Display width is contextual

Unicode Standard Annex #11 describes East Asian Width as a contextual property and explicitly warns
that modern terminal emulators need tailoring. Ambiguous characters, combining marks, variation
sequences, emoji, and CJK punctuation cannot be made universally correct by byte length or a single
browser font metric. Merman's `TerminalWidthProfile::Unicode`/`Cjk` split is therefore the right kind
of public control. The layout and measurement pipeline must continue to use the same profile for
wrapping, placement, extent reporting, and final encoding.

### Color capability belongs to the host adapter

`NO_COLOR`, TTY detection, `TERM`, and `COLORTERM` are environment/policy signals. They should be
resolved by CLI or another host adapter, as Merman already does, and converted into an explicit
`AsciiColorMode`. A library call must remain reproducible when run in CI, a pipe, a file, or an FFI
host with no process environment.

### Contrast is a floor, not the hierarchy

WCAG's 4.5:1 text and 3:1 large-text thresholds are useful conservative checks for RGB/HTML themes,
but terminal ANSI palettes cannot assume a known background or exact colorimetry. Contrast checks
should apply to explicit light/dark/HTML palettes; a terminal-native palette should prefer
`Reset`/named ANSI and preserve hierarchy through glyph choice, spacing, weight, and role placement.
Color must never be the only carrier of edge type, error state, or group membership.

## Agent CLI requirements

Agent and automation consumers differ from an interactive pager:

- The default must be plain, deterministic, and free of escape sequences.
- A caller must be able to request a width and receive a typed outcome without parsing text.
- Complete labels, endpoints, markers, edge labels, and structured fields matter more than fitting
  an arbitrary width.
- The renderer must not infer a token budget, provider, pager, or model persona.
- A wide `Allow` result is valid when the host can scroll or store an artifact; `Fallback` and
  `Error` must remain explicit.
- Any visual hierarchy that disappears in plain mode is a semantic design defect.
- ANSI/HTML decoration must not change logical cell extents or alter wrapping decisions.

The starting `AsciiOutput` report and CLI flags already satisfied most of this boundary. This
implementation improved the internals and visual vocabulary without introducing an `AgentPreset`
inside the renderer.

## Implemented target architecture

### 1. Split request, layout, visual semantics, and encoding

The implementation uses four conceptual layers (the public façade can still change during alpha):

```text
host request
  ├─ viewport / overflow / trim
  ├─ terminal capability (charset, width profile, color mode)
  └─ output/report request
        ↓
family layout policy
  ├─ canonical / compact candidate
  ├─ cell padding and gaps
  ├─ wrap budgets
  └─ route and occupancy policy
        ↓
semantic scene + cell canvas
  ├─ topology and geometry
  ├─ visual roles
  └─ plain/styled cell content
        ↓
output encoder + AsciiOutput report
```

`AsciiRenderOptions` remains the alpha compatibility façade, while `ResolvedAsciiPolicies` carries
narrower host, family-layout, and output policy records. Flowchart and Sequence consume their own
family-local policies; State and XYChart consume separate resolved policies. This prevents a
Flowchart Compact change from silently changing State or another family.

### 2. Expand the semantic role vocabulary

The role taxonomy was expanded without changing Plain output:

| Role group | Implemented roles | Plain-mode fallback |
| --- | --- | --- |
| Canvas/surface | `Surface` | Whitespace, padding, or explicit framing |
| Structure | `NodeBorder`, `GroupBorder`, `EdgeLine`, `EdgeArrow`, `Junction`, `ChartAxis` | Box glyph, line glyph, marker glyph, placement |
| Content | `Text`, `MutedText`, `Title`, `Section`, `EdgeLabel`, `Diagnostic` | Position, indentation, prefix, or label frame |
| State/emphasis | `StatusEmphasis` | Existing status text, glyph, or placement |
| Family accents | `SequenceLifeline`, `SequenceActivation`, `SequenceFrame`, `ChartSeries(i)` | Existing family glyph/ordering semantics |

Do not add a direct RGB field for every future visual idea. Keep roles semantic, make themes map
roles to a palette, and let family renderers decide which roles are meaningful.

### 3. Make hierarchy explicit in geometry

A terminal diagram should be readable in this order:

1. topology and direction;
2. group/subgraph boundaries;
3. node or participant identity;
4. edge direction and kind;
5. edge labels and secondary metadata;
6. diagnostics/disclosure text.

Spacing should reinforce that order. Prefer stable inter-rank/inter-participant gaps and local label
clearance over global whitespace reduction. A compact profile may reduce outer padding first, then
inter-node gaps, but must not reduce the clearance needed to distinguish an edge label from a route
or a group title from a node.

### 4. Treat themes as capability-aware profiles, not automatic detection

The admitted encoder set remains intentionally small:

- `Plain`: no color, canonical semantic glyphs; default for library and agent output.
- `Ansi16Terminal`: `Reset` foreground/background plus sparse named accents; safe when the host
  cannot guarantee a background polarity.
- `Ansi256`/`TrueColor`: explicit light/dark palettes with contrast-tested role mappings.
- `Html`: explicit background/surface roles and CSS-safe output.

The CLI may keep `auto`, but `auto` must resolve before the renderer is called. A “terminal theme”
should not read the OS appearance or query OSC 11 from inside `merman-ascii`.

### 5. Preserve one output contract

Keep `AsciiOutput` as the canonical result. If a later optimization introduces a text-only fast
path, it must be an internal optimization or an explicitly secondary convenience API; it must not
create a second report schema. In particular, viewport outcome, logical extent, projection, and
lossiness must remain available to FFI/CLI callers.

## Alternatives considered

| Option | Benefits | Problems | Decision |
| --- | --- | --- | --- |
| Keep expanding `AsciiRenderOptions` | Smallest immediate diff | More coupling, difficult family-local experiments, theme/layout ownership stays mixed | Reject as final architecture; acceptable migration façade |
| Split layout policy, visual roles, and terminal capability | Preserves current contracts while enabling measured profiles and richer themes | Requires internal/public migration and corpus work | **Adopted** |
| Copy Grok's renderer and presets | Fast visual result for a few families | Second parser, semantic truncation, raw-source fallback, narrow product assumptions | Reject |
| Quantize SVG layout into cells | Reuses existing geometry | Font/browser dependence, poor routing and wrapping, violates ADR 0065 | Reject |
| Add automatic terminal probing to the library | Convenient for one CLI | Non-deterministic FFI/CI behavior and hidden environment dependency | Reject |
| Make compact/terminal style the default now | Better narrow-screen screenshots | Changes compatibility output before evidence and risks semantic loss | Reject for now |

## Implementation evidence matrix

The admission slice was evaluated as a matrix rather than by one screenshot:

| Dimension | Values |
| --- | --- |
| Width | unrestricted, 60, 80, 100, 120 cells |
| Layout | canonical, family-local compact |
| Charset/width | ASCII/Unicode structure crossed with Unicode/CJK width profiles |
| Encoding | Plain, ANSI16, ANSI256, TrueColor, HTML where supported |
| Families | Flowchart and Sequence admission; State isolation characterization; other families deferred |
| Labels | long prose, identifiers with `_-/.'`, CJK, combining marks, emoji/ZWJ, hard breaks |
| Topology | chains, fan-in/out, cycles, subgraphs, dense relation components |

The matrix records these properties through direct metrics or semantic/collision assertions:

- authored-field retention (labels, endpoints, markers, edge labels, structured keys);
- primary/emitted width and height;
- route crossings and node/group overlap;
- label-to-route clearance;
- whitespace ratio and longest uninterrupted blank run;
- plain/styled logical extent equality;
- resource/cancellation behavior at exact and N-minus-one boundaries.

Admission should remain semantic-first: a smaller diagram that drops a field is worse than a wider
diagram that the host can scroll.

The executable matrix lives in
[`viewport_characterization.rs`](../../crates/merman-ascii/tests/viewport_characterization.rs). Its
representative Plain measurements are:

| Fixture | Layout | Extent | Area | Blank cells | Longest blank run |
| --- | --- | ---: | ---: | ---: | ---: |
| Issue #53 Flowchart | canonical | `74×57` | 4218 | 3220 | 51 |
| Issue #53 Flowchart | compact | `58×67` | 3886 | 3006 | 43 |
| Sequence self-messages/notes | canonical | `82×58` | 4756 | 3227 | 20 |
| Sequence self-messages/notes | compact | `78×58` | 4524 | 2982 | 21 |

For the representative ASCII/Unicode-width cases, Flowchart canonical needs Plain fallback at 60
cells and is primary at 80/100/120; Flowchart Compact is primary at all four widths. Sequence
canonical needs Plain fallback at 60 and 80 and is primary at 100/120; Sequence Compact needs
fallback at 60 and is primary at 80/100/120. Every styled encoder preserves the Plain logical
geometry for the same layout. Styled Fallback is rejected during preflight, while Plain Fallback
retains the required authored fields.

## Actual CLI, FFI, and binding impact

The internal policy split preserved the existing Rust option façade, but truthful machine metadata
required an atomic alpha migration:

- CLI report mode now resolves `auto` to Plain and rejects explicit styled report output.
- CLI contract 5 exposes report schemas/streams, per-family layout/width/encoding/fallback arrays,
  and detector-to-family mappings; report-mode failures are typed Plain JSON on stderr.
- `AsciiOutput` and binding output plans use schema 2 and identify `encoding`.
- capability records expose `layout_profiles`, `width_profiles`, `encodings`, and
  `fallback_encodings` for render-time preflight.
- UniFFI binding API 6 replaces the API 5 version-probe symbol because the capability and output-plan
  record layouts changed; generated Swift/Python wrappers and the native library must move together.
- Web, Flutter, Apple, Python, Playground, and generated operation contracts project the same source
  descriptors. Web still exposes text plus capabilities rather than a second structured result API.

No provider-specific agent fields or renderer-owned terminal probing were added.

## Implemented sequence and follow-up

1. Canonical snapshots and the Issue #53 retention fixture remain the regression floor.
2. The internal family-policy seam is implemented; the public builder remains the alpha adapter.
3. `Title`, `Section`, `Surface`, `Diagnostic`, and `StatusEmphasis` roles are implemented without
   changing Plain geometry.
4. ANSI16 uses terminal Reset plus sparse named accents without environment probing.
5. Flowchart and Sequence passed the admission matrix and retain separately resolved Compact
   defaults. State isolation is covered explicitly.
6. Class, ER, XYChart, and structured-text density work remains family-local follow-up rather than
   inheriting Compact by analogy.
7. Schema 2, CLI contract 5, and UniFFI API 6 migrated the public report/capability changes
   atomically.

This sequence is a bounded refactor, not a rewrite of parsers or a new rendering backend.

## Non-goals and residuals

- Pixel parity with Mermaid SVG or browser screenshots.
- Copying Grok's parser, truncation rules, raw-source fallback, or pager implementation.
- Automatic terminal detection in `merman-ascii`.
- A token counter, provider adapter, MCP envelope, or agent persona in the renderer.
- Making color the only carrier of semantic state.
- Claiming that current RGB themes are safe on every terminal background.
- Broad global responsive re-layout before family-local evidence exists.

The previous Flowchart semantic-fallback intermediate has been removed: bounded typed JSON is now
flattened directly from a cancellation-aware writer under the active resource profile. Remaining
Flowchart and Sequence compatibility projections stream from family-owned adapters without cloning
arbitrary nested property trees. Two bounded performance residuals remain: custom RGB themes may
repeat terminal-color quantization per styled cell, and the borrowed `RawValue` flattener reparses
nested slices while walking its policy-limited buffer. These are benchmark-gated optimization
opportunities, not correctness or memory-safety gaps. Other work is product- and family-specific:
Compact admission for Class/ER/XYChart or structured-text families, pager/scroll/copy affordances in
host layers, optional responsive retries, and a structured WASM result convenience API if a real
consumer needs it. None of those residuals justify global layout mutation or renderer-owned
terminal detection.

## Verification closeout

The final implementation gates were run from `research/ascii-terminal-ux` on 2026-08-26 with one
Cargo job at a time:

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo nextest run -p merman-ascii -j1` | 1311 passed |
| `cargo nextest run -p merman --features ascii -j1` | 164 passed, 2 skipped |
| `cargo nextest run -p merman-bindings-core --features ascii -j1` | 156 passed |
| `cargo nextest run -p merman-cli --no-default-features --features ascii -j1` | 91 passed |
| `cargo clippy -p merman-ascii -p merman -p merman-bindings-core -p merman-cli --all-targets -j1 -- -D warnings` | Passed |
| `cargo run --locked -p xtask -- verify-generated` | Passed |
| `python3 scripts/verify-platform-bindings.py` | Passed; tracked generated Flutter ABI remained unchanged |
| Release, installation, recovery, and Nix Python contracts | 83 passed |

The UniFFI-specific extension checks also passed: 24 tests under the ASCII feature and warning-free
Clippy across all `merman-uniffi` targets.

The deletion and caller audit found no remaining `effective_layout` helper, global Compact mutation,
output-prefix family classifier, or duplicate report authority. Compact resolution remains only in
the Flowchart and Sequence family resolvers. `AsciiOutput` owns report metadata; Binding and UniFFI
records are transport projections, and the CLI consumes the canonical report without rebuilding it.
No abandoned experimental source remains in the reviewed diff. The local Issue #53 fixture and
untracked Flutter platform directories are user-owned artifacts and remain excluded from staging.

Independent review run `20260826-214439-e5ea7586` completed with no unresolved P0 or P1 findings.
The closeout fixed State/Flowchart policy leakage, report encoding and failure typing, preflight
capability coverage (including the `swimlane` detector mapping), Sequence direct-model JSON resource
admission, empty-container preservation in complete semantic fallback, terminal control-text
normalization, and release/Nix packaging of the strict capability contract. Remaining P2
opportunities are benchmark-gated repeated RGB-to-ANSI16 quantization and bounded `RawValue` slice
reparsing. A visible escaped control atom that cannot fit the requested viewport also remains an
honest `fallback_unavailable` outcome rather than being clipped.

## Primary source index

### Merman

- [Issue #53: break labels in ASCII mode](https://github.com/Latias94/merman/issues/53)
- [PR #88: semantic-depth ASCII improvements](https://github.com/Latias94/merman/pull/88)
- [PR #100: ASCII contract hardening](https://github.com/Latias94/merman/pull/100)
- [`AsciiRenderOptions`](../../crates/merman-ascii/src/options.rs)
- [`AsciiColorTheme` and roles](../../crates/merman-ascii/src/color.rs)
- [`AsciiOutput` and viewport policy](../../crates/merman-ascii/src/output.rs)
- [`CLI text options and auto-color resolution`](../../crates/merman-cli/src/cli.rs)
- [`CLI option resolution`](../../crates/merman-cli/src/invocation.rs)
- [`ASCII support matrix`](../rendering/ASCII_SUPPORT_MATRIX.md)
- [`Presentation/theme separation`](../rendering/presentation-themes.md)
- [`ADR 0065: ASCII output boundary`](../adr/0065-ascii-output-boundary.md)
- [`Agent viewport/output plan`](../plans/2026-08-25-1232-refactor-ascii-agent-viewport-plan.md)

### Grok Build and Simon Willison tool

- [`xai-grok-markdown Mermaid renderer`](../../repo-ref/grok-build/crates/codegen/xai-grok-markdown/src/mermaid.rs)
- [`Grok Mermaid pager preference`](../../repo-ref/grok-build/crates/codegen/xai-grok-pager-render/src/appearance/render_mermaid.rs)
- [`Grok terminal-default palette`](../../repo-ref/grok-build/crates/codegen/xai-grok-pager-render/src/theme/terminal_default.rs)
- [Simon Willison's Grok Mermaid tool](https://tools.simonwillison.net/grok-mermaid)
- [Grok Build source repository](https://github.com/xai-org/grok-build/tree/c2ad97f87aea4303b6000a2c22128bc91ee76c9b)

### Mermaid and terminal standards

- [Mermaid Flowchart configuration types](../../repo-ref/mermaid/packages/mermaid/src/config.type.ts)
- [Mermaid Flowchart renderer spacing setup](../../repo-ref/mermaid/packages/mermaid/src/diagrams/flowchart/flowRenderer-v3-unified.ts)
- [Mermaid label wrapping utility](../../repo-ref/mermaid/packages/mermaid/src/utils.ts)
- [Mermaid theme resolution](../../repo-ref/mermaid/packages/mermaid/src/config.ts)
- [Mermaid Neo theme variables](../../repo-ref/mermaid/packages/mermaid/src/themes/theme-neo.js)
- [Unicode Standard Annex #11: East Asian Width](https://www.unicode.org/reports/tr11/)
- [NO_COLOR](https://no-color.org/)
- [WCAG 2.2: Contrast (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)
