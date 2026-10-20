# Docgen Resource Policy And Host Theme Defect Diagnosis

Date: 2026-10-20. Status: diagnosis and recommended implementation boundaries; no runtime fix applied.

## Evidence Boundary

- Current main: `a933b02c1976a5034350d96574aea048e393c04e`, matching the remote main observed during diagnosis.
- Released `v0.9.1`: `f6202f702dbde0a75049b1a62b3124feae709b3d`.
- Main selects Mermaid `12.2.0`, commit `1fb7869866dd619168d840291c9c54b4f129758b`; `v0.9.1` selects `12.1.0`, commit `21f72f07ea22c0af48a3149c550654e80d8e40cb`.
- The existing `repo-ref/mermaid` working checkout was at another revision. The selected 12.2 commit was fetched and read through `git show`; its working checkout was not changed.
- All five issue bodies and complete available comment threads were read. They had no comments at the time of diagnosis.
- A fresh main CLI was built with only SVG and the six affected diagram families, using the shared target directory and two build jobs. The build succeeded with existing warnings in the partial feature selection; this is not a Clippy or full-workspace test result.
- Eighteen successful renders covered C4, Gantt, Wardley, ZenUML, Quadrant, and ER with no typed theme, `editor-light`, and `editor-dark`, using `resvg-safe` and the interactive resource profile. Seven named behavioral checks fail on the reported defects. Inputs, SVGs, and observations remain under ignored `target/diagnostics/issues-228-232-20261020/`.
- The Linux font failure was not reproduced end to end on this Windows host. Dependency tracing and inspection of the actual upstream Symbol font provide the font-specific evidence below.

## Mermaid-Backed Repair Contract

The repair uses the selected Mermaid implementation for official configuration precedence, theme-variable derivation, and state semantics. It does not assume that upstream implements Merman's host-theme API. All five reported issues belong to the repair/backport scope; internal restructuring needed to repair them is not excluded merely because it extends an adapter.

Mermaid's `updateCurrentConfig` rebuilds a newly selected theme from raw explicit initialization variables merged with raw allowed source variables. It discards the previous theme's derived variables, not every initialization value. Preserve this distinction: explicit site variables can intentionally survive a source theme selection. Keep host-default projection, authored compatibility variables, explicit site/source variables, and calculated outputs distinguishable until final paint selection.

For plain Mermaid input, use the source-backed behavior as the oracle. For Merman host-theme input, keep that same source precedence and resolve the complete affected family surfaces coherently. Upstream Wardley anchors and ZenUML's plugin do not provide full dark-theme behavior: their missing coverage cannot serve as evidence that Merman's host-theme adapter is complete. The selected ZenUML plugin at `a86a2bf4d8fd2a9045f564b5b37c4c70cde18ca6` calls `renderToSvg(code)` without forwarding Mermaid theme variables.

Tests should compare official behavior with the appropriate branch's Mermaid baseline, and separately assert Merman's host-theme contract. Do not invent a universal contrast algorithm: preserve explicit author colors and use upstream derivation rules or deliberate preset foreground/background pairs.

## Docgen Commit: An Older Resource Contract

