# Sequence ASCII Support

Status: expanding support

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
| Notes | Supported subset | Single-line `Note left of`, `Note right of`, and `Note over` notes render as boxes. |
| Sequence boxes | Supported subset | Boxes render as enclosing text borders around typed actor groups. |
| Activations | Supported subset | `activate`, `deactivate`, `+`, and `-` activation state renders as active lifelines. |
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
| Wrapped notes | `wrapped notes` |
| Multiline notes | `multiline notes` |
| Wrapped boxes | `wrapped boxes` |
| Empty boxes | `empty boxes` |
| Boxes referencing unknown actors | `boxes with unknown actors` |
| Actor create/destroy | `actor create/destroy` |
| Hand-built activation flags without state events | `activations without state events` |
| Invalid activation event ordering | `activation underflow` |
| Message placement controls | `message placement` |
| Wrapped messages | `wrapped messages` |
| Hand-built note models without ordered note messages | `notes without drawable messages` |
| Control messages with no drawable endpoints | `control messages` |
| Messages referencing unknown actors | `messages with unknown actors` |
| Message types outside solid/dotted filled, solid/dotted open, and autonumber | `message types` |

## Known Limitations

- Output comparison for copied upstream sequence fixtures follows upstream's normalized-whitespace
  comparison; trailing spaces in golden files are not product-significant.
- Rich Mermaid sequence constructs such as create/destroy and wrapping need follow-up implementation
  before they can be listed as supported.
- Sequence notes render only as single-line text for now; wrapped notes remain unsupported.
- Sequence box fill colors are intentionally not represented in plain text output. Box labels render
  in the border when present.
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
