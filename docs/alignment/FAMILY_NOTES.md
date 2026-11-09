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

Langium `STRING` values decode `\b`, `\f`, `\n`, `\r`, `\t`, `\v`, and `\0` as control characters;
other escaped characters lose the backslash without Unicode, hexadecimal, or octal decoding.
Editor selections retain the authored token range. This conversion does not apply to Treemap's
literal `STRING2` terminal.

## Architecture

Bracketed unquoted titles retain literal quote characters; a quote inside such a title does not
open a separate multiline string. Authored empty icon text remains present in semantics but uses
the default service background. A nonempty whitespace-only icon string still selects icon text.

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

Numeric-leading class names retain their authored semantic identity; the DOM prefix does not
rename the class or merge it with a separately declared `classId-*` name. The selected DB applies
its numeric target prefix only inside CSS-class and link assignment; tooltip and callback lookup
retain their own raw targets. Quoted note bodies preserve leading and trailing whitespace in
semantics and editor facts.

Quoted namespace IDs preserve authored whitespace, so `N` and `N ` remain distinct groups,
including their qualified descendants. Comma-separated click targets apply link, callback and tooltip
metadata to each existing class without creating missing targets. Editor references and rename ranges
track each target separately, preserving authored whitespace and generic suffixes. Ordinary SVG class labels use
the first authored `color` declaration, while HTML labels retain the CSS declaration order.

Owners: [HTML typography tests](../../crates/merman-render/tests/class_html_typography_test.rs) and
[SVG/layout integration](../../crates/merman-render/tests/class_svg_test.rs).
The [node DOM regressions](../../crates/merman-render/tests/class_mermaid12_dom_test.rs) and
[source-color tests](../../crates/merman-render/tests/class_source_color_parity_test.rs) own the
corresponding namespace, link, and output-specific color contracts.

## ER

Numeric tokens take precedence over unquoted entity text: `12abc` declares `12` before `abc`,
including when the latter owns an alias or relationship. Quote the name to retain a single
numeric-leading identity. Attribute backticks delimit a word even without surrounding whitespace.
Subgraph IDs use ECMAScript trimming in both the semantic DB and editor facts; selections retain
the canonical ID's original source range. Ordinary quoted entity names retain their padding.
Rename quotes a replacement when adjacent parser-owned entities would otherwise join into one
token, preserving the other entity and relationship endpoints. Numeric-leading replacements also
remain one entity. Existing quoted tokens keep their delimiters; style and class target lists keep
their parser-owned unquoted syntax. Editor token spans retain the authored delimiters separately
from the selected name.

Relationship statements create both endpoint entities before resolving completed subgraph IDs.
The same-name entity remains in the semantic DB, preserving later entity IDs, aliases and source
styles; the render projection filters it when the subgraph owns that ID. Style targets and
`classDef` names use their exclusive lexical context, so keyword spellings remain valid there.

Repeated style-class assignments retain their last-occurrence precedence: `red,blue,red` applies
red last. Inline source styles still override class definitions. Source text styles reach HTML
subgraph titles; ordinary SVG subgraph titles use the configured theme title role and inherited
typography, following the selected cluster painter. Ignored SVG source styles do not block typed
theme colors or fonts. Entity labels retain their own source-style behavior.

Ordinary Dagre and ELK relationship labels use `createText`'s source-owned 200px default width,
independent of `flowchart.wrappingWidth`. Prepared label rows and dimensions are shared by layout
and SVG. HTML follows the selected Markdown wrapping policy and can overflow for unbreakable words;
SVG word rows still wrap. SVG reserves the selected two-pixel background padding separately from
the centered text bounds. Dagre retains zero-size relationship label wrappers for empty roles;
ELK and Swimlane omit those empty-role terminals through their shared painter. Ordinary
whitespace-only SVG roles retain background padding without visible text. Browser fonts can change
the selected row boundaries.

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
The [endpoint and lexical-scope regressions](../../crates/merman-core/tests/er_keyword_endpoint_parity_test.rs)
and [ordinary relationship wrapping tests](../../crates/merman-render/tests/er_relationship_wrapping_test.rs)
own the corresponding semantic and terminal contracts.
The [class-order regressions](../../crates/merman-render/tests/er_class_order_parity_test.rs) and
[subgraph theme tests](../../crates/merman-render/tests/er_subgraph_theme_test.rs) own source-style
precedence and visible subgraph title ownership.

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