The [docgen commit](https://github.com/josselin-pujo/docgen/commit/608646f8dd7708f277986b061bb9521453d2e743) uses `0.8.0-alpha.7`, whose layout ceilings were 800,000 for interactive and 1,000,000 for trusted native. Its reported 34-node graph exceeded those ceilings. Node count alone does not establish derived layout cost; the error's `actual` is the rejected charge or admission estimate, not the total budget needed to finish.

Both current main and `v0.9.1` already contain the later calibration: 14,100,000 interactive units and 15,000,000 trusted-native units. See [interactive calibration](../performance/interactive_layout_work_calibration_2026-10-01.md), [native calibration](../performance/native_layout_work_calibration_2026-10-01.md), and commit `740755137`. The exact reported graph is not included in the commit diff and was not obtained during this diagnosis, so these facts do not prove that specific graph fits the new defaults.

The direct `merman-render` dependency added by docgen is unnecessary: even the alpha.7 facade re-exports `RenderResourcePolicy` through `merman::svg`. On `v0.9.1` and main, configuring `SvgEnvironment::with_resource_policy` also supplies graphical input admission unless the caller explicitly overrides it. Separate input and render settings are required for alpha.7, not for ordinary current graphical requests. See [host resource policy](../integration/RESOURCE_POLICY.md).

Recommendation: no new budget increase or default unbounded mode for `v0.9.2`. Offer current finite defaults and a targeted finite override for measured exceptional inputs. Preserve explicitly selected unbounded operation for trusted hosts; backend tree caps do not bound all preceding layout work or host memory. Any further budget or algorithm change requires the actual failing source and measurements.

## #228: Implicit Font Selection Accepts A Symbol Font

[Issue #228](https://github.com/Latias94/merman/issues/228) is independent of theme compilation. `fontdb` 0.23's Linux loader parses fontconfig XML aliases into a single generic-family name; it does not perform the platform's actual matching operation. The upstream [URW Symbol configuration](https://github.com/ArtifexSoftware/urw-base35-fonts/blob/master/fontconfig/urw-standard-symbols-ps.conf) includes a `serif` preference for Standard Symbols PS. The reporter's exact configuration loading order remains unverified.

Main has two generic-selection implementations: `merman-export/src/font_environment.rs:460` and `merman-export/src/lib.rs:3783`. Both accept an existing generic hit; implicit default and final fallback paths can also choose an arbitrary first face. The equivalent released implementations are in `lib.rs` and `fonts.rs`.

The actual Standard Symbols PS font has Unicode cmaps and complete ASCII a-z coverage, but maps `a` to `alpha`, `m` to `mu`, and `A` to `Alpha`. Coverage checks, nonzero glyph IDs, and rejection of only Windows Symbol cmaps do not fix this case. Its OS/2 family class is `0x0C03` (Symbolic), and PANOSE family type is 5 (Pictorial); see the [OpenType classification specification](https://learn.microsoft.com/en-us/typography/opentype/spec/ibmfc).

Recommended shared implicit-font policy:

1. Reject candidates explicitly classified as symbolic/pictorial or Symbol-encoded for generic/default/final implicit fallback. Preserve deliberate named Symbol requests.
2. Preserve valid generic choices and prefer compatible admitted serif/sans/mono candidates. Do not hard-code a Linux font list or silently replace every serif request with sans-serif.
3. Remove arbitrary-first-face success when every implicit candidate is unsuitable; retain an explicit unresolved outcome.
4. Extract small classification metadata while discovery already owns the font bytes. Bounded native discovery replaces `FaceInfo.source` with empty binary data after extracting cmap information, so reparsing all faces later would reject valid bounded faces and add redundant I/O.
5. If platform-equivalent matching is required, use a mature matcher rather than extending an XML alias interpreter. The existing optional Fontique backend uses real Fontconfig substitution/sorting. Enabling it for ordinary binary export changes dependency/linkage and feature contracts, so assess that separately from the bounded-provider repair.

The shared policy can be backported independently of themes. A direct `ttf-parser` dependency may be required in the relevant feature closure; the version is already present transitively. Keep resolver identity consistent with changed behavior.

Tests should include a small appropriately licensed symbol fixture with Unicode ASCII-to-Greek mappings, a real serif candidate, poisoned generic selection, varied insertion order, named Symbol opt-in, symbol-only failure, and metadata retained after bounded discovery clears face sources. Assert the actual selected painting face. A Linux process-level fixture with URW fonts and isolated `FONTCONFIG_FILE` must cover both explicit `serif` and ZenUML's default stack.

## #229: C4 Paint Order And Missing Theme Terminals

[Issue #229](https://github.com/Latias94/merman/issues/229) combines two defects.

`svg/parity/c4/render.rs:68` emits a boundary after its descendant shapes. Default `fill="none"` conceals the error; typed `editor-dark` introduces opaque `#1e293b`, covering its children. This order also exists in `v0.9.1` and pinned Mermaid's transparent-boundary renderer. An explicit filled boundary can expose it on the release branch too.

Split the boundary into a background paint before descendants and foreground border/label after descendants. Preserve transparent parity structure where practical. Do not fix visibility by forcing transparency or adding an SVG postprocessor. Test nested opaque boundaries, not only serialized label order. The existing `c4_svg_test.rs:1379` deliberately records the historical order and needs a separate filled-background contract.

Relationship colors are prepared from `c4.relationship_*` configuration in `c4/typography.rs:160`, with `#444444` defaults; typed presets never reach them. Boundary labels have the same gap. Extend the existing Edge, EdgeLabel, Marker, and ClusterLabel family applicability, and bind final colors in the C4 family plan. Explicit site/source values and per-relation `UpdateRelStyle` retain priority. Arrowheads must follow the final relation stroke.

## #230: Gantt Preset Semantics And Wardley Anchor Ownership

[Issue #230](https://github.com/Latias94/merman/issues/230) reports two independent family gaps.

Gantt already has six normalized task states. `diagram_theme/presets/catalog.rs:1071` nevertheless creates an unqualified Task surface fill for editor presets. It legitimately matches every state and is emitted inline, overriding native state CSS. Changing all unqualified user Task rules to mean only Default would break the deliberate contract in `gantt_svg_test.rs:335`.

Fix presets with state-qualified task fills and paired TaskLabel foregrounds. Critical status is orthogonal: pinned Mermaid gives ActiveCrit the active fill plus critical stroke, and DoneCrit the done fill plus critical stroke. Resolve fill-state and stroke-state within Gantt without adding a global Cartesian product of variants. Preserve explicit custom unqualified rules.

`gantt/theme/css_binding.rs:86` changes ordinary task text but leaves active/done text reading `task_text_dark_color` through `!important` CSS. Labels need final per-state foregrounds, with outside labels resolved against the section/canvas rather than the bar. Cover all six states, combinations, inside/outside labels, milestones, and vertical markers.

Wardley `wardley/theme.rs:65` uses only `anchorLabelColor`, falling back to `#000`; ordinary component labels follow another path. This is inherited upstream behavior, not adequate typed-theme support. Add anchor label terminals to the family plan, preserve explicit anchor overrides and evolved-component colors, and keep no-theme parity stable. The existing family-color fixture lacks anchors.

## #231: ZenUML Needs A Coherent Family Plan

[Issue #231](https://github.com/Latias94/merman/issues/231) is reproducible: typed Title.fill changes to `#e5e7eb`, while the header stays white and participant text stays `#222`. `zenuml/theme.rs:104` supports only the title, and `family_mechanism_matrix.rs:1897` declares generic text unsupported. The guide accurately documents this limitation, but the partial application produces unreadable output.

Additionally, `zenuml/theme.rs:170` hard-codes the title's source-owned flag to false, so explicit `themeVariables.zenuml.textColor` can also lose to typed title fill.

Replace the title-only plan with one prepared ZenUML style plan for frame/header, participants, message lines/labels, lifelines, notes, groups, activations, and typography. Reuse applicable existing semantic targets. Measurement and painting must consume the same prepared typography; explicit namespace properties retain per-terminal ownership. Keep the Mermaid compatibility input separate instead of translating typed defaults back into global JSON.

Suppressing title recoloring alone is a possible containment patch, not completion of theme support. Released `v0.9.1` has no typed-title regression; it has the older all-light ZenUML implementation. Fixing its old host-theme gap requires a release-local namespace adapter and HostTheme mapping, not importing the main typed compiler.

## #232: Preserve Palette Authority Through Final Paint Selection

[Issue #232](https://github.com/Latias94/merman/issues/232) is not a failure to recalculate Mermaid theme values. `parse_pipeline.rs:124` and `config/appearance.rs:95` already select and materialize the source theme from authored layers, consistent with pinned Mermaid's [configuration implementation](https://github.com/mermaid-js/mermaid/blob/1fb7869866dd619168d840291c9c54b4f129758b/packages/mermaid/src/config.ts).

The later selection loses authority at two seams:

- Quadrant text checks exact variable ownership, so a source `theme: base` owns its light palette but does not protect derived point text from typed overrides.
- ER freezes a raw paint-input bitset. A source `forest` theme produces matching row/background/text colors, but derived text does not count as a raw authored variable and loses to typed text.

Retain minimal palette-authority metadata while AppearanceDecision still distinguishes Source, Initialize, Compatibility, and Default. Final family paint resolution must use the same authority and existing variable derivation ownership. Do not infer origin from the final theme string or equal color values: a recipe's own `theme: dark` must not disable its typed rules.

Recommended contract: an explicitly selected registered source theme owns its materialized paint palette, calculated from the raw initialization/source variables that Mermaid preserves; an explicitly selected site theme similarly owns that palette when no source selection supersedes it. Host typed paint supplies the fallback when neither external layer selects the owner. Explicit user variables remain valid calculation inputs; only stale derived defaults are discarded. A recipe's own compatibility theme remains part of the recipe and does not suppress its typed rules. This is a deliberate precedence correction: `er_svg_test.rs:3068` and several typed fixtures currently encode the opposing behavior. Update those expectations and the documentation openly rather than treating them as accidental failures.

Palette authority must not indiscriminately disable independent typography, geometry, assets, or effects. Explicit individual theme variables continue through their real calculation dependencies; do not infer a whole palette replacement from any arbitrary single fill, or guess text colors with global contrast heuristics. Arbitrary explicit author colors cannot carry a universal readability guarantee. Preserve secure filtering, scoped/root choices, same-value ownership, the null sentinel, and invalid-name fallback.

For `v0.9.1`, the equivalent defect occurs earlier: HostTheme compilation flattens derived host tokens into initialization JSON, losing their distinction from explicit site values. Preserve the old public Presentation API, but keep its internal host projection separate so a source theme can materialize its own palette without stale host-derived labels. Include the reported old Timeline `cScale0..2` case.

## Implementation And Backport Order

| Work unit | Main | v0.9.2 from v0.9.1 |
| --- | --- | --- |
| Resource defaults/API | Already repaired | Already repaired; no new fix required |
| Shared implicit font policy | Unify both implementations | Independent semantic backport into older file layout |
| C4 background/foreground paint order | Repair painter | Same existing defect; independent backport |
| C4/Wardley missing color owners | Extend family plans/targets | Repair legacy host projection and family/config readers |
| Gantt preset states and labels | Repair current typed presets/plans | Reproduce through the legacy host-theme entry point and repair its state/foreground contract; the current typed inline path itself is absent |
| ZenUML coherent theming | Complete typed family plan | Adapt old host theme and namespace reader as part of the issue repair |
| Source palette authority | Correct shared provenance/final selection | Release-local internal separation retaining Presentation API |

Share behavior fixtures and assertions, not a large theme-refactor cherry-pick. Create the maintenance branch from the exact released tag; keep its Mermaid 12.1 baseline and public options/API stable. Use independent commits for font policy, C4 paint ordering, family terminal coverage, preset semantics, and palette precedence. Mechanically cherry-pick only where dependencies and tests agree; otherwise implement the same specified behavior through the release's existing owners.

## Verification Required Before Shipping Fixes

- Strengthen existing family and binding tests with pure preset inputs. `host_render_contract.rs::options()` manually injects C4/Wardley/ZenUML settings and therefore does not prove pure preset support. Historical screenshots are not current-head evidence.
- Exercise light/dark, no theme, explicit site overrides, frontmatter/directives, and combined host/source choices. Check actual terminal colors and layer order, not only rule application counts or color-token presence.
- For #232, cover Quadrant and ER with HTML/native labels, plus Timeline on the maintenance branch. Test repeated renders to prevent state/provenance leakage.
- Render selected SVG cases to actual PNG and PDF where relevant; add Linux font-environment coverage. Do not equate nonempty output or nonzero glyph IDs with correct content.
- Run focused `cargo nextest` owners first, then applicable feature, formatting, Clippy, native export, and parity gates. Source-backed parity remains the no-theme control; do not normalize away semantic differences.
- No fixes, commits, pushes, PRs, version changes, or releases were performed by this diagnosis.
