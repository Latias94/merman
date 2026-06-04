# Third-Party Notices

This file records third-party projects that merman intentionally follows for compatibility,
parity, or implementation reference.

## Mermaid

Merman is an independent, headless Rust re-implementation of the Mermaid diagram language and
rendering behavior. It is not affiliated with or endorsed by the Mermaid project.

- Upstream project: <https://github.com/mermaid-js/mermaid>
- Upstream license: MIT, see <https://github.com/mermaid-js/mermaid/blob/develop/LICENSE>
- Current compatibility baseline: `mermaid@11.15.0`

Mermaid's MIT license is compatible with merman's `MIT OR Apache-2.0` licensing model, but any
Mermaid-derived source, test fixture, expected output, documentation excerpt, or mechanically
ported behavior must keep source provenance and license attribution. Merman tracks that provenance
in code comments, fixture names, workstream evidence, and release documentation where applicable.

Use "Mermaid-compatible" or "Mermaid-parity" when describing behavior. Do not describe merman as an
official Mermaid package or imply that Mermaid endorses this project.
