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

## Resource Policy

`AsciiRenderOptions::resources` owns six typed limits: checked grid extent, deterministic layout
work, aggregate logical document cells, actual encoded output bytes, bytes in one grapheme cluster,
and semantic nesting depth. Select a shared `ResourceProfile` with
`AsciiRenderOptions::with_resource_profile`, or tighten one limit with
`with_resource_limit(AsciiResourceLimitId, value)`. `UnboundedForTrustedInput` uses an explicit
unbounded value rather than a numeric sentinel; arithmetic overflow and allocation failure remain
fallible.

Resource limits are hard errors. They never select a relation summary or another lower-fidelity
projection. Plain, ANSI16, ANSI256, TrueColor, and HTML use the same logical document accounting,
while `max_ascii_output_bytes` counts the actual bytes produced by each encoder.

## Terminal Theme API

`AsciiColorTheme::from_terminal_palette` derives terminal roles from a compact `AsciiTerminalPalette`: required `foreground` and `background`, plus optional `line`, `accent`, `muted`, `surface`, and `border` colors. The derived theme maps only terminal-meaningful roles such as text, borders, edge lines/arrows, sequence lifelines, and chart series colors. It does not import SVG CSS-variable semantics into text output. Explicit `AsciiColorTheme::with_role` calls still take precedence after derivation.

Bindings expose the same shape as `ascii.theme` in options JSON. Color values use the existing CSS color parser for opaque terminal colors; transparent colors are rejected rather than silently falling back.

## XYChart ASCII Contract

The XYChart renderer builds one typed `TerminalChartPlan` instead of rebuilding coordinates independently in each orientation. Every series consumes the model's `data` tuples as the source of truth for x/y coordinates; `values` is only a compatibility fallback for manually constructed legacy models that omit `data`. Band axes resolve authored categories, linear axes map each numeric x coordinate through its explicit or inferred range, and negative, reversed, and degenerate value ranges remain deterministic. Data values use shortest-roundtrip formatting, while axis ticks use a separate scale-aware formatter so tiny ranges such as `0.001 --> 0.005` do not collapse into repeated labels.

By default, vertical charts use a five-row value area and three-character category bands. `AsciiRenderOptions::with_xychart_vertical_plot_height` and `AsciiRenderOptions::with_xychart_category_band_width` adjust that compact policy. Multiple bar series divide each category into stable lanes instead of overwriting one another. Line series use their typed x coordinates and share a stair-step topology layer that is painted after bars so mixed plots remain visible; missing samples split paths instead of creating false connecting segments.

Horizontal charts use a ten-character value axis by default; `AsciiRenderOptions::with_xychart_horizontal_plot_width` adjusts it. Multiple bar series receive independent rows per category, and horizontal line samples are connected rather than emitted as isolated points. Category labels come from the typed band axis when present; linear axes retain their typed numeric domain. The complete expanded plot extent is checked before row allocation.

Charts with more than one series render a compact legend row before the plot. When a Mermaid plot statement includes a user-authored series title, the legend uses that typed model title; otherwise it falls back to stable terminal labels such as `Bar 1` and `Line 1`.

The renderer consumes the typed XYChart display policy from `merman-core`. `xyChart.showTitle`, `xyChart.showDataLabel`, `xyChart.showDataLabelOutsideBar`, and `xyChart.xAxis/yAxis.showLabel/showTitle/showTick/showAxisLine` affect terminal output. Tick marks can render independently from axis lines. For a single bar series, data labels stay close to the bars and respect `showDataLabelOutsideBar`. For line and multi-series charts, `showDataLabel` emits exact `values:` rows keyed by series title and typed x coordinate. Multi-series identity, authored point labels, clipped samples, missing values, orphan labels, duplicate or over-wide categories, quantized point collisions, overlapping dense bars, same-row coordinate collisions, and insufficient grouped-bar lanes also trigger deterministic disclosure so semantic facts are not silently lost when terminal geometry is approximate.

## Relation Summary Diagnostics

Class and ER diagrams fall back to readable `relations:` summary sections when a topology cannot be drawn as a deterministic terminal grid, when class relationships cross namespace/container boundaries, or when route or overlay collision checks would damage a box. Default output hides that internal reason to keep terminal text stable and user-facing. Enable `AsciiRenderOptions::with_relation_summary_diagnostics(true)` to add a muted diagnostic row such as `reason: crossing` directly under `relations:`. Possible reason keys are `crossing`, `route_collision`, and `overlay_collision`. Resource limits always return `AsciiError::ResourceLimitExceeded` instead of selecting this summary path.

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