Hidden comments may begin immediately after scalar fields without intervening whitespace. Explicit
source references remain in authored order, including repeated occurrences, in semantics and editor
facts. Layout selects each matching source frame once in frame declaration order, following the
upstream DB's frame-membership rule.

Owners: [semantic/editor contract tests](../../crates/merman-core/tests/eventmodeling_contract.rs),
[SVG emitter](../../crates/merman-render/src/svg/parity/eventmodeling.rs), and
[SVG tests](../../crates/merman-render/tests/eventmodeling_svg_test.rs).

## Flowchart

Dagre and ELK are separate layout paths selected through the
[configuration contract](CONFIG_FRONTMATTER_SUPPORT.md). The source-backed ELK probe lane checks
the Mermaid-reachable adapter surface; it is not a promise to implement every Eclipse ELK option.
Stable source edge identity must survive import, routing, label projection, and SVG emission.

Subgraph membership for grouped chains reverses the group order while retaining authored order
within each ampersand group. First-owner filtering and dropped-member diagnostics consume that
same membership sequence; node declarations and edge expansion retain their independent source
orders. Nested and repeated subgraphs keep their existing completed-declaration ownership.

Accessibility directives are recognized at the authoritative lexer's plain cursor. The same text
inside quoted, Markdown or other label payloads remains label content and cannot replace the
diagram's accessible name. Asset width and height updates follow the selected DB's truthiness
before numeric conversion: numeric zero preserves an earlier size, while string `"0"` is explicit.
Quoted numeric dimensions accept the selected `Number(...)` string forms, including hexadecimal,
binary and octal values, and use ECMAScript whitespace. Empty arrays convert to zero; single-item
arrays follow their string conversion. Invalid truthy dimensions replace earlier values with
`NaN`; overflow and explicit infinity remain non-finite canonical values. Compatibility JSON
projects these values as `null`, as upstream JSON serialization does. Dagre, ELK and Swimlane
reject non-finite dimensions when an icon/image shape consumes them, before provider layout;
unused dimension metadata on ordinary shapes, collapsed descendants and group-replaced vertices
remains accepted. This is an explicit rendering
boundary instead of reproducing upstream SVG with invalid coordinates. Plain YAML objects with a
non-callable `toString` fail numeric conversion, including inside arrays. The shared scalar parser
also owns Journey scores; each family retains its separate authored-value and invalid-score policy.

Label, shape and visual-property metadata also retain the selected DB's authored-value truthiness:
empty strings, false and numeric zero do not replace earlier values. Default icon/image labels
are cleared only when the current update supplies that visual, not merely because an earlier
update set one. Explicitly restored labels and their text kind survive unrelated metadata updates.

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

Task-data tokens stop before a semicolon without discarding the remaining physical-line input.
For `A:a,2026-01-01,1d;B:b,after a,1d`, the second task's label is `;B`, and its ID and dependency
retain their authored offsets. Empty comma fields retain their positions when classifying explicit
task IDs for editor navigation. Exclusion validation rejects an all-weekdays mask only when a
nonempty day scan needs a working day; zero-duration milestones and shorter empty scans remain valid.
Replayed input retains the selected comment-rule priority at its cursor; literal percent markers
inside an already-started task label or task-data token remain payload.

Prefix directives also retain unconsumed same-line input and its editor ranges. Date/axis formats,
tick intervals and inclusion/exclusion lists stop before `#` or `;`; `todayMarker` stops before
`;` while retaining `#`. Day selectors and fixed flags respect the generated lexer's ASCII word
boundaries. Titles, sections and accessibility values own their full line. Merman retains its
working `topAxis` support rather than reproducing the selected runtime's missing `TopAxis()` action.
One-separator directives can consume a physical LF immediately after the keyword, retaining the
following line's authored payload. An extra inline blank or a second LF cannot be substituted for
that single separator. Day selectors, click prefixes and accessibility punctuation use their
separate repeated-whitespace rules. Click target IDs consume one whitespace character when leaving
their lexer state; the default state's bare LF ends the statement, while whitespace starting with
an inline blank can continue across LF. Public preprocessing normalizes CRLF and bare CR before these
tokens are read; editor ranges still refer to the original source. Empty closed description blocks
emit no replacement value and preserve an earlier description; missing scalar values and unclosed
blocks retain recovery diagnostics.
Quoted click values and callback arguments retain exclusive ownership through their closing
delimiter or EOF. Both successful and failed delimiter scans advance recovery from the consumed
source boundary, preventing repeated suffix scans; long payloads observe cooperative cancellation.

