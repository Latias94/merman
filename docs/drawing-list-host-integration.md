# DrawingList host-integration contract

This note records the renderer requirements raised in issue #114 and the boundary of the
current `DrawingList` implementation. It is deliberately written for host-renderer authors:
Compose Multiplatform, Flutter, Canvas, Skia-like renderers, and other consumers that want to
paint a Mermaid diagram without embedding an SVG interpreter.

## What the list is (and is not)

`DrawingList` is an ordered, retained paint instruction stream produced after Mermaid semantic
analysis and layout. A family owns its geometry and resolved style; SVG is one serializer of the
canonical render document. The list is not an editable scene graph and it does not ask a host to
choose node positions, route edges, or redo Mermaid layout. An Excalidraw-like editor can consume
the list as an import or display snapshot, but editing and hit-testing APIs are outside this
contract.

The intended host flow is:

1. discover `drawing-list-json` and validate the advertised protocol version;
2. decode the self-contained resource tables and ordered commands;
3. replay commands with a balanced save/restore and layer/group stack;
4. use semantic groups for hit-testing, tooltips, accessibility, and navigation metadata;
5. reject an unmet text/effect obligation instead of substituting a different visual silently.

A `DrawPath` with both `fill: null` and `stroke: null` retains its geometry and semantic
ownership but produces no paint. Hosts must not substitute a default fill or stroke. Empty
`DrawText` commands likewise preserve their semantic position without painting glyphs.

Semantic descriptions carry source or explicitly authored accessibility metadata. They are not a
serialization of every family model relationship. In particular, Gantt does not synthesize English
`"<section> section"` descriptions for tasks: the section's text, background, and placement remain
in the public drawing, while machine-readable task-to-section membership remains in the typed
semantic/layout artifacts. Hosts must not infer a relationship by parsing description prose. A
future portable relationship API would need an explicit versioned contract.

Before bytes are exposed, the renderer computes a document-wide footprint (commands, resources,
path segments, stroke dash entries, text/glyph work, inline assets, pixels, and maximum state
nesting) and charges it to the existing operation work budget. Protocol limits still validate the
exact document counts and serialized byte ceiling; the footprint is an additional
admission/accounting signal, not a second wire contract.

## SVG diagnostic associations

Stable DrawingList identity, text bounds, and shaping obligations live in the public document.
Production SVG does not duplicate them as `data-merman-resource`, `data-merman-semantic-id`,
`data-merman-bounds`, or `data-merman-text-obligation` attributes. Rust diagnostic callers may set
`SvgDebugOptions.include_drawing_list_metadata = true` to request those associations on projected
SVG elements. Compact projections may merge commands, so this is not a complete reverse mapping
and consumers must not reconstruct the public document from these attributes.

The switch changes diagnostic attributes only. Source DOM IDs/classes, paint/resource references,
ARIA, descriptions, links, and functional markers such as `data-merman-native-text` remain intact.
Use the public semantic/resource tables for host integration and source DOM IDs for SVG interaction.

## Requirements mapped to the current contract

