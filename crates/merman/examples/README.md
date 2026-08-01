# Merman Rust Examples

Run these commands from the repository root.

Every command below disables the `merman` facade's default `complete-svg` feature and selects the smallest capability set required by that example. Remove `--no-default-features` when evaluating the default complete SVG experience instead.

Examples `01` through `08`, plus `11`, `12`, and `13`, accept Mermaid source on stdin. If stdin is an interactive terminal, they do not wait for input: they print a short note to stderr and render a built-in example. Pipe source or redirect a `.mmd` file to replace that input.

## Parse And Inspect

These examples need only the always-available parser and semantic model:

```bash
cargo run -p merman --no-default-features --example example_02_semantic_json
cargo run -p merman --no-default-features --example example_08_deterministic_gantt
```

## Basic SVG And Layout

The `svg` leaf provides the basic renderer without Cytoscape, ELK, or RaTeX:

```bash
cargo run -p merman --no-default-features --features svg \
  --example example_01_svg_basic > out.svg
cargo run -p merman --no-default-features --features svg \
  --example example_03_layout_json
cargo run -p merman --no-default-features --features svg \
  --example example_06_svg_pipeline > pipeline.svg
cargo run -p merman --no-default-features --features svg \
  --example example_07_theme_css > themed.svg
cargo run -p merman --no-default-features --features svg \
  --example example_09_multiple_diagrams
```

## Terminal And Binary Output

Output features imply the lower-level capabilities they require:

```bash
cargo run -p merman --no-default-features --features ascii \
  --example example_04_ascii_output
cargo run -p merman --no-default-features --features png \
  --example example_05_raster_output -- target/example.png
```

Pass `-- --ascii` to `example_04_ascii_output` for ASCII-only output.

## Host Output And Themes

These examples use only the basic SVG path. They demonstrate a custom output pipeline, a reusable host theme profile, and a stylized built-in host theme:

```bash
cargo run -p merman --no-default-features --features svg \
  --example example_11_custom_output_environment > host-preview.svg
cargo run -p merman --no-default-features --features svg \
  --example example_12_host_theme_profile > host-theme.svg
cargo run -p merman --no-default-features --features svg \
  --example example_13_stylized_theme_showcase > showcase.svg
```

## Custom Input

Pipe a Mermaid string:

```bash
printf "flowchart LR\nA --> B\n" | \
  cargo run -p merman --no-default-features --features svg \
    --example example_01_svg_basic > out.svg
```

Redirect a Mermaid file:

```bash
cargo run -p merman --no-default-features --features svg \
  --example example_06_svg_pipeline < fixtures/flowchart/basic.mmd > pipeline.svg
```

Render custom PNG output:

```bash
printf "flowchart LR\nA --> B\n" | \
  cargo run -p merman --no-default-features --features png \
    --example example_05_raster_output -- target/example.png
```

## Output Paths

- `example_01`, `example_06`, `example_07`, `example_12`, and `example_13` write SVG to stdout.
- `example_11` writes host-controlled resvg-safe SVG to stdout.
- `example_02`, `example_03`, and `example_08` write JSON to stdout.
- `example_04` writes terminal text to stdout.
- `example_05` writes PNG to `target/merman-raster-example.png` by default, or to the path passed after `--`.
- `example_09` writes SVG files to `target/merman-multiple-diagrams/`.
- `profile_render` writes a profiling summary to stderr and is intended for CPU profilers.

## Profiling

Use `profile_render` when a profiler needs a long, single-stage loop instead of a Criterion benchmark harness. The example parses and lays out the input once for `--stage render`, then keeps the CPU inside SVG rendering for the requested duration.

```bash
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

The Architecture fixture requires `layout-cytoscape`, which already implies `svg`. For another fixture, select its actual optional layout or math leaf instead of relying on the facade default.