Structural whitespace, inclusion/exclusion token lists and task-field/tag trimming use the
ECMAScript set. BOM is whitespace, while NEL remains literal task/ID text. Click bindings use each
lexed ID without a second trim; a NEL-bearing ID therefore keeps its matching link and editor
references. Callback arguments use ECMAScript trimming outside their optional quotes. Date-parser
fallback dialects remain governed by the separate date owner.

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
Multi-step second, minute, hour and month intervals filter the base interval's local field rather
than adding a fixed multiple after the first tick. Their field resets follow D3 at enclosing-unit
boundaries. Exclusion shading retains the scan's initial time of day when matching formatted rules,
then snaps only the painted range to calendar-day boundaries.
`gantt.useMaxWidth` selects responsive or intrinsic SVG dimensions from the prepared layout size.

Owners: [date parsing/formatting](../../crates/merman-core/src/diagrams/gantt/date.rs),
[date tests](../../crates/merman-core/src/diagrams/gantt/tests.rs), and
[SVG regressions](../../crates/merman-render/tests/gantt_svg_test.rs). The
[comparison runtime policy](../../crates/xtask/src/cmd/compare/diagrams/gantt.rs) owns snapshot time inputs.

## GitGraph

Commit and arrow colors use an eight-slot ring. Redux branch lines and labels separately use the
configured theme color limit; color themes reserve the main-branch slot when cycling other branches.
A branch and its commit can consequently have different color class indices. Merge tags retain their
literal text and are XML-escaped in SVG; they do not introduce active markup.
`gitGraph.useMaxWidth` selects responsive or intrinsic SVG dimensions. The final viewport applies
`gitGraph.diagramPadding` after measuring painted elements, including rotated commit labels;
browser-dependent text bounds remain a residual rather than a fixed coordinate target.
Both padding and `gitGraph.titleTopMargin` retain their signs; invalid final viewport dimensions
still fail the shared root validation. Branch display rows follow the selected LF, literal
backslash-n and HTML-break splitting and trimming, without changing authored branch identities,
commit IDs or tags. Ordinary leftward BT fork edges retain the source-owned endpoint side and arc sweep.
Quoted GitGraph fields use the selected Langium escape conversion before model construction;
editor selections continue to address the authored escape sequences.
Quotes can separate commands or commit types from their next operand without whitespace.
Repeated scalar options keep only their winning binding occurrence in editor facts; repeated tags
remain cumulative. Merge validation follows the selected DB's head-ownership order, including its
distinct self-merge diagnostics.

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
Scores accept the selected `Number(...)` string forms, including nondecimal integer prefixes.
Score and actor trimming use ECMAScript whitespace. The first people field owns editor payloads;
later colon segments do not become actor names. Section tokens cannot silently discard following
task data; `section : 4` remains a task named `section ` rather than a section declaration.
Structural dispatch and accessibility values use the same ECMAScript whitespace set: BOM is
whitespace and NEL remains literal text. Title/section tokens consume one whitespace separator,
retaining subsequent payload whitespace; task names omit leading indentation and retain their
authored tails. Parser-derived editor facts refer to the original byte slices, including repeated
keyword text, Unicode values, CRLF and a task after a closed accessibility block. Title and section
tokens admit exactly one separator, including LF, while accessibility prefixes admit repeated
whitespace across lines. Payloads retain their own punctuation and trailing whitespace. Empty
description blocks preserve earlier text; missing scalar values and unclosed blocks reject while
retaining useful recovery facts.
At a plain lexer cursor, one BMP character other than `}` immediately followed by `%%` starts a
comment, including a literal NEL prefix. Longer labels and astral prefixes retain their percent
markers; accessibility lexer states continue owning their complete payloads.
Multiline statement ranges may enclose source comments deleted by preprocessing. Selections still
require exact mapping, so a deleted or generated identity never becomes a guessed rename target.
The existing long-actor-label browser-text receipt was renewed after task indentation was
corrected. Restoring only the old six spaces in its five HTML and five native task text nodes
reconstructs the previous full SVG signature exactly. A fresh selected-runtime browser capture
confirms the corrected task labels and the archived nine-row actor legend; deterministic
measurement still produces seven legend rows and a 0.955-pixel wider canvas. Those pre-existing
measurement differences, the input/upstream digests, modes and comparison precision remain
unchanged; the renewed receipt binds only the corrected local output.
Legend and inherited label typography are separate from the configured task font used by tspan
labels; theme evidence follows the visible selected path.

