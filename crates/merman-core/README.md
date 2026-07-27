# merman-core

[![Crates.io](https://img.shields.io/crates/v/merman-core.svg)](https://crates.io/crates/merman-core) [![Documentation](https://docs.rs/merman-core/badge.svg)](https://docs.rs/merman-core) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

`merman-core` is the parser and semantic-model crate behind [`merman`](https://crates.io/crates/merman). Use it when you need Mermaid detection, metadata, compatibility semantic JSON, parser-backed editor facts, or typed render models without pulling in layout, SVG, or binary-export dependencies.

Most applications that want rendered output should use the `merman` facade instead.

## Quick Start

Use the installation command projected for the current repository release state:

<!-- BEGIN GENERATED RELEASE README CORE_INSTALL -->

```sh
cargo add merman-core --git https://github.com/Latias94/merman
```

<!-- END GENERATED RELEASE README CORE_INSTALL -->

Parse Mermaid into its compatibility semantic JSON projection:

```rust
use merman_core::{Engine, ParseOptions};

fn main() -> Result<(), merman_core::Error> {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_sync("flowchart TD; A[API] --> B[DB];", ParseOptions::strict())?
        .expect("diagram detected");

    assert_eq!(parsed.meta.diagram_type, "flowchart-v2");
    println!("{}", parsed.model);

    Ok(())
}
```

## What It Provides

- Mermaid detection and preprocessing, including front matter and directives.
- Strict and lenient parsing through `ParseOptions`.
- Structured parse diagnostics with exact spans, insertion points, or explicit fallback locations.
- Compatibility semantic JSON via `Engine::parse_diagram_sync`.
- Typed render models via `Engine::parse_diagram_for_render_model_sync`.
- Parser-backed editor facts and family capability metadata.
- Metadata-only parsing for integrations that only need type, title, and effective config.
- Runtime-agnostic async APIs plus synchronous helpers.

`merman-core` has no default Cargo features. Mermaid parsing, configuration, sanitization, detection, and family facts are unconditional; optional `system-*` features only make explicit host runtime adapters available.

## Skip Detection When The Type Is Known

Markdown renderers often know the diagram type from the fence info string. Use the `*_with_type_sync` APIs to skip detection.

```rust
use merman_core::{Engine, ParseOptions};

fn main() -> Result<(), merman_core::Error> {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_with_type_sync(
            "sequence",
            "sequenceDiagram\nAlice->>Bob: Hello",
            ParseOptions::strict(),
        )?
        .expect("diagram detected");

    assert_eq!(parsed.meta.diagram_type, "sequence");
    Ok(())
}
```

Common internal ids include `flowchart-v2`, `sequence`, `classDiagram`, `stateDiagram`, `architecture`, `mindmap`, and `gantt`.

## Rendering Handoff

If the next step is layout or SVG rendering, prefer `Engine::parse_diagram_for_render_model_sync`. It returns the typed render projection of the same family-owned semantics and avoids building a large compatibility JSON tree. Applications that want complete SVG or layout JSON should normally use `merman::svg::HeadlessRenderer`, which carries this typed projection through the canonical render operation.

```rust
use merman_core::{Engine, ParseOptions};

fn main() -> Result<(), merman_core::Error> {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync("flowchart TD; A --> B", ParseOptions::strict())?
        .expect("diagram detected");

    println!("{} -> {}", parsed.meta.diagram_type, parsed.model.kind());
    Ok(())
}
```

## Compatibility

`merman-core` tracks Mermaid `@11.16.0` and treats pinned upstream behavior as the compatibility target. Compatibility semantic JSON is the public serialized parser projection. It is not a second successful grammar or the master built-in render input. Typed render models and editor facts project the same family semantic construction into purpose-specific shapes.

The built-in Diagram Family catalog is the authoritative source for ids, aliases, detector order, parser/editor/render capabilities, metadata, configuration namespaces, and authoring headers. The pinned Mermaid catalog is complete and independent of Cargo feature selection. Custom parser overlays remain explicit and do not inherit a built-in renderer or editor capability.

The public Rust flowchart render type is `diagrams::flowchart::FlowchartModel`. The former `FlowchartV2Model` type name was removed during the architecture reset without a deprecated alias. This rename does not change Mermaid's `flowchart-v2` diagram id or the compatibility layout JSON `FlowchartV2` variant key.

Core does not decide user-visible diagnostic merge policy. It reports parser facts and capability gaps; `merman-analysis` owns rule ids, Markdown remapping, recovered-parser deduplication, and editor-facing fallback policy.

## Migration Notes

`Error::DiagramParse` carries `diagnostic: ParseDiagnostic` instead of a raw parse-message field. Call `diagnostic.message()` for display text, and use `diagnostic.span()`, `diagnostic.span_kind()`, and `diagnostic.code()` when an integration can preserve structured parser metadata.

Railroad repetition bounds use `RailroadRepeatBound` for both `min` and `max`. Use `ZERO`, `ONE`, or `RailroadRepeatBound::from(value)` for finite bounds and `RailroadRepeatBound::INFINITY` for an unbounded maximum. Finite bounds serialize as JSON numbers, while infinity serializes as `null`.

## Maintainer Parser Generation

The Class, ER, Flowchart, Sequence, and State grammars use checked-in LALRPOP output. Downstream builds compile those parsers directly and do not run the LALRPOP generator. After editing any `src/diagrams/*_grammar.lalrpop` file, regenerate and verify the complete five-parser set:

```console
cargo run -p xtask -- gen-lalrpop-parsers
cargo run -p xtask -- verify-lalrpop-parsers
```

`verify-generated` includes the same byte-for-byte freshness check.

See the [parser generation guide](https://github.com/Latias94/merman/blob/main/docs/development/PARSER_GENERATION.md) for source ownership, transaction semantics, review expectations, and the parser/editor/LSP verification sequence.
