# Sequence ASCII Support

Status: supported subset

This document describes the current `merman-ascii` sequence support boundary. The renderer consumes
`merman-core` `SequenceDiagramRenderModel` values; it does not parse Mermaid text itself.

## Supported

| Capability | Status | Notes |
| --- | --- | --- |
| Diagram family | Supported subset | `sequenceDiagram` inputs that parse into `SequenceDiagramRenderModel`. |
| Diagram titles | Supported subset | `title`/`title:` render as a centered text row above the participant boxes. |
| Participants | Supported | Participant order follows `actorOrder`; labels use actor descriptions. `participant`, `actor`, `boundary`, `control`, `entity`, `database`, `collections`, and `queue` declarations render as terminal participant boxes. Mermaid-valid identifiers with spaces, dashes, equals signs, and Unicode are retained; reference-private quoted and spaced-alias extensions are not admitted. |
| Participant boxes | Supported | ASCII and Unicode box drawing with centered labels, wrapped actor labels, and HTML/Markdown break labels. Mermaid-compatible output renders top participant boxes only by default; `AsciiRenderOptions::with_sequence_mirror_actors(true)` also renders bottom participant boxes. |
| Lifelines | Supported | One lifeline row before each message and one trailing lifeline row. |
| Solid filled messages | Supported | `A->>B` and reverse direction messages. |
| Dotted filled messages | Supported | `A-->>B` and reverse direction messages. |
| Solid open messages | Supported | Mermaid `A->B` signals render as solid headless lines. |
| Dotted open messages | Supported | Mermaid `A-->B` signals render as dotted headless lines. |
| Solid cross messages | Supported | `A-xB` and reverse direction messages render cross endpoints. |
| Dotted cross messages | Supported | `A--xB` and reverse direction messages render cross endpoints. |
| Point and bidirectional messages | Supported | Point endpoints and bidirectional filled endpoints retain their typed source/target ownership. |
| Half-arrow messages | Supported | All solid/dotted, forward/reverse, top/bottom filled and open half-arrow variants render distinctly. Unicode uses native marker glyphs; 7-bit ASCII adds a lineward `|` stem to filled halves. |
| Central connection decorations | Supported | Source, target, and dual `()` decorations render independently from endpoint markers. |
| Self messages | Supported | Loop-style self calls with labels. |
| Message labels | Supported | Empty labels, single-word labels, multiword labels, and `wrap:` message labels. |
| Notes | Supported subset | `Note left of`, `Note right of`, and `Note over` notes render as boxes; multiline note text and `wrap:` notes wrap by display width. |
| Sequence boxes | Supported subset | Boxes render as enclosing text borders around typed actor groups; wrapped and multiline box labels render as additional label rows. Boxes with no actor anchors render as diagram-wide terminal regions instead of inventing hidden participants. |
| Activations | Supported subset | `activate`, `deactivate`, `+`, and `-` activation state renders as active lifelines. Activation records follow Mermaid parser ordering independently from actor visibility, so a deactivation after a destroying signal can close an activation that began before destruction. |
| Actor create/destroy | Supported subset | Created participants render at their creating message; destroyed participants terminate with `x`/`×` and stop their lifeline. Parser-backed models carry the signal that actually consumed each pending lifecycle request, so ASCII and SVG share Mermaid's create-before-destroy and last-declaration-wins ordering. Legacy direct models without that sidecar fall back to the compatibility lifecycle maps. |
| Autonumber | Supported subset | Visible autonumber commands with optional start/step from the typed model. |
| Sequence control blocks | Supported subset | `loop`, `opt`, `break`, `rect`, and `par_over` render as single-section frames; `alt`/`else`, `par`/`and`, and `critical`/`option` render as sectioned frames. Frames derive their horizontal bounds from the participants used by their descendant messages, notes, and activation directives, while unrelated lifelines remain outside. Nested frames keep stable insets and empty sections fall back to the full participant span. |
| Control-block combinations | Supported subset | Notes, activations, create/destroy lifecycle rows, and participant boxes are covered with control-block frames. |
| Character sets | Supported | ASCII and Unicode output via `AsciiRenderOptions::ascii()` and `unicode()`. |
| ANSI/HTML color roles | Supported subset | Opt-in `AsciiColorMode` can emit foreground roles for participants, lifelines, activations, messages, notes, boxes, and control frames. Mermaid `box` fill colors in supported sequence syntax (`rgb`/`rgba`/`hsl`/`hsla`/named colors) and parseable `rect` backgrounds render as terminal/HTML backgrounds when they can be represented without alpha blending. |
| Actor links and properties | Accepted, intentionally omitted | Links and presentation properties are retained by the typed model for SVG consumers but do not block terminal rendering or leak URLs/style metadata into text output. |