Actor identities use ECMAScript's UTF-16 lexical order. Legend enumeration separately places
canonical array-index names in numeric order, retaining each actor's original color and position;
task markers retain authored people order. Zero task width and height remain valid through
configuration, section and task geometry, face placement and activity lines.

Owners: [Journey model](../../crates/merman-model/src/families/journey.rs),
[layout](../../crates/merman-render/src/journey.rs), and
[SVG tests](../../crates/merman-render/tests/journey_svg_test.rs).

## Requirement and Block

Requirement's root width switch follows the selected renderer's `state.useMaxWidth` owner.
Palette slots follow the border palette length; background colors wrap independently. The scalar
`THEME_COLOR_LIMIT` does not remove rules for slots stamped on Requirement nodes.
Palette CSS streams through the operation's bounded SVG writer, so repeated long color values
cannot expand into an unchecked intermediate stylesheet before byte admission.

Requirement accessibility prefixes consume ECMAScript whitespace across physical lines around
their punctuation. Scalar values own the following line, including hash, semicolon and percent
text; blocks own their original source through the first closing brace. Empty closed blocks emit
no replacement value, while scalar EOF without a value and unclosed blocks reject with recovery
facts. A closed block can resume into another root statement on the same line. Structural BOM is
whitespace and NEL remains literal. Editor selections retain original byte boundaries after CRLF
normalization and comment deletion. SVG metadata consumes the model's complete accessible text
without a second trim, including NEL-only values. The lexer's title token is not an admitted Requirement grammar
statement; directives preceding the header remain subject to the public detector's separate rules.
Before a root token begins, the selected lexer's broad direction rules precede whitespace skipping
and preserve their TB/BT/RL/LR rule order. An immediately matched accessibility prefix keeps its
exclusive payload; leading whitespace can instead expose a whole-line direction token. Literal
NEL prefixes that match no such token reject before legacy definition/style helpers can publish
identity ranges. Recovery retains independent later declarations and references.
Default whitespace consumes its entire run across physical lines before the next token starts.
Direction lookahead and source resumption reuse known line boundaries, so repeated same-line
accessibility blocks do not repeatedly search the remaining line.

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

Canvas bounds include the horizontal extent of each visible label using its actual text anchor
and raw SVG text measurement. Outlined foreground and background copies occupy the same bounds;
their stroke does not add artificial padding. Host font measurements remain a bounded residual.
Unquoted CSV fields reject embedded double quotes; doubled quotes in escaped fields decode once,
and recovery retains later records with their original source ranges.

Owners: [palette](../../crates/merman-render/src/sankey/theme.rs) and
[SVG tests](../../crates/merman-render/tests/sankey_svg_test.rs).

## Timeline

Section suffixes are parsed as remaining source tokens; a valid event still attaches to the
previous task when the section changes. Accessibility punctuation is retained in the model.
The selected upstream family exposes those fields through its common DB without direct SVG
accessibility getters, so model parity does not imply matching upstream SVG metadata.

Both LR and TD render each section occurrence using all tasks with the same section text.
The model and editor facts retain each task's authored occurrence; the render index shares one
text-keyed bucket across repeated names instead of copying tasks or rescanning the whole model.

For Neo gradients, Merman retains a complete local gradient definition in both orientations.
The selected upstream TD renderer emits the shared stylesheet's gradient reference without
creating its definition. This unresolved paint reference is an intentional difference, not a
text-measurement allowance; Merman preserves its valid SVG resource and the requested styling.

At fresh token boundaries, comment recognition follows the selected lexer's single-character
prefix rule, including `x%% hidden` and `:%% hidden` after a section. A leading single `%` also
starts a comment unless immediately followed by `{`. The prefix is one UTF-16
code unit other than `}`; an astral character or a longer prefix remains task text. Percent pairs
inside event, title, section and accessibility payloads remain literal. Skipped comments create
neither tasks nor editor payload facts, and later events still attach to the preceding task.
Header recognition retains the JavaScript word boundary, and whitespace follows ECMAScript;
U+0085 is payload text rather than ignorable indentation. Direction headers require ASCII spaces
or tabs before `LR`/`TD`; other remaining tokens resume normal parsing. A repeated header token
is rejected instead of becoming a period task.

