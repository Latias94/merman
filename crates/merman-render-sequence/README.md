# merman-render-sequence

Mermaid Sequence preparation, layout, and complete SVG emission. This crate owns typography,
text and math measurement, participant drawing, control frames, and activation rendering.
`svg` enables Mermaid parity output and `native-compact` adds the opt-in compact profile. The
facade supplies the original operation context, finalizes diagram IDs, and checks terminal output.
Preparation and emission share the same operation budget. Select `math` to include RaTeX.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
