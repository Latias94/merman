# merman-render-sequence

This crate owns layout preparation and complete SVG emission for Sequence and ZenUML.
Both families use the operation's text measurer, viewport policy, and bounded SVG writer.
The facade selects the operation context, finalizes diagram IDs, and admits terminal output.

The default `diagram-sequence` feature provides Mermaid Sequence preparation, typography,
and layout. Add `svg` for Mermaid parity, `native-compact` for compact SVG, or `math` for RaTeX.
Select `--no-default-features --features diagram-zenuml` for ZenUML layout and SVG. That feature
owns ZenUML style, label measurement, and embedded [companion assets](assets/zenuml/README.md).

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman` for source-to-output rendering.