## Explicitly Unsupported

These features return `AsciiError::UnsupportedFeature` instead of silently dropping semantics:

| Feature | Error feature |
| --- | --- |
| Unknown actor types | `actor types` |
| Boxes referencing unknown actors | `boxes with unknown actors` |
| Hand-built lifecycle maps referencing unknown actors | `actor lifecycle actors` |
| Hand-built lifecycle maps with out-of-range message indices | `actor lifecycle message indices` |
| Hand-built create lifecycle maps not bound to the created receiver | `actor creation messages` |
| Hand-built destroy lifecycle maps not bound to a message endpoint | `actor destruction messages` |
| Messages before create or after destroy | `actor lifecycle visibility` |
| Nonempty hand-built actor orders that omit, duplicate, or reference unknown actors | `actor order` |
| Hand-built activation flags without the matching target state event | `activation state events` |
| Invalid activation event ordering | `activation underflow` |
| Message placement controls | `message placement` |
| Provided hand-built note facts that disagree with their ordered note messages | `note model consistency` |
| Message payload variants that disagree with the typed message kind | `message payload shape` |
| Structural message records containing endpoints, text, or flags that their kind cannot consume | `message record shape` |
| Non-finite autonumber start or step values | `autonumber values` |
| Orphaned, missing, reordered, or actor-mismatched central-connection records | `central connection records` |
| Malformed hand-built control blocks | `control block ordering` |
| Messages referencing unknown actors | `messages with unknown actors` |
| Message types outside the typed signal, control, lifecycle, note, and autonumber model | `message types` |

## Known Limitations

- Direct typed models may leave the compatibility `actorOrder` vector empty; participants then follow the
  deterministic ordered actor map. They may also omit the duplicate `notes` vector; ordered Note
  message records then remain the drawable source of truth. When either compatibility projection is
  supplied, it must agree exactly with the underlying actor or message facts.
- Output comparison for copied upstream sequence fixtures follows upstream's normalized-whitespace
  comparison; trailing spaces in golden files are not product-significant.
- Diagram titles render as terminal text above the participant row; accessibility titles remain
  metadata and are not rendered in the text diagram.
- Mermaid 11.15 defaults `sequence.mirrorActors` to `false`; bottom participant boxes are therefore
  opt-in in `merman-ascii` instead of part of the default golden fixture contract.
- Sequence messages and notes wrap with deterministic terminal display-width heuristics; this is a
  text rendering approximation rather than Mermaid's browser font measurement path.
- Empty sequence boxes render as diagram-wide terminal regions because the typed model has no actor
  anchors to constrain the box horizontally.
- Sequence box fill colors render as terminal/HTML backgrounds only when Mermaid supplies a color
  the terminal can represent faithfully. Mermaid sequence syntax does not preserve `#hex` box/rect
  colors because `#` is handled as a comment marker upstream.
- Mermaid `rect` style/color expressions render as frame backgrounds when the value is a parseable
  terminal color. Browser-only transparency/alpha forms stay visible as labels rather than being
  approximated.
- Mermaid actor declarations and extended actor types render as terminal participant boxes instead
  of SVG-specific actor shapes. Actor links and presentation properties are accepted as SVG
  metadata and intentionally omitted from ASCII output.
- CJK/emoji width is measured for box sizing, but full multi-cell text placement needs dedicated
  follow-up coverage before being listed as supported.

## Test Coverage

The support boundary is covered by:

- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`

Golden tests compare against copied `mermaid-ascii` Unicode and ASCII sequence fixtures for the
initial supported subset.
