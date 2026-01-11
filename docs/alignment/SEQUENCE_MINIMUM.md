# Sequence Diagram Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for `sequenceDiagram` parsing in
`merman`.

## Baseline

Upstream baseline: `mermaid@11.12.2` (see `docs/adr/0001-upstream-baseline.md`).

## Supported (current)

- Header: `sequenceDiagram`
- Statement separators: newline and `;`
- Comments:
  - `%%` line / inline comments
  - `#` line / inline comments
- Participants:
  - implicit participants via usage in signals/notes (e.g. `Alice->Bob:Hello`)
  - explicit declarations:
    - `participant <id>`
    - `participant <id> as <display>`
    - extended syntax: `participant <id>@{ ... }` (YAML/JSON-ish metadata; `type` overrides the participant type)
    - `actor <id>`
    - `actor <id> as <display>`
  - participant-level wrap via `wrap:` / `nowrap:` prefixes in the `as <display>` text
- Signals (message lines):
  - arrows (subset): `->`, `-->`, `->>`, `-->>`, `-x`, `--x`, `-)`, `--)`, `<<->>`, `<<-->>`
  - activation syntax (subset): `+` / `-` after the arrow token
  - message text via `:<text>` (with optional `wrap:` / `nowrap:` prefixes)
- Notes:
  - `Note left of <actor>: <text>`
  - `Note right of <actor>: <text>`
  - `Note over <actor>: <text>` (single actor coerced to `[actor, actor]` like Mermaid)
  - `Note over <actor1>,<actor2>: <text>`
  - notes are represented both as a `notes[]` entry and as a `messages[]` entry with
    `type = LINETYPE.NOTE`
- Metadata:
  - `title: ...` and `title ...`
  - `accTitle: ...`
  - `accDescr: ...` and multiline `accDescr{ ... }`
- Wrap configuration:
  - directive `%%{wrap}%%` sets the default wrap behavior for signals/notes, matching Mermaid's
    `wrap -> sequence.wrap` wiring (parsing-only view)
- Actor metadata statements:
  - `links <actor>: { ... }` (JSON object merged into `actor.links`)
  - `link <actor>: <label> @ <url>` (merged into `actor.links`)
  - `properties <actor>: { ... }` (JSON object merged into `actor.properties`)
  - `details <actor>: <id>` (parsed; headless behavior TBD)
- Boxes:
  - `box <color?> <title?> ... end`
  - `fill` color is validated against CSS color keywords and `rgb()/rgba()/hsl()/hsla()` forms
  - `actorKeys` ordering matches Mermaid insertion order within the box
- Create / destroy:
  - `create participant <id>` / `create actor <id> as <display>` (subset)
  - `destroy <id>`
  - `createdActors` / `destroyedActors` are recorded as message indices, matching Mermaid `SequenceDB`
  - semantic checks (Mermaid-compatible):
    - a created participant must have a following message targeting it
    - a destroyed participant must have a following message involving it (from or to)
- Control blocks:
  - `loop <text?> ... end` (start/end control messages)
  - `opt <text?> ... end`
  - `alt <text?> ... else <text?> ... end` (multiple `else` branches supported)
  - `par <text?> ... and <text?> ... end`
  - `par_over <text?> ... end`
  - `critical <text?> ... option <text?> ... end`
  - `break <text?> ... end`

## Not yet implemented (Mermaid-supported)

- Full error surface parity (token/loc/expected) with Mermaid Jison errors.

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `sequenceDiagram` grammar and
behavior compatibility at the pinned baseline tag.
