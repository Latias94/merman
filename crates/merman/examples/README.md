# Merman Rust Examples

Every user-facing example in this directory is a self-contained Rust file. It declares its own Mermaid source, uses no shared `support` module, and can be copied into another crate's `examples/` directory.

Run commands below from the Merman repository root. Start with the default complete SVG build:

```sh
cargo run -p merman --example render_svg > diagram.svg
```

## Choose The Right Entry Point

| Your task | Start with | Why |
| --- | --- | --- |
| Render one source string to SVG | [`render_svg.rs`](render_svg.rs) | Uses `Renderer` with a typed `SvgRequest` and keeps the operation boundary explicit. |
| Embed several SVGs in one HTML document | [`embed_multiple_svgs.rs`](embed_multiple_svgs.rs) | Uses typed SVG requests with caller-owned IDs that remain unique after normalization. |
| Render many independent files with one policy | [`render_many.rs`](render_many.rs) | Reuses one configured `Renderer` across operations. |
| Export a bounded PNG | [`render_png.rs`](render_png.rs) | Selects a fit box, scale, background, and allocation limits before rasterization. |
| Render bounded terminal text | [`render_terminal.rs`](render_terminal.rs) | Selects Unicode or ASCII-only output explicitly. |
| Emit a plain JSON report for an agent or log | [`render_agent_log.rs`](render_agent_log.rs) | Uses canonical report serialization and typed outcome handling. |
| Apply a terminal host palette | [`terminal_palette.rs`](terminal_palette.rs) | Maps explicit RGB colors to terminal roles with TrueColor encoding. |
| Inspect the parsed semantic model | [`inspect_semantics.rs`](inspect_semantics.rs) | Uses `Engine` without requiring SVG rendering. |
| Inspect computed geometry and routes | [`inspect_layout.rs`](inspect_layout.rs) | Stops after typed layout and serializes the result. |
| Apply application-wide Mermaid defaults | [`configure_mermaid.rs`](configure_mermaid.rs) | Keeps host configuration outside user-authored diagram source. |
| Make relative dates deterministic | [`deterministic_gantt.rs`](deterministic_gantt.rs) | Pins "today" and the local offset for snapshots and reproducible builds. |
| Apply a ready-made product presentation | [`presentation_profile.rs`](presentation_profile.rs) | Combines the `merman-modern` profile with a semantic editor theme. |
| Map an application's own theme tokens | [`custom_presentation_theme.rs`](custom_presentation_theme.rs) | Builds a `HostTheme` from semantic roles instead of family-specific CSS. |
| Control consumer SVG cleanup and styling | [`custom_svg_pipeline.rs`](custom_svg_pipeline.rs) | Builds an explicit resvg-safe, background, and scoped-CSS pipeline. |

Use `Renderer` with a typed `RenderRequest` for every source-to-target operation. SVG request IDs can be normalized with `merman::svg::sanitize_svg_id`; dynamic integrations should use stable ASCII keys and ensure the normalized results are unique rather than deriving IDs only from display titles. The same request seam also carries layout, presentation, resource, pipeline, and cancellation policy.

## Run By Task

Render one standalone SVG or one HTML document containing multiple SVGs:

```sh
cargo run -p merman --example render_svg > diagram.svg
cargo run -p merman --example embed_multiple_svgs > diagrams.html
```

Reuse one renderer for several independent SVG files:

```sh
cargo run -p merman --example render_many -- target/example-svgs
```

Render PNG and terminal output. These capabilities are intentionally outside the default SVG feature set:

```sh
cargo run -p merman --features png --example render_png -- target/diagram.png
cargo run -p merman --features ascii --example render_terminal
cargo run -p merman --features ascii --example render_terminal -- --ascii
```

Inspect parser and layout results:

```sh
cargo run -p merman --example inspect_semantics
cargo run -p merman --example inspect_layout
cargo run -p merman --example deterministic_gantt
```

Configure host defaults, presentation, and output policy:

```sh
cargo run -p merman --example configure_mermaid > configured.svg
cargo run -p merman --example presentation_profile > presentation.svg
cargo run -p merman --example custom_presentation_theme > custom-theme.svg
cargo run -p merman --example custom_svg_pipeline > consumer-safe.svg
```

## Host Integration Recipes

Merman accepts explicit requests and returns artifacts or typed errors. The host owns terminal
capability detection, scrolling, UI actions, file storage, caching, scheduling, and process isolation.
These recipes compose existing APIs; they introduce no additional library preset or host framework.

