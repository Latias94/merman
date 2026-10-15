# Mermaid Theme Style Precedence

This note records the source-backed style origins that the portable theme compiler must preserve.
The evidence is pinned to Mermaid `11.17.2` at commit
`dcb694ddb58dc5ad3502e7e903cac05fd812eac3` in `repo-ref/mermaid`.

## Configuration Origins

The effective configuration chain is:

```text
built-in defaults < initialize/site config < source frontmatter < source init directive
```

Mermaid materializes built-in theme variables in `defaultConfig.ts`, applies site configuration in
`mermaidAPI.ts`, and then overlays frontmatter and init directives in `preprocess.ts` and
`config.ts`. Multiple directives are merged in source order, so a later directive wins a
conflicting field. A source `theme` can also cause theme variables to be recomputed; a source that
only supplies `themeVariables` overlays the already selected theme instead.

The machine-readable fixture matrix admits only the source-owned visual origins that currently
have parser-backed consumers: frontmatter and init directives. Built-in and site configuration
remain part of the documented host/configuration chain, but they are not fixture entries until a
source-compatibility fixture can observe and consume them without fabricating source evidence.

`themeCSS` is different. It is one resolved string value, not a typed theme-variable origin. A
source value replaces the site value rather than concatenating it. The stylesheet is emitted after
the generated family theme CSS and before generated `classDef` rules. Generated `classDef`
declarations carry `!important`, so ordinary `themeCSS` cannot beat them through specificity alone.
When both declarations are important, specificity and the remaining cascade rules decide the
winner. Inline styles have another, family-specific precedence. Therefore raw `themeCSS` stays in
the compatibility/`Unverified` lane and is never admitted as a portable origin rank.

## CSS Emission Order

The pinned renderer assembles the style element in this order:

```text
common CSS
family theme CSS from themeVariables
common look/Neo CSS
resolved themeCSS
generated classDef CSS
```

The stylesheet is installed before the renderer starts layout and measurement. Mermaid 11.17.2
also preserves the generated root `font-family`, `font-size`, and `fill` declarations as inheritable
root rules instead of forcing another descendant namespace. That means browser CSS can affect
measurements, but native exporters do not necessarily implement the same selector or computed-style
surface. The matrix therefore separates the admitted generated-paint subset from a pre-layout
typography residual. A portable semantic plan must be resolved before layout; it must not reparse
the final SVG to infer origins.

## Family Matrices

### Flowchart

For ordinary node properties, the typed order is:

```text
theme variables
< classDef default
< classDef node
< assigned classes in assignment order
< inline node style
```

`flowDb.ts` stores class definitions, assigned classes, and inline styles separately. The shape
compiler combines class styles first and inline styles afterwards, with later values replacing the
same property. Edge labels and paths need a separate matrix: default `linkStyle` is copied first,
edge-specific `linkStyle` is appended later, and generated important class rules can still beat a
normal path declaration.

### ClassDiagram

ClassDiagram does not have the same global origin order. A `classDef` declared after a class
assignment walks nodes that already carry that class and copies the declarations into their node
style lists. The opposite order is asymmetric: a later class assignment only appends the class
name and does not copy an earlier definition into the node style list. `getClasses()` returns the
node map rather than the private class-definition map, so there is no later generated class-name
rule that repairs this ordering. `style` statements append directly to the node style list.
Consequently, classDef/assignment/inline conflicts are encounter-order semantics and must remain a
family-local adapter rule.

Class text measurement also occurs before the final style list is applied. Typography that arrives
only through class styles therefore has a source-backed layout residual unless the adapter promotes
it into the pre-layout semantic plan.

### StateDiagram

StateDiagram resolves assigned classes and then inline styles after the complete parse:

```text
theme variables < assigned classes in assignment order < inline style
```

There is no implicit `classDef default` application to every state. Repeated `style` statements
replace the existing style list for that state rather than appending each declaration.

### EntityRelationshipDiagram

ER has two distinct style planes. For paint properties, every entity starts with the implicit
`default` class, named assigned classes are compiled in stored class order, and inline declarations
are appended last:

```text
implicit default class < assigned classes in class order < inline style
```

Only that paint subset is classified as `paint-only`. Typography declarations from the same
default, assigned, and inline origins are separated into `labelStyles`, passed into `addText`, and
therefore affect `getBBox()` before node bounds and graph layout freeze. Those typography origins
retain the same order but remain explicit `Unverified` residuals until the ER adapter promotes them
into its pre-layout semantic plan. ER family CSS also supplies font family and edge-label font size;
that browser-computed stylesheet path remains a separate compatibility residual rather than being
silently treated as structured paint.

### SequenceDiagram

SequenceDiagram has no general `classDef`, `class`, or `style` grammar. Its layout-visible
typography is carried by actor/message/note font configuration, with root font fields overriding
those groups. `rect` and `box` colors are dedicated paint data, not generic theme origins.

`themeCSS` that changes sequence typography can be visible in a browser while measurement still
uses config font fields. This is an explicit `Unverified` residual, not a reason to add a broad CSS
cascade implementation to the core.

Sequence semantics are currently documentation-only. They are excluded from the machine matrix
until the fixture parser and a source-compatibility consumer for Sequence are admitted; the closed
matrix must not claim support that its fixtures cannot observe.