Title, section and event tokens consume one whitespace separator followed by a nonempty payload;
the separator can be LF. When a keyword does not complete its directive token, ordinary period
parsing still applies. Accessibility prefixes instead consume repeated whitespace across physical
lines and retain punctuation in their scalar or block values. Empty closed description blocks
preserve earlier text; missing scalar EOF values and unclosed blocks reject. Tokens borrow the
original parser buffer and resume at their consumed endpoint, preserving event ownership and
editor ranges through normalized line endings and removed comments.

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
Authored `section-*` class names remain literal in SVG and theme ownership; generated sections
cycle through the display policy. The ICON lexer state spans physical lines, retains nonempty raw
payloads, and treats an empty `::icon()` as no decoration. Completed icons still enforce their
statement boundary; an empty prefix emits no token, so a following adjacent decorator is retained.
Parser-derived source ranges are shared by editor facts.

Owners: [layout](../../crates/merman-render/src/mindmap.rs) and
[12.3 regressions](../../crates/merman-render/tests/mindmap_mermaid12_3_test.rs).

## Packet

Quoted labels can span physical lines; semantic text retains the newline and editor selections
retain the authored range. `packet.useMaxWidth: false` exports numeric SVG width and height from
the final viewBox. The default remains responsive.

Split fields use inclusive native `bits` counts (`end - start + 1`), including in compatibility
JSON and ASCII output. The selected Mermaid DB omits the final bit from that redundant field only
when splitting a block; its renderer still derives inclusive width from the endpoints. Merman
keeps the consistent native count rather than reproducing a value rejected by its ASCII contract.

Owners: [parser](../../crates/merman-core/src/diagrams/packet.rs),
[SVG tests](../../crates/merman-render/tests/packet_svg_test.rs), and
[ASCII split-field tests](../../crates/merman-ascii/tests/new_family_models/packet.rs).

## Pie

`pieOuterStrokeWidth` affects the prepared outer-circle radius as well as its CSS. The selected
renderer uses `parseFontSize`'s numeric component for this radius, without converting CSS units into
pixels. The default width is two and produces a radius of 186 layout units.

A left legend moves the pie by the measured legend text width plus its swatch and spacing. A title
can expand the final viewBox independently; it does not change that pie translation. Browser text
measurement can still alter the legend width and root bounds.

Owners: [layout](../../crates/merman-render/src/pie.rs),
[theme preparation](../../crates/merman-render/src/pie/theme.rs), and
[SVG tests](../../crates/merman-render/tests/pie_svg_test.rs).

## Quadrant Chart

Title and single-line accessibility terminals consume their full physical-line payload. Semicolons
and percent pairs within that payload remain literal text. Ordinary quoted and Markdown-delimited
labels can span physical lines; their backslashes remain literal under the selected Jison grammar.
Multiline tokens belong to one statement, and editor selections retain the original payload range.

Markdown delimiters select the upstream text token; they do not add rich Markdown rendering to this
family's direct SVG text terminals. Font measurement remains subject to the shared headless boundary.

Owners: [parser](../../crates/merman-core/src/diagrams/quadrant_chart.rs) and
[SVG tests](../../crates/merman-render/tests/quadrantchart_svg_test.rs).

## Radar

A numeric curve with a different number of values than axes remains in the semantic model and
legend but produces no curve geometry, as in the selected renderer. Other curves keep their
original series indices, and maximum-value scaling still considers all authored curves.

Source ticks first apply the selected DB's upper limit of 32. A positive fractional effective value
then produces `ceil(ticks)` grid lines, using that effective value as the radius denominator. For
example, `ticks 2.5` draws three lines at `1/2.5`, `2/2.5`, and `3/2.5` of the radius; the final line
extends beyond the nominal radius. Circle and polygon grids share this rule. Resource accounting
uses the actual iteration and point counts.

Owners: [layout](../../crates/merman-render/src/radar.rs),
[theme evidence](../../crates/merman-render/src/radar/theme.rs), and
[SVG tests](../../crates/merman-render/tests/radar_svg_test.rs).

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

Sequence numeric tokens require the selected lexer's ASCII space or LF right boundary; a
semicolon directly after an autonumber value does not satisfy it. EOF retains the parser's
synthetic line boundary. The same numeric-token priority applies to ordinary message endpoints:
`A->>007 : text` is rejected, while `A->>007: text` and explicit numeric participant declarations
retain the authored ID. Relative-note keywords require their ASCII word boundary, so actor names
such as `left office` and `right office` remain complete identifiers.

