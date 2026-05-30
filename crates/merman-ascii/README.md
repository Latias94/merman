# merman-ascii

[![Crates.io](https://img.shields.io/crates/v/merman-ascii.svg)](https://crates.io/crates/merman-ascii)
[![Documentation](https://docs.rs/merman-ascii/badge.svg)](https://docs.rs/merman-ascii)
[![Crates.io Downloads](https://img.shields.io/crates/d/merman-ascii.svg)](https://crates.io/crates/merman-ascii)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

`merman-ascii` is the terminal/text rendering crate for
[merman](https://github.com/Latias94/merman). It renders Mermaid typed models as stable ASCII or
Unicode text output for terminals, logs, documentation pipelines, and environments where SVG is not
the right output format.

This crate is intentionally model-driven. It consumes typed models from `merman-core`; it does not
parse Mermaid syntax itself.

## Current Status

This crate contains the public API foundation, options, errors, third-party provenance, copied
upstream golden fixtures, flowchart rendering, and expanding sequence rendering. Flowcharts with
LR/TD/TB/BT/RL root directions, boxed nodes, multiline node labels, common terminal shape
approximations, edge labels, open/dotted and thick edges, length spacing, and titled/nested
subgraphs with multiline and wrapped title rows can render through `render_flowchart`.
Basic sequence diagrams with participants, filled/open solid and dotted messages, self messages,
wrapped message labels, wrapped notes, sequence boxes, activations, actor create/destroy lifecycle
markers, visible autonumber, and sequence control blocks can render through `render_sequence` or
`render_model`. The classDiagram slice can render class boxes, members, methods, relationship
labels, single relationship layouts, layered chain/star multi-relationship layouts, and
adjacent-layer crossing layouts resolved by layer reordering for extension, dependency,
aggregation, and composition through `render_class` or `render_model`; same-endpoint and simple
mixed-parallel relationships render as distinct lanes, and simple spanning-level relationships route
through side lanes. Cyclic and unresolved dense graph shapes remain explicit diagnostics. Unrelated
standalone classes render as separate components beside the relationship layout. The ER slice can
render entity boxes, attributes, relationship labels, identifying and non-identifying lines, common
cardinality markers, layered chain/star
multi-relationship layouts, and adjacent-layer crossing layouts resolved by layer reordering through
`render_er` or `render_model`; same-endpoint and simple mixed-parallel relationships render as
distinct lanes, and simple spanning-level relationships route through side lanes. Cyclic and
unresolved dense graph shapes remain explicit diagnostics. Unrelated standalone entities render as
separate components beside the relationship layout. The XYChart slice can
render deterministic compact vertical bars, stair-step lines, mixed bar/line overlays, horizontal
bars, inferred numeric x labels, and ASCII/Unicode chart characters through `render_xychart` or
`render_model`; richer legends and full terminal graph layout remain follow-on work. Shipped
diagram families have opt-in ANSI/HTML foreground color roles through `AsciiColorMode`; flowchart
also maps Mermaid `classDef`, `class`, inline `style`, and `linkStyle` foreground colors for
`color` and `stroke`. Fill/background interpretation remains follow-on work.

Broader flowchart and sequence compatibility is tracked under
`docs/workstreams/ascii-renderer-compatibility-expansion/`,
`docs/workstreams/ascii-sequence-parity/`, and follow-on workstreams.

See `FLOWCHART_SUPPORT.md` and `SEQUENCE_SUPPORT.md` for the current support matrices.

## Shipped Diagram Matrix

| Diagram family | Public entry points | Shipped text subset |
| --- | --- | --- |
| flowchart/graph | `render_flowchart`, `render_model`, `merman::ascii::render_ascii_sync`, `merman-cli render --format ascii|unicode` | LR/TD/TB/BT/RL root directions, boxed nodes, common terminal shape approximations, labels, open/dotted/thick edges, titled/nested subgraphs with multiline and wrapped title rows, opt-in ANSI/HTML foreground color roles, and foreground `classDef`/`class`/`style`/`linkStyle` mapping. |
| sequenceDiagram | `render_sequence`, `render_model`, `merman::ascii::render_ascii_sync`, `merman-cli render --format ascii|unicode` | Participants, solid/dotted messages, notes, boxes, activations, lifecycle markers, autonumber, core control blocks, and opt-in ANSI/HTML foreground color roles. |
| classDiagram | `render_class`, `render_model`, `merman::ascii::render_ascii_sync`, `merman-cli render --format ascii|unicode` | Class boxes, members, methods, labels, single relationships, layered chain/star multi-relationship layouts, adjacent-layer crossing layouts resolved by layer reordering, same-endpoint lanes, simple mixed-parallel lanes, simple spanning-level side lanes, unrelated standalone class components, and opt-in ANSI/HTML foreground color roles. |
| erDiagram | `render_er`, `render_model`, `merman::ascii::render_ascii_sync`, `merman-cli render --format ascii|unicode` | Entity boxes, attributes, labels, identifying/non-identifying relationships, common cardinality markers, layered chain/star multi-relationship layouts, adjacent-layer crossing layouts resolved by layer reordering, same-endpoint lanes, simple mixed-parallel lanes, simple spanning-level side lanes, unrelated standalone entity components, and opt-in ANSI/HTML foreground color roles. |
| xychart | `render_xychart`, `render_model`, `merman::ascii::render_ascii_sync`, `merman-cli render --format ascii|unicode` | Compact vertical bars, stair-step lines, mixed overlays, horizontal bars, titles, axes, inferred numeric labels, and opt-in ANSI/HTML foreground color roles. |

Diagram families not listed here currently return `AsciiError::UnsupportedDiagram` through the
typed `render_model` path.

## XYChart ASCII Contract

The XYChart renderer uses a terminal-native scale instead of SVG coordinates. Vertical charts use a
fixed five-row value area, three-character category bands, and evenly divided y ticks from the typed
y-axis range. Bar heights are rounded into that five-row area. Line plots use the same scale and are
drawn as compact stair-step lines, then overlaid after bars so mixed plots remain visible.

Horizontal charts use a ten-character value axis and the same typed y-axis range for bar width and
line marker placement. Category labels come from the typed band x-axis when present; otherwise the
renderer infers numeric labels from the typed linear x-axis. Output is trimmed per line and remains
stable for snapshot tests.

## Intended Use

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
  - Source commit used for the initial port plan and copied fixtures:
    `6fffb8e2714acab2c4cb41c78894fabbc62cee56`
  - Upstream license: MIT
  - License copy: `LICENSES/mermaid-ascii-MIT.txt`
  - Fixture source inventory: `tests/testdata/mermaid-ascii/README.md`
- [`lukilabs/beautiful-mermaid`](https://github.com/lukilabs/beautiful-mermaid)
  - Source commit used for reference planning:
    `2ac8bbbb060ca0a65a6a21f3200bd99b1587b488`
  - Upstream license: MIT
  - License copy: `LICENSES/beautiful-mermaid-MIT.txt`
  - Intended use: reference algorithms, output ideas, and tests for class, ER, xychart, color, and
    multiline ASCII work.

The local `repo-ref/` directory is gitignored and is only a research reference. Any derived source,
fixtures, or notices required for builds and releases must live in tracked paths in this crate.
`merman-ascii` remains model-driven: reference parsers are not copied into this crate.

## License

`merman-ascii` follows the workspace license: `MIT OR Apache-2.0`.

Ported algorithm work and copied fixtures derived from reference implementations preserve upstream
MIT license notices in `LICENSES/`.