## Compiler Consequences

1. Resolve typed semantic patches after effective Mermaid configuration and before family layout.
2. Give Flowchart and State explicit family adapters for their source-backed origin order.
3. Preserve ClassDiagram encounter-order behavior instead of forcing it into a universal precedence
   rank.
4. Preserve ER's default/assigned/inline order separately for structured paint and pre-layout
   typography; reject or report typography until the ER adapter can measure with the resolved plan.
5. When Sequence is admitted, model its dedicated typography and paint inputs directly; do not
   invent unsupported generic class origins.
6. Keep raw `themeCSS` as a trusted compatibility lane with target-specific diagnostics.

The machine-readable mechanism and fixture contract lives in `fixtures/themes/manifest.json`.
`fixture-style-precedence` is intentionally raw-CSS-heavy and is marked `source-compatibility`; it
cannot prove a portable target capability. Typed evidence is split across
`fixture-semantic-style-capabilities` for State relational/negation rules and typography,
`fixture-class-semantic-capabilities` for Class relational rules,
`fixture-er-semantic-capabilities` for ER relational rules, and `fixture-ordinal-palette` for ordinal
palettes. Their hash-bound structured theme inputs are the mechanism and value-facet authority;
capabilities and the five output contracts are derived from those inputs and the translation table
rather than repeated in expectation JSON.
The source-compatibility fixtures cover visual frontmatter and init configuration, Flowchart node
and edge ordering, both ClassDiagram encounter-order directions plus inline and typography
residuals, State theme/assigned/inline order, and ER default/assigned/inline paint plus typography
residual order. Every
machine-matrix entry has at least one fixture consumer, and every hashed Mermaid evidence file is
cited by at least one matrix entry. None of these fixtures invents animation or standalone CSS
transform behavior absent from the pinned reference source.

## Pinned Evidence Index

All paths below are relative to `repo-ref/mermaid` at the pinned commit.

| Contract | Source evidence |
| --- | --- |
| Built-in, site, frontmatter, and directive config order | `packages/mermaid/src/defaultConfig.ts:18-34`; `packages/mermaid/src/mermaidAPI.ts:72-76,663-686`; `packages/mermaid/src/preprocess.ts:19-63`; `packages/mermaid/src/config.ts:29-47`; `packages/mermaid/src/mermaidAPI.spec.ts:804-824` |
| `themeCSS`, generated root typography, and generated `classDef` CSS order | `packages/mermaid/src/mermaidAPI.ts:105-201,219-326,572-588`; `packages/mermaid/src/styles.ts:34-46,115-163` |
| Flowchart default/node/assigned/inline node styles | `packages/mermaid/src/diagrams/flowchart/parser/flow.jison:533-570`; `packages/mermaid/src/diagrams/flowchart/flowDb.ts:216-224,415-438,471-485,1026-1093`; `packages/mermaid/src/rendering-util/rendering-elements/handDrawnShapeStyles.ts:31-103` |
| Flowchart default and edge-specific link styles | `packages/mermaid/src/diagrams/flowchart/flowDb.ts:1224-1274`; `packages/mermaid/src/rendering-util/rendering-elements/edges.js:64-120,773-829` |
| ClassDiagram asymmetric encounter-order copying and late style application | `packages/mermaid/src/diagrams/class/classDb.ts:122-134,192-194,331-367,684-695`; `packages/mermaid/src/diagrams/class/classRenderer-v3-unified.ts:34-39`; `packages/mermaid/src/diagrams/class/shapeUtil.ts:14-27`; `packages/mermaid/src/rendering-util/rendering-elements/shapes/classBox.ts:13-33,236-267` |
| State assigned-class and inline order, without implicit default | `packages/mermaid/src/diagrams/state/parser/stateDiagram.jison:291-345`; `packages/mermaid/src/diagrams/state/stateDb.ts:293-306,624-644,685-694`; `packages/mermaid/src/diagrams/state/dataFetcher.ts:145-227,279-296`; `packages/mermaid/src/diagrams/state/stateCommon.ts:47-59` |
| ER implicit default, assigned-class order, inline paint, and pre-layout typography residuals | `packages/mermaid/src/diagrams/er/parser/erDiagram.jison:121-187,229-251`; `packages/mermaid/src/diagrams/er/erDb.ts:61-72,168-227,303-317,377-427`; `packages/mermaid/src/diagrams/er/styles.ts:42-49,74-96`; `packages/mermaid/src/rendering-util/rendering-elements/shapes/erBox.ts:25-27,48-50,92-127,318-333,343-398`; `packages/mermaid/src/rendering-util/rendering-elements/shapes/handDrawnShapeStyles.ts:31-103` |
| Sequence typography and dedicated rect/box paint channels | `packages/mermaid/src/diagrams/sequence/sequenceDiagram.ts:10-16`; `packages/mermaid/src/diagrams/sequence/sequenceRenderer.ts:375-413,469-479,875-887,1181-1201,2144-2150`; `packages/mermaid/src/diagrams/sequence/sequenceDb.ts:145-152,391-418`; `packages/mermaid/src/diagrams/sequence/svgDraw.js:379-410`; `packages/mermaid/src/diagrams/common/svgDrawCommon.ts:54-65` |