Completed blocks can directly precede another block, statement, branch
or closing token; ordinary line statements still require their line boundary. Grammar sources
and their checked-in LALRPOP projection change together, while editor facts use the same token tape.

Native Sequence scanning keeps LF as a statement boundary after inline blanks. The selected
Jison `\s+` rule can consume that LF and reject a following statement; this input-formatting
failure remains an intentional compatibility difference.

Control keyword polygons use Mermaid's minimum drawn width of 50, independently of the raw
`sequence.labelBoxWidth` used for frame/title layout. Typed typography can expand that width;
painting, keyword centering and effect bounds follow the actual polygon. Visible autonumber
strings use ECMAScript number formatting before selecting their length-dependent font size.

Sequence math comparison requires its managed math backend. An unavailable backend is a reported
tooling skip, never a comparator allowance. Owners:
[Sequence SVG tests](../../crates/merman-render/tests/sequence_svg_test.rs),
[Timeline SVG tests](../../crates/merman-render/tests/timeline_svg_test.rs), and
[browser-text comparator](../../crates/xtask/src/cmd/compare/browser_text_layout.rs).

## State

Root-only interaction, scale, hide and accessibility directives do not reserve ordinary state
names inside an exclusive composite body. Canonical parsing and editor facts share this lexical
scope and retain original authored ranges. `state.useMaxWidth: false` emits intrinsic root
dimensions, and choice, fork and join nodes retain safe static links and tooltip titles.

Unquoted composite IDs use their own single-token boundary and retain punctuation such as `-`
and `:`. A plain quoted alias splits its identifier and first description at colons, ignoring later
segments as the selected parser does; a composite alias retains its complete literal ID.

An empty final concurrent region is accepted by the selected parser, but its renderer fails on a
leaf `divider` shape. Merman retains its existing region partitioning for that input; no successful
upstream SVG contract is established by this parser-only case.

Subpixel label dimensions can change legal ELK route topology and terminal-straightening choices.
Controlled tests distinguish provider/paint behavior from measurement inputs. They do not turn
browser coordinates into production answers or excuse label identity, presentation, or DOM changes.
Inspect descendant paths, label boxes, and compound graph inputs before changing root bounds.
The retained browser-text entries admit only their exact `parity` / `parity-root` signatures;
they do not waive `structure` or `strict` comparisons. Current entries and hashes live in the
[browser-text catalog](../../fixtures/_verification/browser-text-layout-residuals.json), rather
than the old 12.0 investigation's fixture counts or successful commands.

Owners: [keyword scope tests](../../crates/merman-core/tests/state_keyword_scope_test.rs),
[viewport and interaction tests](../../crates/merman-render/tests/state_viewport_interaction_test.rs),
[controlled State ELK tests](../../crates/merman-render/tests/state_elk_layout_test.rs),
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

## Treemap

Quoted names use the selected `STRING2` terminal: physical newlines remain part of the name, and
backslashes are literal. The first physical line supplies the item's indentation; lines inside a
quoted name do not create additional nodes. Editor selections retain the original payload range.

Finite weights follow the selected binary64 number conversion, including leading-dot zero forms
such as `.0` and values beyond the signed integer range. Large decimal integers can round to the
same binary64 value; their authored token ranges remain distinct from the converted value.
`treemap.useMaxWidth: false` exports intrinsic SVG dimensions using the final painted bounds.
Browser-dependent text measurement can still change those bounds.

Owners: [parser](../../crates/merman-core/src/diagrams/treemap.rs),
[literal text tests](../../crates/merman-render/tests/treemap_literal_text_test.rs), and
[SVG tests](../../crates/merman-render/tests/treemap_svg_test.rs).

## TreeView

Bare names and descriptions retain literal `%%` once their payload token has begun. Quoted names
can span physical lines; the shared parser retains the label text and its authored source range
for semantic and editor consumers. Comment tokens remain hidden at grammar token boundaries.
Indentation after a completed name or annotation belongs to the following node even across hidden
comments and line breaks. A second indentation token before that name, or a dangling indentation
token at EOF, is invalid under the selected grammar.

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

Reserved lexer keywords used as band categories require quotes, such as `"bar"`. Quoted values and
ordinary identifiers retain their authored selections. Band scales use the first occurrence of
each category to build the unique coordinate domain, as D3 does; canonical categories, plot data
and repeated tick occurrences remain intact. Repeated categories therefore share a position without
compressing later unique categories. XY Chart always emits a responsive SVG in the selected upstream
renderer, including when `xyChart.useMaxWidth` is false.

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