| Host context | Inputs the host chooses | Existing API / example | Result to consume | Boundary |
| --- | --- | --- | --- | --- |
| Terminal display | Charset, display-cell width, family layout, overflow | `AsciiRequest`; [`render_terminal.rs`](render_terminal.rs) | `AsciiOutput.text` and extents | Host decides where/how to print or scroll. |
| Agent pipe or log | Plain encoding, explicit width, fallback permission | `AsciiOutput::report()`; [`render_agent_log.rs`](render_agent_log.rs) | One JSON object including text and metadata | Hard failures remain errors, not retry instructions. |
| Themed terminal | Explicit palette and supported color encoding | `AsciiTerminalPalette`; [`terminal_palette.rs`](terminal_palette.rs) | TrueColor text with the same logical layout | Palette detection and styled-to-plain retry belong to the host. |
| Browser/editor preview | Host theme, Mermaid overrides, diagram ID, SVG policy | `HostTheme`, `SvgRequest`; [`custom_presentation_theme.rs`](custom_presentation_theme.rs) | SVG artifact | Host performs DOM admission and insertion. |
| Image export | Fit box, scale, background, resource budget | `PngRequest`; [`render_png.rs`](render_png.rs) | Bytes and `RasterPlan` dimensions | Saving the example's file is application code after rendering. |

The existing APIs cover these scenarios. SVG themes and ASCII palettes have separate semantics;
there is no need to introduce a shared theme object to use the same application colors in both.

### Bounded terminal text and machine reports

The terminal examples use Sequence or Flowchart, the two families that currently admit `Auto`.
For a generic host, inspect the family's `layout_profiles`, `encodings`, and `fallback_encodings`
from the [capability matrix](../../../docs/rendering/ASCII_SUPPORT_MATRIX.md) before constructing a
request. Use Canonical for supported families without Auto. Capability admission does not promise
that every dense topology or feature will render; handle the returned typed error too.

```sh
cargo run -p merman --no-default-features --features ascii --example render_terminal
cargo run -p merman --no-default-features --features ascii --example render_terminal -- --ascii
cargo run -p merman --no-default-features --features ascii --example render_agent_log -- 80
cargo run -p merman --no-default-features --features ascii --example render_agent_log -- 40
```

`render_agent_log` takes a width in display cells and writes one schema-3 JSON report to stdout.
Its status message goes to stderr. The 40-column request demonstrates complete structured fallback;
80 columns fit the primary output. `requested_layout_profile` describes the request,
`layout_profile` the selected primary geometry, and `compact_attempted` the optional second layout.
`emitted_width`/`emitted_height` describe the text actually returned, including a fallback.
Use these fields rather than counting bytes or stripping ANSI to measure width.

A successful `Primary`, `WideAllowed`, or `Fallback` is different from a `RenderError`:

- `Ascii(WidthOverflow { .. })` reports the width refusal selected by `OverflowPolicy::Error`.
- `Cancelled(..)` includes cooperative cancellation/deadline termination.
- `ResourceLimitExceeded(..)` describes a quota refusal; it does not authorize a larger retry.
- Parse, invalid option, unsupported feature, and unavailable fallback errors must also reach the host.

The Rust example propagates errors before publishing stdout. For a machine CLI with an existing
versioned error envelope, use `merman-cli render --ascii-report`; do not parse the Rust example's
human stderr as a wire protocol. A source file requires `--output -` to direct the report to stdout:

```sh
merman-cli render diagram.mmd --format unicode --ascii-layout-profile auto --ascii-max-width 80 --ascii-overflow fallback --ascii-color plain --ascii-report --output -
```

`Allow` returns a complete primary result even when wide. `Error` returns a width error. `Fallback`
tries the admitted typed structured projection, which can still fail if it cannot fit or retain
required fields. It is not raw Mermaid source, clipping, or automatic recovery from resource limits.
Auto chooses between at most two primary layouts; a narrower candidate may be taller. It does not
promise to fit every diagram, bound the number of terminal rows, or wrap every edge label.

### Terminal colors

```sh
cargo run -p merman --no-default-features --features ascii --example terminal_palette
```

This example supplies an application-owned palette, selects TrueColor, and rejects width overflow.
It deliberately emits ANSI even when redirected: the host explicitly chose that encoding. For an
unknown terminal palette, ANSI16 uses terminal-default foreground/background and named accents;
it does not promise the exact RGB values supplied to `AsciiColorTheme`. ANSI256 approximates RGB,
while TrueColor encodes the supplied RGB roles. Color is supplementary to the text topology.

Viewport fallback and CLI report output currently require Plain. A styled request with `Fallback`
is invalid even if the particular diagram fits. A host may choose a separate Plain request after a
width refusal, but must keep that decision explicit and must not retry cancellation or quota errors.

