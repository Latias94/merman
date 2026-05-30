# ASCII Color Role API - Milestones

Status: Closed
Last updated: 2026-05-30

## Exit Criteria

- The public color API shape is documented and accepted through an ADR.
- Default `AsciiColorMode::Plain` output remains byte-for-byte identical to current output.
- ANSI/HTML color insertion happens only after layout and measurement are complete.
- Flowchart has one parser-backed colored vertical slice before broader adoption.
- Mermaid style/class/linkStyle mapping is either implemented behind tests or split into a follow-on.

## Milestones

- M0: DONE. Draft API design and ADR 0067 accepted the public `AsciiRenderOptions` migration.
- M1: DONE. Role-aware canvas and forced output encoders landed without changing default rendering.
- M2: DONE. Flowchart semantic color roles landed with forced TrueColor and HTML coverage.
- M3: DONE. ACR-051 completed the shared substrate, ACR-052 completed class/ER role adoption,
  ACR-053 completed XYChart role adoption, ACR-054 completed sequence role adoption, and ACR-060
  completed the flowchart foreground style-mapping subset.
