# Sequence ASCII Support

Status: supported subset

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
| Solid cross messages | Supported | `A-xB` and reverse direction messages render cross endpoints. |
| Dotted cross messages | Supported | `A--xB` and reverse direction messages render cross endpoints. |
| Self messages | Supported | Loop-style self calls with labels. |
| Message labels | Supported | Empty labels, single-word labels, multiword labels, and `wrap:` message labels. |
| Notes | Supported subset | `Note left of`, `Note right of`, and `Note over` notes render as boxes; `wrap:` notes wrap by display width. |
| Sequence boxes | Supported subset | Boxes render as enclosing text borders around typed actor groups. |
| Activations | Supported subset | `activate`, `deactivate`, `+`, and `-` activation state renders as active lifelines. |
| Actor create/destroy | Supported subset | Created participants render at their creating message; destroyed participants terminate with `x`/`×` and stop their lifeline. |
| Autonumber | Supported subset | Visible autonumber commands with optional start/step from the typed model. |
| Sequence control blocks | Supported subset | `loop`, `opt`, and `break` render as single-section frames; `alt`/`else`, `par`/`and`, and `critical`/`option` render as sectioned frames. |
| Character sets | Supported | ASCII and Unicode output via `AsciiRenderOptions::ascii()` and `unicode()`. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Diagram titles | `diagram titles` |
| Actor-shaped participants | `actor participant shapes` |
| Wrapped actor labels | `wrapped actor labels` |
| Actor links/properties | `actor links/properties` |
| Multiline notes | `multiline notes` |
| Wrapped boxes | `wrapped boxes` |
| Empty boxes | `empty boxes` |
| Boxes referencing unknown actors | `boxes with unknown actors` |
| Hand-built lifecycle maps referencing unknown actors | `actor lifecycle actors` |
| Hand-built lifecycle maps with out-of-range message indices | `actor lifecycle message indices` |
| Hand-built create lifecycle maps not bound to the created receiver | `actor creation messages` |
| Hand-built destroy lifecycle maps not bound to a message endpoint | `actor destruction messages` |
| Messages or notes before create or after destroy | `actor lifecycle visibility` |
| Hand-built activation flags without state events | `activations without state events` |
| Invalid activation event ordering | `activation underflow` |
| Message placement controls | `message placement` |
| Hand-built note models without ordered note messages | `notes without drawable messages` |
| Deferred sequence control blocks (`rect`, `par_over`) | `control messages` |
| Empty control block sections | `empty control block sections` |
| Nested control blocks | `nested control blocks` |
| Malformed hand-built control blocks | `control block ordering` |
| Messages referencing unknown actors | `messages with unknown actors` |
| Message types outside solid/dotted filled, solid/dotted open, solid/dotted cross, and autonumber | `message types` |

## Known Limitations

- Output comparison for copied upstream sequence fixtures follows upstream's normalized-whitespace
  comparison; trailing spaces in golden files are not product-significant.
- Wrapped actor labels and wrapped boxes remain unsupported because they require multi-line
  participant and group-box layout.
- Sequence messages and notes wrap with deterministic terminal display-width heuristics; this is a
  text rendering approximation rather than Mermaid's browser font measurement path.
- Sequence box fill colors are intentionally not represented in plain text output. Box labels render
  in the border when present.
- Mermaid `actor` shapes and actor links/properties are intentionally rejected for now because the
  current text renderer draws lifecycle-aware participant boxes, not Mermaid's richer actor shapes.
- CJK/emoji width is measured for box sizing, but full multi-cell text placement needs dedicated
  follow-up coverage before being listed as supported.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`

Golden tests compare against copied `mermaid-ascii` Unicode and ASCII sequence fixtures for the
initial supported subset.