| Host requirement | Current v1 representation | Alpha status |
| --- | --- | --- |
| Host-shaped text | `TextRun` carries the ordered font-family list, size, weight, style, fill, optional stroke, fill/stroke paint order, anchor, baseline, direction, language, reserved `bounds`, and measurement provenance. | Supported for plain host-text runs. The host must honor the declared obligation and should compare its measured bounds with `bounds`. |
| Explicit line placement | `DrawText` has one origin/bounds pair and a line height. Several adapters already emit one command per line, but v1 still permits a newline inside `text`. | Partial. Do not infer independent line origins from a newline; a future protocol revision should make a text block/line sequence normative. |
| Stable element identity | `BeginSemanticGroup`/`EndSemanticGroup` wrap ordered visual commands; `SemanticAnnotation` carries a stable id, role, title, description, and optional link. | Supported for document/node/edge/group/label identity. |
| Navigation and tooltip metadata | Links are data on semantic annotations. Family adapters apply the shared Mermaid navigation security policy before storing the logical URI; SVG serialization escapes that already-clean logical value at the attribute boundary. | Supported for one link per semantic annotation. Multiple links are not silently flattened; affected input fails closed until an ordered link collection is versioned. |
| Marker geometry | Arrowheads and other Mermaid markers are expanded into ordinary path resources. | Supported without host marker primitives. |
| Gradients and patterns | Linear/radial gradients expose stops, spread, transform, and deterministic resource ids; patterns reference an image resource and tile geometry. | Supported where the family adapter admits the effect. |
| Images | Image bytes are inline in a bounded resource table; no URL is an implicit fetch instruction. v1 admits complete, non-animated PNG payloads and verifies their dimensions and alpha metadata before publication. | Supported for static PNG. Fallback alpha is `opaque` for RGB data and `straight` for PNG data with transparency. Hosts must not perform ambient I/O while decoding. Additional formats require separate protocol admission backed by a complete decoder. |
| Clip paths | `ClipPath` references a path resource and an explicit fill rule. | Supported. |
| Blend mode | `SetBlendMode` and layer blend state carry Sankey/Treemap-style compositing intent. | Supported when the host has the named blend mode; otherwise the host must reject or use an explicitly negotiated fallback. |
| Group opacity | `BeginLayer`/`EndLayer` paint children as one composited group; Gantt axis ticks use this for their line and label. Layer bounds are in the local user space under the current transform at `BeginLayer`; opening a layer does not reset graphics state. | Hosts must composite the group once. Multiplying each child's opacity is not equivalent when child paint overlaps. |
| Root background | Most current family adapters paint their background as the first ordered path, preserving paint order but not exposing root-background semantics as a separate field. | Known gap. Do not guess that the first path is a background. A future protocol revision should add an authoritative root background/clear field and remove duplicate root paints. |
| Filters and drop shadows | There is no generic filter graph in v1. A non-portable effect must become a bounded `DrawRasterSubtree` with provenance or a structured unavailable error. | Supported only through explicit fallback/error; never approximate by dropping the effect. |
| Math | v1 has `GlyphRun`, `Outline`, and raster obligations, but the current Mermaid math path is HTML/KaTeX-oriented and does not yet splice a RaTeX `DisplayList` into the public document. | Known gap. Current correct behavior is fail-closed. The next implementation should translate RaTeX display items directly to paths/glyph resources, not parse generated SVG or require host KaTeX fonts. |
| SVG output | `RenderDocument` owns the public list and an SVG-only structural sidecar. The generic SVG encoder is admitted family-by-family while source-backed parity bridges remain. | Migration in progress. The sidecar is not a second public geometry source. |

## Text policy

The default host contract is host-shaped text, not universal outline conversion. Merman's layout
already depends on the configured text-measurement service, so replacing every label with outlines
would discard the host measurement contract and would make ordinary labels unnecessarily heavy.

The obligation on each text command is authoritative:

- `HostText` means the host may shape the run only with a compatible text capability and should
  verify the reserved bounds;
- `GlyphRun` means positioned glyph data is available and must not be substituted with a host
  string run;
- `Outline` means the referenced path is the portable glyph/label geometry;
- `RasterFallback` means the fallback record, bounds, pixels, scale, format, and source identity
  are part of the result.

Journey titles resolve parent-relative `em`, `%`, and `ex` against the configured root SVG font
size, independently of task-label font size. The deterministic text profile approximates `ex` as
half an em and resolves `rem` against a 16px host-document baseline; it does not read ambient
browser CSS or claim exact font x-height metrics. Both public text and SVG carry the resulting px
size. Hosts should consume that value rather than evaluate the authored CSS unit again.

Rich HTML/Markdown labels that cannot be represented as one honest run remain structured failures
or explicit raster subtrees. They are never reduced to plain text merely to make a host renderer
continue.

## Math policy

RaTeX is the right source-level integration point because its `DisplayList` is already a flat,
renderer-neutral list. The intended adapter is:

```text
RaTeX DisplayItem
  ├─ GlyphPath / Path  -> DrawingList path resource + outline command
  ├─ Line              -> stroked path command
  └─ Rect              -> filled path command
```

The adapter must preserve the formula's scale, baseline, color, and reserved label bounds. It
must not emit KaTeX family names and assume that Android, iOS, desktop, or web hosts have the same
KaTeX fonts. If a glyph cannot be converted to a portable outline/resource within the operation
budget, the operation returns a documented fallback/error rather than the source formula as
ordinary text.

## Background and protocol evolution

An explicit root background is useful for native compositing, but adding an optional field while
leaving the existing background path commands in place would create two sources of truth. The
correct change is therefore a protocol revision with one authoritative rule:

1. the document declares the root clear/background paint (or transparent);
2. the serializer/host applies it exactly once before the command stream;
3. family-local interior background shapes remain ordinary commands;
4. old documents are not reinterpreted by guessing from command order.

The same versioning discipline applies to explicit per-line text records and ordered multi-link
metadata. These are protocol changes, not private `x-*` extensions, because they affect layout or
interaction semantics.

## Release-facing statement

For the current alpha, it is accurate to say that plain, fully admitted diagrams can be consumed
by a native host from a self-contained renderer-neutral list: paths, text style/measurement
metadata, semantic groups, markers, gradients, images, clips, blend intent, and bounded fallback
provenance are available. It is not accurate to claim complete native math, generic filter
support, authoritative root-background semantics, arbitrary rich text, or multi-link semantics
until the protocol and family coverage explicitly provide them.

This boundary is intentional: a host can rely on the data it receives, and a missing capability is
visible as a structured result instead of a visually incomplete document.