Rust maps colors through `AsciiColorTheme::from_terminal_palette`. Bindings expose the palette as
`ascii.theme` in [Options JSON](../../../docs/bindings/OPTIONS_JSON.md). SVG uses the independent
[semantic presentation theme](../../../docs/rendering/presentation-themes.md); selecting one does
not synchronize the other or select an SVG pipeline.

### Browser preview and image artifacts

[`render_svg.rs`](render_svg.rs) demonstrates the default Mermaid-parity SVG target. For custom
host roles, use [`custom_presentation_theme.rs`](custom_presentation_theme.rs): that example explicitly
chooses resvg-safe output, so change the pipeline choice explicitly when adapting it for a browser.
Parity may contain HTML labels. `readable` is a specialized text overlay and can duplicate those
labels in a browser. `resvg-safe` addresses raster-consumer compatibility. None of these names replaces
host DOM admission; see the [SVG pipeline contract](../../../docs/rendering/SVG_OUTPUT_PIPELINE.md).

Use [`render_png.rs`](render_png.rs) for an optional high-fidelity image. `Renderer::render()` returns
bytes and a raster plan before the example writes a file. A host can inspect the dimensions, keep the
bytes in memory, or save them without adding file/viewer actions to the library. PNG/JPEG/PDF select
the export path; callers do not need to build a separate product rendering backend.

```sh
cargo run -p merman --no-default-features --features svg --example custom_presentation_theme > custom-theme.svg
cargo run -p merman --no-default-features --features png --example render_png -- target/diagram.png
```

For browser Workers, choose a [Web package](../../../platforms/web/README.md) with the required
capabilities. A string helper such as Web `renderAscii` is not a typed report API. The shipped
[native Node package](../../../platforms/node/README.md) has a different capability recipe and does
not include ASCII or PNG; workspace-wide support does not imply package availability.

Merman rendering is synchronous with cooperative checkpoints. A responsive editor or agent chooses
its own worker/thread/process and retains `OperationControl` for cancellation. A deadline cannot
interrupt an opaque call already in progress; hard interruption belongs to the host's isolation
boundary, as described in [ADR-0008](../../../docs/adr/0008-async-and-runtime.md).

## Copy Into An Application

These examples describe this source checkout. Auto layout and schema-3 ASCII reports are
unreleased changes here; do not assume they are available in `0.8.0-alpha.6`. To run the new
terminal examples in another crate, use a path dependency on this checkout:

```toml
[dependencies]
merman = { path = "/path/to/merman/crates/merman", default-features = false, features = ["ascii"] }
serde_json = "1" # Needed by render_agent_log.
```

For the published alpha.6 API, use examples from its matching release tag and dependency:

```toml
[dependencies]
merman = { version = "=0.8.0-alpha.6" }
```

Copy the relevant `.rs` file into your application's `examples/` directory and run it by filename:

```sh
cargo run --example render_svg
```

Enable `features = ["png"]` on the Merman dependency when copying `render_png`, or `features = ["ascii"]` when copying `render_terminal`, `render_agent_log`, or `terminal_palette`. Add `serde_json = "1"` when copying `inspect_semantics`, `inspect_layout`, `configure_mermaid`, `deterministic_gantt`, or `render_agent_log`.

## Minimize Features Later

The commands above favor a successful first run. Once the workflow is known, disable defaults and compile only the observable capabilities it needs:

| Examples | Minimal selection |
| --- | --- |
| `inspect_semantics`, `deterministic_gantt` | No Merman features |
| `render_svg`, `embed_multiple_svgs`, `render_many`, `inspect_layout`, `configure_mermaid`, `custom_presentation_theme`, `custom_svg_pipeline` | `svg` |
| `presentation_profile` | `layout-elk` (also enables `svg`) |
| `render_terminal`, `render_agent_log`, `terminal_palette` | `ascii` |
| `render_png` | `png` (also enables `svg`) |

For example:

```sh
cargo run -p merman --no-default-features --features svg --example render_svg > diagram.svg
cargo run -p merman --no-default-features --example inspect_semantics
cargo run -p merman --no-default-features --features png --example render_png
```

A minimal SVG build returns a typed `missing-capability` error when an input needs an optional layout engine or math renderer. It never silently substitutes a different semantic result. See the [capability guide](../../../docs/FEATURES.md) for dependency declarations and feature forwarding.

## Profiling

`profile_render.rs` is a maintainer tool rather than a learning example. It keeps the CPU inside a selected render stage for profilers:

```sh
CARGO_PROFILE_BENCH_DEBUG=true cargo flamegraph \
  --profile bench \
  -p merman \
  --no-default-features \
  --features layout-cytoscape \
  --example profile_render \
  -o target/bench/flamegraphs/profile_render_architecture_medium.svg \
  -- \
  --input crates/merman/benches/fixtures/architecture_medium.mmd \
  --stage render \
  --seconds 20
```
