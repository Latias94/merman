# merman-ascii

`merman-ascii` is the terminal/text rendering crate for
[merman](https://github.com/Latias94/merman). It renders Mermaid typed models as stable ASCII or
Unicode text output for terminals, logs, documentation pipelines, and environments where SVG is not
the right output format.

This crate is intentionally model-driven. It consumes typed models from `merman-core`; it does not
parse Mermaid syntax itself.

## Current Status

This crate contains the public API foundation, options, errors, third-party provenance, copied
upstream golden fixtures, flowchart rendering, and expanding sequence rendering. Flowcharts with
boxed nodes, multiline node labels, common terminal shape approximations, edge labels, open/dotted
edges, length spacing, and titled/nested subgraphs can render through `render_flowchart`. Basic
sequence diagrams with participants, filled/open solid and dotted messages, self messages,
wrapped message labels, wrapped notes, sequence boxes, activations, actor create/destroy lifecycle
markers, visible autonumber, and sequence control blocks can render through `render_sequence` or
`render_model`.

Broader flowchart and sequence compatibility is tracked under
`docs/workstreams/ascii-renderer-compatibility-expansion/`,
`docs/workstreams/ascii-sequence-parity/`, and follow-on workstreams.

See `FLOWCHART_SUPPORT.md` and `SEQUENCE_SUPPORT.md` for the current support matrices.

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

The ASCII renderer work is based on the MIT-licensed
[`AlexanderGrooff/mermaid-ascii`](https://github.com/AlexanderGrooff/mermaid-ascii) project.

- Source commit used for the initial port plan and copied fixtures: `6fffb8e`
- Upstream license: MIT
- License copy: `LICENSES/mermaid-ascii-MIT.txt`
- Fixture source inventory: `tests/testdata/mermaid-ascii/README.md`

The local `repo-ref/` directory is gitignored and is only a research reference. Any derived source,
fixtures, or notices required for builds and releases must live in tracked paths in this crate.

## License

`merman-ascii` follows the workspace license: `MIT OR Apache-2.0`.

Ported algorithm work and copied fixtures derived from `mermaid-ascii` preserve the upstream MIT
license notice in `LICENSES/mermaid-ascii-MIT.txt`.
