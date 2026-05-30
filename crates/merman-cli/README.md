# merman-cli

`merman-cli` is the command-line interface for [merman](https://crates.io/crates/merman). It can detect, parse, layout, and render Mermaid diagrams without a browser.

## Install

```sh
cargo install merman-cli
```

From a local checkout:

```sh
cargo install --path crates/merman-cli
```

## Commands

```sh
merman-cli detect path/to/diagram.mmd
merman-cli parse --pretty --meta path/to/diagram.mmd
merman-cli layout --pretty path/to/diagram.mmd
merman-cli render path/to/diagram.mmd --out out.svg
merman-cli render --format unicode path/to/diagram.mmd
merman-cli render --format ascii path/to/diagram.mmd
merman-cli render --format png --out out.png path/to/diagram.mmd
merman-cli render --format jpg --out out.jpg path/to/diagram.mmd
merman-cli render --format pdf --out out.pdf path/to/diagram.mmd
```

If no input path is provided, or the input path is `-`, `merman-cli` reads Mermaid source from stdin.

```sh
printf "flowchart TD\nA[API] --> B[DB]\n" | merman-cli render --out out.svg
```

## Rendering Options

`render` writes SVG to stdout by default. Use `--out` for files, `--format ascii|unicode` for terminal text, and `--format png|jpg|pdf` for raster or PDF export.

Useful flags:

- `--text-measurer deterministic|vendored` controls text measurement. `vendored` is better for visual output; `deterministic` is useful for stable fixture-style output.
- `--math-renderer none|ratex` enables optional `$$...$$` math rendering. `ratex` requires the `ratex-math` Cargo feature; Flowchart and Sequence support math-only labels plus single-formula prose/math labels.
- `--id <diagram-id>` sets the root SVG id and internal marker id prefix.
- `--scale <n>` controls PNG/JPG raster scale.
- `--background <css-color>` sets raster background.
- `--hand-drawn-seed <n>` stabilizes rough/hand-drawn rendering where supported.
- `--viewport-width <w>` and `--viewport-height <h>` configure viewport-sensitive layouts.
- `--suppress-errors` emits an error diagram instead of failing on parse errors.

ASCII/Unicode output is feature-gated in the Rust package:

```sh
printf "flowchart LR\nA --> B\n" | cargo run -p merman-cli --features ascii -- render --format ascii -
printf "classDiagram\nclass Animal\n" | cargo run -p merman-cli --features ascii -- render --format unicode -
```

With `--features ascii`, terminal text rendering currently supports flowchart/graph,
sequenceDiagram, classDiagram, erDiagram, and xychart. Other diagram families remain available for
SVG/raster rendering but return an unsupported-diagram error for `--format ascii|unicode` until a
typed text renderer is added.

ClassDiagram and erDiagram text output include class/entity boxes plus layered chain/star
relationship layouts. Denser, crossing, cyclic, parallel, or unrelated relationship graphs return
explicit diagnostics instead of silently dropping edges.

RaTeX math rendering is also feature-gated:

```sh
printf "flowchart LR\nA[\"$$x^2$$\"] --> B\n" | cargo run -p merman-cli --features ratex-math -- render --math-renderer ratex -
```

## SVG Input Rasterization

`merman-cli render --format png|jpg|pdf` can also rasterize existing SVG input when the input starts with `<svg`.

```sh
merman-cli render --format png --out diagram.png diagram.svg
```

The raster path applies merman's `resvg`-safe SVG cleanup before conversion.
