# Sequence ASCII Support

Status: initial tracer-bullet support

This document describes the current `merman-ascii` sequence support boundary. The renderer consumes
`merman-core` `SequenceDiagramRenderModel` values; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported subset | `sequenceDiagram` inputs that parse into `SequenceDiagramRenderModel`. |
| Participants | Supported | Participant order follows `actorOrder`; labels use actor descriptions. |
| Participant boxes | Supported | ASCII and Unicode box drawing with centered labels. |
| Lifelines | Supported | One lifeline row before each message and one trailing lifeline row. |
| Solid filled messages | Supported | `A->>B` and reverse direction messages. |
| Dotted filled messages | Supported | `A-->>B` and reverse direction messages. |
| Solid open messages | Supported | `A->B` and reverse direction messages. Unicode output uses open arrowheads. |
| Dotted open messages | Supported | `A-->B` and reverse direction messages. Unicode output uses open arrowheads. |
| Self messages | Supported | Loop-style self calls with labels. |
| Message labels | Supported | Empty labels, single-word labels, and multiword labels. |
| Autonumber | Supported subset | Visible autonumber commands with optional start/step from the typed model. |
| Character sets | Supported | ASCII and Unicode output via `AsciiRenderOptions::ascii()` and `unicode()`. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Diagram titles | `diagram titles` |
| Actor-shaped participants | `actor participant shapes` |
| Wrapped actor labels | `wrapped actor labels` |
| Actor links/properties | `actor links/properties` |
| Notes | `notes` |
| Sequence boxes | `boxes` |
| Actor create/destroy | `actor create/destroy` |
| Activations | `activations` |
| Message placement controls | `message placement` |
| Wrapped messages | `wrapped messages` |
| Control messages with no drawable endpoints | `control messages` |
| Messages referencing unknown actors | `messages with unknown actors` |
| Message types outside solid/dotted filled, solid/dotted open, and autonumber | `message types` |

## Known Limitations

- Output comparison for copied upstream sequence fixtures follows upstream's normalized-whitespace
  comparison; trailing spaces in golden files are not product-significant.
- Rich Mermaid sequence constructs such as notes, boxes, activations, create/destroy, and wrapping
  need follow-up implementation before they can be listed as supported.
- Mermaid `actor` shapes and actor links/properties are intentionally rejected for now because the
  initial port renders only participant boxes and plain text.
- CJK/emoji width is measured for box sizing, but full multi-cell text placement needs dedicated
  follow-up coverage before being listed as supported.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`

Golden tests compare against copied `mermaid-ascii` Unicode and ASCII sequence fixtures for the
initial supported subset.
