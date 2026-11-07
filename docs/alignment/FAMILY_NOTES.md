# Family Compatibility Notes

These notes describe family-specific limits and interpretation rules that are useful when choosing
or debugging an output. They complement the [parity policy](PARITY_POLICY.md),
[configuration contract](CONFIG_FRONTMATTER_SUPPORT.md), and
[semantic-label contract](SEMANTIC_LABEL_PARITY.md). They do not enumerate supported syntax,
duplicate fixture inventories, or record successful gate runs.

The [selected source graph](README.md#machine-authorities), family capabilities, compare facts,
and per-family baseline manifests own current admission. Historical input names and controlled
experiments retain their original scope; they do not establish support for a later Mermaid
release. Families without a special note use the shared contracts and their executable evidence.

## Architecture

Compound layout is a source-backed headless approximation of Cytoscape/FCoSE. Service body/label
unions, group expansion, operation randomness, and host text measurements can all affect geometry;
a seed makes a reference reproducible without proving pixel identity with an arbitrary browser run.
Architecture labels lack shared `data-id` attributes, so semantic-label admission uses the direct
owning edge group and path, rejects ambiguous repeated text, and requires exact label geometry.

`normalized` in an Architecture fixture name records import provenance, not a separate grammar
mode or a parser-only exemption. Historical Mermaid 11.12.3 CLI rejections do not classify the
current corpus. The [input sources](../../fixtures/architecture/) and
[baseline manifest](../../fixtures/upstream-svgs/architecture/_baseline-manifest.json) own current
input bytes, inclusion, and provenance; do not recreate an exclusion from the suffix.

Owners: [Architecture layout](../../crates/merman-render/src/architecture.rs),
[layout tests](../../crates/merman-render/tests/architecture_layout_test.rs),
[SVG tests](../../crates/merman-render/tests/architecture_svg_test.rs), and
[semantic-label admission](SEMANTIC_LABEL_PARITY.md).

## C4

Quoted macro values use ECMAScript whitespace trimming for semantics while editor facts retain
their authored ranges. Physical newlines are admitted inside quoted tokens; formatting an entire
macro across lines outside such a token remains invalid. Explicit source line breaks are resolved
before wrapping; decoded final rows do not reinterpret literal entity-encoded `<br>` text.

Mermaid's screen-derived width limit needs an explicit host input in a headless operation.
`LayoutOptions::screen_available_width` projects browser `screen.availWidth`; when absent, C4 uses
the operation's container width. Text wrapping can consequently differ between hosts. Relationship
offsets belong to their ordered relation and named field. Browser text differences do not excuse
incorrect shape intersections or swapped offsets.

Owners: [C4 layout](../../crates/merman-render/src/c4.rs),
[relation geometry tests](../../crates/merman-render/tests/c4_relation_geometry_test.rs), and
[public typed-render tests](../../crates/merman/tests/c4_typed_render.rs).

## Class

Final HTML title bounds use the displayed XHTML font, including the title weight, separately from
the wrapping-budget probe. Text measurements can change ELK placement and terminal bends. The
[Class residual contract](SEMANTIC_LABEL_PARITY.md#class-elk-measurement-residuals) explains the
required identities, node-side attachment, presentation, and exact signed geometry; it provides
the maintained rationale for the canary's residual entries.

Generic parameters are authored type text, including the value `group`; they do not select
namespace sizing. Quoted `cssClass` target lists retain each comma-separated fragment's whitespace
for lookup, as the selected DB does. Use `"A,B"` to target both declared classes A and B;
`"A, B"` does not match B through its space-prefixed second fragment.

Owners: [HTML typography tests](../../crates/merman-render/tests/class_html_typography_test.rs) and
[SVG/layout integration](../../crates/merman-render/tests/class_svg_test.rs).

## ER

An explicit `layout: swimlane` selects the bundled provider directly from ER's measured entities,
relationships and compound memberships. Its generated relationship-label nodes use non-Markdown text,
the Flowchart wrapping width and root `htmlLabels`; lane titles separately follow
`flowchart.htmlLabels`. Layout owns the prepared label terminals and lane paint geometry, so SVG
does not parse or lay out the graph again. Sanitized HTML font sizes participate in label measurement
and output while preserving the configured wrapping width. Class and State retain their own provider
selection.

Font shaping can select different wrapped rows in browser and headless output; each local row
reserves its resolved font height. Inline icons retain the shared advance estimates, with the styled
text viewport as a lower bound. This does not establish exact combined icon/text bounds or loaded
icon assets.

The selected upstream Swimlane provider fails on ER self-loops. Merman reports this boundary as
an explicit model error instead of emitting a successful diagram with missing relationships.
Use Dagre or ELK when ER relationships include self-loops. Fonts, browser bounds and hand-drawn
primitives retain the shared artifact limits; this backend does not establish pixel identity.

Owners: [ER provider adapter](../../crates/merman-render/src/er/swimlane.rs),
[shared measured graph](../../crates/merman-render/src/swimlane/flat.rs), and
[ER Swimlane tests](../../crates/merman-render/tests/er_swimlane_test.rs).

## Error

Suppressed parse failures can produce the typed Error artifact. A State input stored in
`fixtures/error/` tests that failure path; it does not establish support for the rejected State
syntax. Version labels follow the selected runtime identity.

Owners: [Error fixtures](../../fixtures/error/),
[compare policy](../../crates/xtask/src/cmd/compare/diagrams.rs), and
[Info/Error SVG tests](../../crates/merman-render/tests/info_error_svg_test.rs).

## Event Modeling

`entity`, `note`, and `gwt` remain in canonical semantics, reference validation, and editor facts.
The render projection omits them because the selected Mermaid DB/renderer consumes frames and data
entities instead of those AST collections. Do not invent visible boxes for grammar-only constructs.
Data type names do not select distinct HTML/Markdown renderers. Local data labels emit escaped
text; browser HTML injection, non-breaking-space serialization, and measured wrapping retain a
bounded compatibility difference.

Owners: [semantic/editor contract tests](../../crates/merman-core/tests/eventmodeling_contract.rs),
[SVG emitter](../../crates/merman-render/src/svg/parity/eventmodeling.rs), and
[SVG tests](../../crates/merman-render/tests/eventmodeling_svg_test.rs).

## Flowchart

Dagre and ELK are separate layout paths selected through the
[configuration contract](CONFIG_FRONTMATTER_SUPPORT.md). The source-backed ELK probe lane checks
the Mermaid-reachable adapter surface; it is not a promise to implement every Eclipse ELK option.
Stable source edge identity must survive import, routing, label projection, and SVG emission.

Accessibility directives are recognized at the authoritative lexer's plain cursor. The same text
inside quoted, Markdown or other label payloads remains label content and cannot replace the
diagram's accessible name. Asset width and height updates follow the selected DB's truthiness
before numeric conversion: numeric zero preserves an earlier size, while string `"0"` is explicit.

Controlled measurement evidence distinguishes three boundaries: dimensions supplied to a provider,
raw provider output, and the final clipped/straightened/painted route. The
[browser-dimension replay](../../crates/merman-render/src/flowchart/elk/browser_measurements_tests.rs)
replaces invocation dimensions and executes the Rust provider. The
[terminal writer tests](../../crates/merman-render/src/svg/parity/flowchart/browser_measurement_tests.rs)
also consume captured placement/routes and independently collected terminal primitives. Passing
the writer replay alone does not prove a complete browser-measured Rust layout pipeline.

The retained `local_mermaid_12_2_elk_label_clearance_8368` input illustrates why those boundaries
matter: controlled measurements traced a different final bend to strict shape-boundary attachment
after provider routing. Equal raw point counts do not guarantee equal painted paths. That diagnosis
does not justify a clipping epsilon, a route rewrite, or a production fixture branch. Current
[provider captures](../../fixtures/_verification/flowchart-elk-browser-measurements.json),
[terminal measurements](../../fixtures/_verification/flowchart-svg-terminal-measurements.json), and
[browser-text receipts](../../fixtures/_verification/browser-text-layout-residuals.json) own the
signed input, artifact, source, and mode identities. Use their selected runtime/provider capture;
an older instrumented runtime or a different workspace dependency resolution is a separate oracle.

Title bounds depend on the font used by the destination surface. A host title-bbox answer can
provide actual bounds; a font-agnostic estimate cannot guarantee containment after arbitrary font
substitution. Root-paint residuals remain independently signed by the browser oracle under the
[viewport policy](PARITY_POLICY.md#root-viewport-transition).

## Gantt

The date parser implements evidenced Day.js-style tokens and JavaScript date fallback forms;
arbitrary locale formats and every `Date.parse()` dialect are not a compatibility promise.
Runtime time and timezone inputs also affect dates. A host-sensitive daylight-saving or
seconds-only test is not a portable snapshot contract.

Fractional days and weeks round before changing the local calendar date; month and year setters
truncate their target field and clamp the day. Invalid JavaScript Date results, including the
month-end calculation used by calendar setters, keep the task's start as its end. Exclusion shading
stops at the sixth local-calendar anniversary. Section names that are canonical JavaScript array
indices enumerate numerically before other names; compact row packing follows that order without
changing authored semantic order.
Custom tick intervals retain Mermaid's estimated 10,000-tick admission rule. Generated ticks,
calendar-day filtering and excluded-day inspection consume the operation work budget and report a
resource error when exhausted. Filtered day ranges admit their minimum scan work before retaining ticks.

Owners: [date parsing/formatting](../../crates/merman-core/src/diagrams/gantt/date.rs),
[date tests](../../crates/merman-core/src/diagrams/gantt/tests.rs), and
[SVG regressions](../../crates/merman-render/tests/gantt_svg_test.rs). The
[comparison runtime policy](../../crates/xtask/src/cmd/compare/diagrams/gantt.rs) owns snapshot time inputs.

## GitGraph

Commit and arrow colors use an eight-slot ring. Redux branch lines and labels separately use the
configured theme color limit; color themes reserve the main-branch slot when cycling other branches.
A branch and its commit can consequently have different color class indices. Merge tags retain their
literal text and are XML-escaped in SVG; they do not introduce active markup.

Owners: [parser](../../crates/merman-core/src/diagrams/git_graph.rs),
[color binding](../../crates/merman-render/src/gitgraph/css_binding.rs), and
[12.3 regressions](../../crates/merman-render/tests/gitgraph_mermaid_12_3_regression_test.rs).

## Ishikawa and Venn

Both have seeded headless hand-drawn rendering. Their exact registered RoughJS cases use the
structure-only boundary declared in [compare facts](../../crates/xtask/src/cmd/compare/diagrams.rs);
Rust `roughr` paths are not coordinate-identical to JavaScript RoughJS. This does not downgrade
classic SVG or grant a family-wide geometry exemption. Venn layout derives from the admitted
Venn optimization/intersection source, rather than ad hoc circle placement.

## Journey

Fractional scores are canonical values and determine face position and expression. Invalid scores
use the existing explicit invalid-score state, including during canonical serialization of direct
non-finite model values. `journey.textPlacement` selects `fo`, `old`, or SVG `tspan` paths.
Legend and inherited label typography are separate from the configured task font used by tspan
labels; theme evidence follows the visible selected path.

Owners: [Journey model](../../crates/merman-model/src/families/journey.rs),
[layout](../../crates/merman-render/src/journey.rs), and
[SVG tests](../../crates/merman-render/tests/journey_svg_test.rs).

## Requirement and Block

Requirement's root width switch follows the selected renderer's `state.useMaxWidth` owner.
Palette slots follow the border palette length; background colors wrap independently. The scalar
`THEME_COLOR_LIMIT` does not remove rules for slots stamped on Requirement nodes.
Palette CSS streams through the operation's bounded SVG writer, so repeated long color values
cannot expand into an unchecked intermediate stylesheet before byte admission.

Block's spaced class list `class A, B hot` retains the selected upstream object behavior: A is
classified, while an existing B already attached to the hierarchy remains unclassified. The raw
space-prefixed B lookup creates a replacement database binding. The flat object model retains the
original hierarchy reference and the replacement separately; compatibility `blocksFlat` projects
the latest database bindings, while drawing follows attached objects.

Owners: [Requirement config](../../crates/merman-render/src/requirement/config.rs),
[palette binding](../../crates/merman-render/src/requirement/css_binding.rs),
[Block model](../../crates/merman-model/src/families/block.rs), and
[Block SVG tests](../../crates/merman-render/tests/block_svg_test.rs).

## Sankey

Native string-keyed palette lookup deliberately avoids inherited JavaScript object properties.
The selected upstream renderer can emit invalid `[object Object]` paint for an allowed `__proto__`
node ID; Merman retains a valid palette color and does not reproduce that artifact. Visible finite
value labels use ECMAScript number formatting after the source-owned rounding step.

Owners: [palette](../../crates/merman-render/src/sankey/theme.rs) and
[SVG tests](../../crates/merman-render/tests/sankey_svg_test.rs).

## Timeline

Section suffixes are parsed as remaining source tokens; a valid event still attaches to the
previous task when the section changes. Accessibility punctuation is retained in the model.
The selected upstream family exposes those fields through its common DB without direct SVG
accessibility getters, so model parity does not imply matching upstream SVG metadata.

At fresh token boundaries, comment recognition follows the selected lexer's single-character
prefix rule, including `x%% hidden` and `:%% hidden` after a section. The prefix is one UTF-16
code unit other than `}`; an astral character or a longer prefix remains task text. Percent pairs
inside event, title, section and accessibility payloads remain literal. Skipped comments create
neither tasks nor editor payload facts, and later events still attach to the preceding task.
Header recognition retains the JavaScript word boundary, and whitespace follows ECMAScript;
U+0085 is payload text rather than ignorable indentation. Direction headers require ASCII spaces
or tabs before `LR`/`TD`; other remaining tokens resume normal parsing. A repeated header token
is rejected instead of becoming a period task.

Owners: [Timeline parser](../../crates/merman-core/src/diagrams/timeline.rs) and
[SVG tests](../../crates/merman-render/tests/timeline_svg_test.rs).

Owners: [Ishikawa SVG tests](../../crates/merman-render/tests/ishikawa_svg_test.rs),
[Venn layout](../../crates/merman-render/src/venn.rs), and
[Venn SVG tests](../../crates/merman-render/tests/venn_svg_test.rs).

## Mindmap

Rectangle, rounded and hexagon label preparation clears the requested node width and uses
`flowchart.wrappingWidth`, whose default is 120. Other shapes consume the projected Mindmap width.
Fractional padding remains part of layout and SVG geometry. Font-dependent wrapping and FCoSE
placement retain the shared browser-measurement boundary.

Owners: [layout](../../crates/merman-render/src/mindmap.rs) and
[12.3 regressions](../../crates/merman-render/tests/mindmap_mermaid12_3_test.rs).

## Railroad

`railroad`, `railroadEbnf`, `railroadAbnf`, and `railroadPeg` share typed layout and rendering.
Parsed `compactMode`, `showMarkers`, repetition separators, and maximum repetition metadata do
not gain drawing effects that the selected upstream renderer omits. Browser text-bbox differences
belong to the shared root diagnostic boundary; fixed width floors and fixture-specific roots are
not valid substitutes for measurement.

Owners: [Railroad layout](../../crates/merman-render/src/railroad.rs),
[parser tests](../../crates/merman-core/src/tests/railroad.rs), and
[SVG/grammar-combination tests](../../crates/merman-render/tests/railroad_svg_test.rs).

## Sequence and Timeline

The 12.3 reference collection retains exact browser-text snapshots where wrapping and derived
grouping differ from earlier captures. Same-environment replay of the archived 12.2 and selected
12.3 runtimes produced matching upstream outputs for the reviewed cases, as did the old/current
Rust outputs. This bounds collector-environment drift; it does not identify fonts, browser, or
locale individually. The [migration](../migrations/mermaid-12-3.md) and
[signed browser-text catalog](../../fixtures/_verification/browser-text-layout-residuals.json)
own the scope. Text, child counts, and line spacing remain observable; unregistered changes fail.

Activations follow the live layout cursor, including notes and actor creation, rather than the
previous message's arrow position. The creation/destruction canary retains its browser-measurement
residual; its exact local signature was renewed after this correction changed only the activation
rectangle's vertical position and height. Input, upstream SVG, admitted modes and normalization
remain unchanged. Its differing actor measurements and overall canvas are still bounded residuals,
not evidence of pixel-identical output.

Participant `properties.icon` fragment references follow the selected output security mode.
Strict-like SVG omits the icon nodes that Mermaid's final sanitizer removes; loose parity SVG
retains the source fragment reference. Retaining a `use` node does not prove that its symbol ID
resolves, and this contract does not establish external image loading. Special participant
`properties.class` values remain observable on their drawn bodies in either mode.

Bare Sequence participant redeclarations retain an existing display alias. Single-line accessibility
values retain hashes and semicolons through the physical line boundary; those values do not change
ordinary message or control-statement comment rules.

Sequence math comparison requires its managed math backend. An unavailable backend is a reported
tooling skip, never a comparator allowance. Owners:
[Sequence SVG tests](../../crates/merman-render/tests/sequence_svg_test.rs),
[Timeline SVG tests](../../crates/merman-render/tests/timeline_svg_test.rs), and
[browser-text comparator](../../crates/xtask/src/cmd/compare/browser_text_layout.rs).

## State

Subpixel label dimensions can change legal ELK route topology and terminal-straightening choices.
Controlled tests distinguish provider/paint behavior from measurement inputs. They do not turn
browser coordinates into production answers or excuse label identity, presentation, or DOM changes.
Inspect descendant paths, label boxes, and compound graph inputs before changing root bounds.
The retained browser-text entries admit only their exact `parity` / `parity-root` signatures;
they do not waive `structure` or `strict` comparisons. Current entries and hashes live in the
[browser-text catalog](../../fixtures/_verification/browser-text-layout-residuals.json), rather
than the old 12.0 investigation's fixture counts or successful commands.

Owners: [controlled State ELK tests](../../crates/merman-render/tests/state_elk_layout_test.rs),
[semantic-label admission](SEMANTIC_LABEL_PARITY.md), and
[State SVG diagnostics](../rendering/STATE_DEBUG_SVG.md#parity-root-viewport-debugging).

## Swimlane

Swimlane reuses Flowchart parsing and emission but owns lane preparation, layout, and routing.
Its deterministic identifier tie-break compares UTF-16 code units. Mermaid paths using
`localeCompare` may order mixed case, diacritics, or non-Latin identifiers differently according to
the browser's locale and ICU build. Attribute an observed collation difference narrowly;
lane membership, DOM ownership, and routing defects still require source-backed fixes.

The retained `_upstream_ddlt/` inputs are layout evidence outside ordinary snapshot admission.
Their finite geometry and SVG tests do not replace the semantic/layout/baseline evidence required
to promote an input. Owners: [Swimlane implementation](../../crates/merman-render/src/swimlane/mod.rs),
[layout tests](../../crates/merman-render/tests/swimlane_layout_test.rs), and
[public typed-render tests](../../crates/merman/tests/swimlane_typed_render.rs).

## TreeView

Browser text bboxes and fallback fonts, especially for emoji and non-ASCII labels, can move root
bounds. The render environment exposes `RawBBoxWidth` and `RawBBoxHeight` as separate measurement
operations. A uniform lattice correction or emoji-specific geometry would not represent those
inputs. Parser diagnostics are Rust-native rather than exact Langium messages and offsets.

Owners: [TreeView layout](../../crates/merman-render/src/tree_view.rs) and
[SVG/root tests](../../crates/merman-render/tests/tree_view_svg_test.rs).

## Wardley

Annotation sizing uses distinct text-length and bbox measurements from the operation's measurer.
Coincident link endpoints are rejected instead of emitting upstream non-finite geometry. Browser
font differences remain measurement diagnostics, not a fixture-specific layout rule.

Owners: [Wardley layout](../../crates/merman-render/src/wardley.rs) and
[SVG tests](../../crates/merman-render/tests/wardley_svg_test.rs).

## XYChart

Host measurements must use SVG/layout units. Mermaid reverses the browser CTM's horizontal and
vertical scales; headless layout has no screen transform to reverse. Applying that conversion
again would distort layout. Titles require available plot height including padding, and center on
the final plot area; a title that cannot fit vertically is omitted rather than forcibly wrapped.

The selected parser accepts orientation keywords without regard to case, but its DB selects horizontal
layout only for the exact lowercase value `horizontal`. Uppercase `HORIZONTAL` consequently remains
vertical. Statement boundaries can occur on the same physical line outside quoted and bracketed values.

Owners: [XYChart layout](../../crates/merman-render/src/xychart.rs),
[SVG tests](../../crates/merman-render/tests/xychart_svg_test.rs), and
[literal-text tests](../../crates/merman-render/tests/xychart_literal_text_test.rs).

## ZenUML

ZenUML has its own grammar, canonical semantic/editor model, typed layout, and native SVG. It is a
separate external comparison lane against the selected ZenUML Core graph, outside the built-in
Mermaid SVG matrix. Canvas/DOM shaping, icon glyph metrics, and baseline rounding remain browser
measurement boundaries. Layout formulas must trace to the selected oracle's SVG/positioning
sources; a Sequence translation or fixture-specific widths are not acceptable replacements.

Companion updates require the manual external-browser admission workflow, including security and
resource evidence. Parser acceptance alone does not admit a version. Owners:
[ZenUML parser decision](../adr/0075-zenuml-parser-technology.md),
[typed-render tests](../../crates/merman/tests/zenuml_typed_render.rs),
[SVG tests](../../crates/merman-render/tests/zenuml_svg_test.rs), and
[browser probe contract](../../tools/upstreams/ZENUML_BROWSER_ADMISSION_PROBES.json).
