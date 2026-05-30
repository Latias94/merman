# ASCII Color Role API - Milestones

Status: Active
Last updated: 2026-05-30

## Exit Criteria

- The public color API shape is documented and accepted through an ADR.
- Default `AsciiColorMode::Plain` output remains byte-for-byte identical to current output.
- ANSI/HTML color insertion happens only after layout and measurement are complete.
- Flowchart has one parser-backed colored vertical slice before broader adoption.
- Mermaid style/class/linkStyle mapping is either implemented behind tests or split into a follow-on.

## Milestones

- M0: DONE. Draft API design and ADR 0067 accepted the public `AsciiRenderOptions` migration.
- M1: Add role-aware canvas and forced output encoders without changing default rendering.
- M2: Apply semantic color roles to one flowchart slice.
- M3: Decide broader family adoption and Mermaid style mapping lanes.
