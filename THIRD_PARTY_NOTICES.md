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

## Venn Layout References

Merman's Venn layout kernel follows the pinned layout and geometry behavior used by Mermaid
`venn-beta`.

- Upstream project: <https://github.com/upsetjs/venn.js>
- Upstream package: `@upsetjs/venn.js@2.0.0`
- Upstream license: MIT
- Optimizer reference: `fmin@0.0.4`
- Optimizer license: BSD-3-Clause

The Rust implementation in `crates/merman-render/src/venn.rs` is a source-backed compatibility
port for headless rendering. Keep provenance comments and numeric oracle tests when changing this
kernel.
