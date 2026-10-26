# merman-core

[![Crates.io](https://img.shields.io/crates/v/merman-core.svg)](https://crates.io/crates/merman-core) [![Documentation](https://docs.rs/merman-core/badge.svg)](https://docs.rs/merman-core) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

`merman-core` is the parser and semantic-model crate behind [`merman`](https://crates.io/crates/merman). Use it for canonical Mermaid meaning and metadata, with optional editor facts, full presentation, or compatibility JSON. It does not require layout, SVG, or binary-export dependencies.

Most applications that want rendered output should use the `merman` facade instead.

The examples below target the current checkout. Use tagged documentation and matching dependencies for published releases.

## Source Feature Migration

The current checkout keeps empty default features and adds positive `diagram-*` selectors.
Select `all-diagrams` to retain every built-in parser, or select only the families your host accepts:

```toml
merman-core = { path = "../merman/crates/merman-core", default-features = false, features = ["diagram-gantt"] }
```

Family-exclusive typed models and enum variants are conditional. The complete
identity catalog still recognizes disabled families, whose strict parse returns
`UnsupportedDiagram`. `supported_diagrams()` enumerates compiled parsers;
`diagram_family_capabilities()` retains complete identities with actual implementation flags.
See the [feature guide](../../docs/FEATURES.md#select-diagram-families) for custom registries,
suppression, and Cargo feature-unification rules.

### Semantic Editor Capture

Source builds can request canonical models and parser recovery facts through `SemanticParser`.
Enable `editor-facts` with the required diagram selectors; this API does not require
`presentation` or `compat-json`:

```toml
merman-core = { path = "../merman/crates/merman-core", default-features = false, features = ["diagram-class", "editor-facts"] }
```

```rust
use merman_core::{ParsedEditorFacts, SemanticParser, SemanticSnapshotCapture};

let capture = SemanticParser::new()
    .capture_diagram_snapshot_sync("classDiagram\nclass User\n");
let SemanticSnapshotCapture::Snapshot(snapshot) = capture else {
    panic!("valid Class input should reach its family parser");
};
assert!(snapshot.outcome().parsed_model().is_some());
let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
    panic!("the enabled Class parser provides editor facts");
};
assert!(facts.symbols.iter().any(|symbol| symbol.name == "User"));
```

A family parse failure retains its original error and requested recovery facts in the snapshot.
Failures before family metadata retain source-configuration evidence in
`SemanticSnapshotCapture::Failed` or `Panicked`. Controlled variants return cooperative
cancellation through the outer `OperationControlResult`; known-type variants skip detection.
Ordinary `parse_diagram_*` calls omit editor observation.

## Quick Start

Select the required families without output or presentation features:

```toml
merman-core = { path = "../merman/crates/merman-core", default-features = false, features = ["diagram-flowchart", "diagram-sequence"] }
```

Parse canonical meaning without evaluating visual theme programs:

```rust
use merman_core::{SemanticDiagramModel, SemanticParser};

fn main() -> Result<(), merman_core::Error> {
    let parsed = SemanticParser::new()
        .parse_diagram_sync("flowchart TD; A[API] --> B[DB];")?;

    assert_eq!(parsed.metadata.diagram_type, "flowchart-v2");
    let SemanticDiagramModel::Flowchart(model) = parsed.model else {
        unreachable!("the Flowchart parser returns its canonical model");
    };
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.edges[0].from, "A");

    Ok(())
}
```

## What It Provides

- Mermaid detection and preprocessing, including front matter and directives.
- Strict and lenient parsing through `ParseOptions`.
- Structured parse diagnostics with exact spans, insertion points, or explicit fallback locations.
- Canonical typed models via `SemanticParser::parse_diagram_sync`.
- Compatibility semantic JSON via `MermaidEngine::parse_diagram_sync` with `compat-json`.
- Paired typed render models via `MermaidEngine::parse_diagram_for_render_model_sync` with `presentation`.
- Parser-backed editor facts and family capability metadata.
- Full metadata-only parsing for integrations that need type, title, and evaluated config.
- Project-owned civil and offset time types that preserve Mermaid's wide year domain.
- Runtime-agnostic async APIs plus synchronous helpers.

`merman-core` has no default Cargo features. Select `all-diagrams` or the required `diagram-*`
families to compile built-in parsers and typed models. Configuration, sanitization, detection, and
complete identity facts remain available without a family; optional `system-*` features make
explicit host runtime adapters available.

`presentation` enables the full `MermaidEngine`, theme evaluation, and paired render context.
`compat-json` implies `presentation` because Mermaid's JSON includes presentation-derived fields.
`editor-facts` is independent of both. Cargo feature union may compile these capabilities, but
`SemanticParser` still performs only the requested semantic work.
The `terminal-text` leaf supplies shared terminal normalization and Unicode width helpers.
ASCII and facade diagnostic owners select it explicitly; a direct semantic-only core consumer
does not compile those helpers.

Relative operation deadlines use the native monotonic clock on supported targets. Browser-facing
`wasm32-unknown-unknown` artifacts must enable `operation-deadlines` to expose the deadline methods
and use the Web monotonic clock. This keeps browser adapters out of pure WASM dependency closures;
cancellation remains available in every artifact.

## Deterministic Time

Use `time::CivilDate` for date-only runtime controls such as `runtime::RuntimePolicy::with_fixed_today`, then install the policy through `SemanticParser::with_runtime_policy` or `MermaidEngine::with_runtime_policy`. Its canonical text syntax uses four unsigned digits for years `0000` through `9999`, a leading `+` for later years, and `-` plus at least four digits for negative years. The signed 32-bit year domain includes Mermaid's `+10000` and `-10000` boundaries. `time::CivilDateTime`, `time::UtcOffset`, and `time::OffsetDateTime` provide checked calendar and instant conversions without exposing a third-party time type in the public API.

System clock and named time-zone access remain opt-in through the corresponding `system-*` features. Named-zone rules are implemented with Jiff internally, while deterministic and fixed-offset profiles do not pull Jiff into their dependency closure.

## Skip Detection When The Type Is Known

Markdown renderers often know the diagram type from the fence info string. Use the `*_with_type_sync` APIs to skip detection.

```rust
use merman_core::SemanticParser;

fn main() -> Result<(), merman_core::Error> {
    let parser = SemanticParser::new();
    let parsed = parser
        .parse_diagram_with_type_sync(
            "sequence",
            "sequenceDiagram\nAlice->>Bob: Hello",
        )?;

    assert_eq!(parsed.metadata.diagram_type, "sequence");
    Ok(())
}
```

Common internal ids include `flowchart-v2`, `sequence`, `classDiagram`, `stateDiagram`, `architecture`, `mindmap`, and `gantt`.

## Rendering Handoff

If the next step is low-level layout or SVG rendering, prefer
`MermaidEngine::parse_diagram_for_render_model_sync` with `presentation` enabled. It returns the typed render projection of the same
family-owned semantics and avoids building a large compatibility JSON tree. Applications that
start with Mermaid source and want complete SVG, ASCII, layout JSON, or binary output should
normally use `merman::Renderer` with a typed target request; that facade carries the same
projection through the canonical operation control, runtime context, and resource policy.

```rust
use merman_core::{MermaidEngine, ParseOptions};

fn main() -> Result<(), merman_core::Error> {
    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync("flowchart TD; A --> B", ParseOptions::strict())?
        .expect("diagram detected");

    println!(
        "{} -> {}",
        parsed.metadata().diagram_type,
        parsed.model().kind()
    );
    Ok(())
}
```

## Compatibility

For compatibility JSON, add `compat-json` and call `MermaidEngine::parse_diagram_sync(source,
ParseOptions::strict())`. This preserves full theme/config evaluation and its error phase.
`SemanticMetadata::config` retains admitted authored config; `semantic_config` contains effective
semantic policy without calculated theme values. Invalid visual programs can succeed on the
semantic route while the full route returns its existing theme error. Both routes retain
configuration admission, security normalization, and family validation.

The old `Engine` and `ParseMetadata` aliases are removed. Use `MermaidEngine` and
`PresentationMetadata` for full operations, or `SemanticParser` and `SemanticMetadata` for lean
operations. Full custom detectors and compatibility adapters remain full-only. See the
[source migration](../../docs/migrations/render-reuse-host-integration.md).

Semantic JSON uses `ManagedSemanticJson`; keep its managed owner for deep clone, equality, destruction, and output. Direct State, Block, Treemap, and Ishikawa model users must migrate to canonical document/child indices. See the [0.8.0 to 0.9.0 migration](../../docs/release/V080_TO_V090_UPGRADE_GUIDE.md#rust-models-and-json).

Published Merman 0.9.2 retains the Mermaid `12.1.0` baseline selected by stable 0.8.0. Development
targets Mermaid `12.2.0`; use the matching release tag for published parser behavior. The selected
pinned upstream behavior graph is the compatibility authority for the current checkout.
Compatibility semantic JSON is the public serialized parser projection. It is not a second
successful grammar or the master built-in render input. Typed render models and editor facts
project the same family semantic construction into purpose-specific shapes.

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
