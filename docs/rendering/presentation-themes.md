# Historical Presentation Theme Ownership

This page records the retired Options 2 presentation interface. It is historical context,
not an API guide for the current source. The former host theme and presentation profile builders
have been removed; their inputs are not aliases for compiled diagram themes.

The old interface separated semantic host colors, product presentation behavior, Mermaid
configuration, and SVG output policy. The former `merman-modern` profile combined theme defaults,
Neo look, an ELK preference, and Flowchart rendering behavior. Current callers select a compiled
theme and set Mermaid `look`, layout, and output policy through their independent owners.

Use these maintained entry points:

| Task | Current owner |
| --- | --- |
| Create, customize, or share a compiled diagram theme | [Diagram theme guide](custom-diagram-themes.md) |
| Apply a bundled preset in Rust | [`theme_preset.rs`](../../crates/merman/examples/theme_preset.rs) |
| Compile application color tokens in Rust | [`custom_diagram_theme.rs`](../../crates/merman/examples/custom_diagram_theme.rs) |
| Configure Mermaid compatibility and binding options | [Options JSON](../bindings/OPTIONS_JSON.md#diagram-theme) |
| Select SVG output policy | [SVG output pipeline](SVG_OUTPUT_PIPELINE.md) |
| Configure terminal palettes and color encoding | [Terminal theme API](../../crates/merman-ascii/README.md#terminal-theme-api) |
| Migrate an older presentation integration | [Theme migration reference](../release/ALPHA7_TO_0_8_0_THEME_MIGRATION.md) |

## Terminal themes

Terminal palette semantics remain current and are documented by the
[ASCII renderer](../../crates/merman-ascii/README.md#terminal-theme-api), with binding fields under
[ASCII options](../bindings/OPTIONS_JSON.md#ascii-options). A host may map application colors into
both output inputs, but selecting a compiled SVG theme does not populate `ascii.theme` or choose
a terminal color mode.

## Historical evidence

The original ownership decision is retained in
[ADR-0077](../adr/0077-presentation-theme-and-output-ownership.md). The versioned theme authoring
facade is described in [ADR-0082](../adr/0082-versioned-theme-authoring-facade.md). Git history
preserves the retired API examples and Options 2 payloads; use the migration reference above
when adapting an older caller.
