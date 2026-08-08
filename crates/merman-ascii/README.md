# merman-ascii

[![Crates.io](https://img.shields.io/crates/v/merman-ascii.svg)](https://crates.io/crates/merman-ascii) [![Documentation](https://docs.rs/merman-ascii/badge.svg)](https://docs.rs/merman-ascii) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

`merman-ascii` is the terminal/text rendering crate for [merman](https://github.com/Latias94/merman). It renders Mermaid typed models as stable ASCII or Unicode text output for terminals, logs, documentation pipelines, and environments where SVG is not the right output format.

This crate is intentionally model-driven. It consumes typed models from `merman-core`; it does not parse Mermaid syntax itself.

Terminal geometry uses `TerminalWidthProfile::Unicode` by default. Select
`TerminalWidthProfile::Cjk` when the target terminal renders East Asian ambiguous characters as
wide. Measurement, wrapping, truncation, placement, and output use the selected profile together;
the character set only selects structural glyphs and does not change authored-text width policy.
Because Unicode box-drawing and marker glyphs are East Asian Ambiguous, the CJK profile uses
single-cell ASCII structure even when `AsciiCharset::Unicode` was requested. Authored Unicode text
is preserved. This deterministic fallback prevents borders and routes from occupying two grid
cells per structural token.

`merman-ascii` has no optional Cargo features. Mermaid language semantics are unconditional in `merman-core`; system clock, time-zone, random, and timing adapters do not change which typed models this crate can render.

> **Implementation crate:** applications should select the `ascii` feature on the [`merman`](https://crates.io/crates/merman) facade. Depend on `merman-ascii` directly only when the host already owns a typed `merman-core::RenderSemanticModel`.

This model renderer does not own a runtime-policy constructor, so it does not forward `system-*` features. Applications that need host-derived values select adapters on their parsing/facade owner, capture one operation context, and pass its local time zone through `render_model_with_local_time_zone`. Deterministic and sandboxed applications should provide explicit operation values instead of enabling system adapters.

## Quick Start

Most applications should use the `merman` facade so parsing and text rendering stay in one operation:

```toml
[dependencies]
merman = { git = "https://github.com/Latias94/merman", default-features = false, features = ["ascii"] }
```

```rust
use merman::ascii::HeadlessAsciiRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = HeadlessAsciiRenderer::new().with_strict_parsing();
    let output = renderer
        .render_ascii_sync("flowchart LR\n  Source --> Terminal")?
        .expect("diagram detected");

    println!("{output}");
    Ok(())
}
```

Depend on `merman-ascii` directly only when the application already owns a typed `merman-core::RenderSemanticModel`.

## Supported Families

ASCII support describes the quality of the terminal projection, not whether Merman can parse the Mermaid family. Query the runtime capability records when an application needs to populate an output picker.

| Semantic coverage | Primary projection | Families | Output contract |
| --- | --- | --- | --- |
| Partial | Diagrammatic | Flowchart, Sequence, State, Class, ER, TreeView, XYChart | Core semantics render with documented limits; Class and ER can independently fall back to structured relation text. TreeView still needs a complete typed-field and terminal-usefulness review. |
| Partial | Structured text | Gantt, GitGraph, Journey, Kanban, Mindmap, Packet, Timeline | Ordered, readable reports or outlines instead of browser-oriented chart geometry. Packet currently reports ranges in rows rather than preserving spatial bit widths. |

Every concrete built-in typed family has one capability record. `semantic_coverage`,
`primary_projection`, and `structured_text_fallback` are independent; legacy `support_level` is
derived from them. Other Mermaid families return `AsciiError::UnsupportedDiagram` through the typed
model path. The generated [ASCII/Unicode support matrix](https://github.com/Latias94/merman/blob/main/docs/rendering/ASCII_SUPPORT_MATRIX.md) is the user-facing source of truth for exact limits. Family-specific engineering detail lives in [Flowchart](https://github.com/Latias94/merman/blob/main/crates/merman-ascii/FLOWCHART_SUPPORT.md), [Sequence](https://github.com/Latias94/merman/blob/main/crates/merman-ascii/SEQUENCE_SUPPORT.md), and [State](https://github.com/Latias94/merman/blob/main/crates/merman-ascii/STATE_SUPPORT.md) support notes.

## Terminal Theme API

`AsciiColorTheme::from_terminal_palette` derives terminal roles from a compact `AsciiTerminalPalette`: required `foreground` and `background`, plus optional `line`, `accent`, `muted`, `surface`, and `border` colors. The derived theme maps only terminal-meaningful roles such as text, borders, edge lines/arrows, sequence lifelines, and chart series colors. It does not import SVG CSS-variable semantics into text output. Explicit `AsciiColorTheme::with_role` calls still take precedence after derivation.

Bindings expose the same shape as `ascii.theme` in options JSON. Color values use the existing CSS color parser for opaque terminal colors; transparent colors are rejected rather than silently falling back.

## XYChart ASCII Contract

The XYChart renderer uses a terminal-native scale instead of SVG coordinates. By default, vertical charts use a five-row value area, three-character category bands, and evenly divided y ticks from the typed y-axis range. `AsciiRenderOptions::with_xychart_vertical_plot_height` and `AsciiRenderOptions::with_xychart_category_band_width` can widen that compact plot policy without changing the typed model contract. Bar heights are rounded into the configured value area. Line plots use the same scale and are drawn as compact stair-step lines, then overlaid after bars so mixed plots remain visible.

Horizontal charts use a ten-character value axis by default and the same typed y-axis range for bar width and line marker placement. `AsciiRenderOptions::with_xychart_horizontal_plot_width` adjusts that axis. Category labels come from the typed band x-axis when present; otherwise the renderer infers numeric labels from the typed linear x-axis. Output is trimmed per line and remains stable for snapshot tests.

Charts with more than one series render a compact legend row before the plot. When a Mermaid plot statement includes a user-authored series title, the legend uses that typed model title; otherwise it falls back to stable terminal labels such as `Bar 1` and `Line 1`.

The renderer consumes the typed XYChart display policy from `merman-core`. `xyChart.showTitle`, `xyChart.showDataLabel`, `xyChart.showDataLabelOutsideBar`, and `xyChart.xAxis/yAxis.showLabel/showTitle/showTick/showAxisLine` affect terminal output. Tick marks can render independently from axis lines so hidden axis lines do not accidentally hide tick intent. For a single bar series, data labels stay close to the bars and respect `showDataLabelOutsideBar`. For line charts and multi-series charts, `showDataLabel` emits explicit `values:` rows keyed by series title and category so terminal output has a stable tooltip replacement without covering the plot.

## Relation Summary Diagnostics

Class and ER diagrams fall back to readable `relations:` summary sections when a topology cannot be drawn as a deterministic terminal grid, when class relationships cross namespace/container boundaries, when route or overlay collision checks would damage a box, or when the selected routed scene exceeds `AsciiRenderOptions::max_grid_cells`. Default output hides that internal reason to keep terminal text stable and user-facing. Enable `AsciiRenderOptions::with_relation_summary_diagnostics(true)` to add a muted diagnostic row such as `reason: grid_budget actual=12 limit=1` directly under `relations:`. Possible reason keys are `crossing`, `route_collision`, `overlay_collision`, and `grid_budget`.

## Direct Model API

```rust,no_run
use merman_ascii::{AsciiRenderOptions, AsciiRenderer};
use merman_core::{Engine, ParseOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "flowchart TD\nsubgraph one\nA((Start)) -- go --> B[(DB)]\nend",
            ParseOptions::strict(),
        )?
        .expect("diagram detected");

    let renderer = AsciiRenderer::new(AsciiRenderOptions::default())?;
    let text = renderer.render_model(&parsed.model)?;

    println!("{text}");
    Ok(())
}
```

## Upstream Provenance

The ASCII renderer work is based on and informed by MIT-licensed reference implementations:

- [`AlexanderGrooff/mermaid-ascii`](https://github.com/AlexanderGrooff/mermaid-ascii)
  - Source commit used for the initial port plan and copied fixtures: `6fffb8e2714acab2c4cb41c78894fabbc62cee56`
  - Upstream license: MIT
  - License copy: `LICENSES/mermaid-ascii-MIT.txt`
  - Fixture source inventory: `tests/testdata/mermaid-ascii/README.md`
- [`lukilabs/beautiful-mermaid`](https://github.com/lukilabs/beautiful-mermaid)
  - Source commit used for reference planning: `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488`
  - Upstream license: MIT
  - License copy: `LICENSES/beautiful-mermaid-MIT.txt`
  - Intended use: reference algorithms, output ideas, and tests for class, ER, xychart, color, and multiline ASCII work.
  - Promoted ideas are re-expressed as local semantic probes, including ampersand flowchart fan-in/fan-out, Class annotations and methods, ER attributes with identifying relationships, Sequence multi-message ordering, and XYChart multi-series value disclosure.

The local `repo-ref/` directory is gitignored and is only a research reference. Any derived source, fixtures, or notices required for builds and releases must live in tracked paths in this crate. `merman-ascii` remains model-driven: reference parsers are not copied into this crate.

## License

`merman-ascii` follows the workspace license: `MIT OR Apache-2.0`.

Ported algorithm work and copied fixtures derived from reference implementations preserve upstream MIT license notices in `LICENSES/`.
