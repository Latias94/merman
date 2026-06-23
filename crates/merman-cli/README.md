# merman-cli

[![Crates.io](https://img.shields.io/crates/v/merman-cli.svg)](https://crates.io/crates/merman-cli)
[![Documentation](https://docs.rs/merman-cli/badge.svg)](https://docs.rs/merman-cli)
[![Crates.io Downloads](https://img.shields.io/crates/d/merman-cli.svg)](https://crates.io/crates/merman-cli)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange.svg)](https://www.rust-lang.org)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

`merman-cli` is a browserless Mermaid command-line renderer for SVG, PNG, JPG, PDF, and
ASCII/Unicode text output. The top-level command functionally mirrors common `mmdc` workflows,
while developer subcommands expose merman's parse, layout, and render internals.

## Install

```sh
cargo install merman-cli
```

The default binary includes SVG/PNG/JPG/PDF export, ASCII/Unicode text output, and RaTeX math
rendering.

This crate installs `merman-cli`, not `mmdc`.

From a local checkout:

```sh
cargo install --path crates/merman-cli
```

## Quick Start

Top-level usage mirrors common `mmdc` workflows:

```sh
merman-cli -i diagram.mmd -o diagram.svg
merman-cli -i diagram.mmd -o diagram.png -t dark -b transparent
merman-cli -i diagram.mmd -o diagram.pdf --pdfFit
merman-cli -i diagram.mmd -o -
```

`-` reads from stdin or writes to stdout:

```sh
printf "flowchart TD\nA[API] --> B[DB]\n" | merman-cli -i - -o -
printf "flowchart TD\nA[API] --> B[DB]\n" | merman-cli -o out.svg
```

When `-o` is omitted, top-level mode writes `<input>.svg` for file input and `out.svg` for stdin.
The output format is inferred from the output extension unless `-e, --outputFormat, --format` is
provided.

## Output Formats

| Format | Top-level extension | Status |
|---|---|---|
| SVG | `.svg` | Default, browserless renderer |
| PNG | `.png` | Rust raster output |
| PDF | `.pdf` | Rust PDF output through SVG conversion |
| JPG/JPEG | `.jpg`, `.jpeg` | Rust extension beyond upstream `mmdc` |
| ASCII | `.txt`, `.ascii` | Rust extension, enabled by default |
| Unicode | `.txt`, `.ascii` | Rust extension, enabled by default |

SVG output uses the Mermaid-parity contract. PNG, JPG, and PDF output use the export contract: the
CLI applies the `resvg-safe` SVG pipeline before raster/PDF conversion so strict headless renderers
do not have to understand Mermaid HTML labels in `<foreignObject>`.

Examples:

```sh
merman-cli -i diagram.mmd -o diagram.svg
merman-cli -i diagram.mmd -o diagram.png
merman-cli -i diagram.mmd -o diagram.jpg
merman-cli -i diagram.mmd -o diagram.pdf
merman-cli -i diagram.mmd -o diagram.txt -e unicode
```

## Markdown Input

`.md` and `.markdown` input files activate Markdown mode. Mermaid code blocks are extracted,
rendered as numbered artefacts, and optionally rewritten back into Markdown image links.

```sh
merman-cli -i README.md -o README.svg
```

The command above writes `README-1.svg`, `README-2.svg`, and so on. The template output file itself
is not written unless the output path is Markdown.

```sh
merman-cli -i README.md -o README.rendered.md
```

The command above writes numbered SVG artefacts and rewrites Mermaid fences in
`README.rendered.md` to Markdown image links.

Use `--artefacts` or the Rust-friendly `--artifacts` alias to place images in a separate directory:

```sh
merman-cli -i docs/input.md -o docs/output.md --artifacts docs/assets
```

Use `--jobs` to bound parallel chart rendering. Results are still linked in source order:

```sh
merman-cli -i docs/input.md -o docs/output.md --jobs 4
```

Markdown mode does not support stdout output because it may need to write multiple artefact files.

## Icon Packs

Iconify packs are loaded into a Rust SVG icon registry, so flowchart and architecture icon nodes can
embed real icon SVGs without a browser.

Load an Iconify package name:

```sh
merman-cli -i diagram.mmd -o diagram.svg --iconPacks @iconify-json/logos
```

`merman-cli` first looks for `node_modules/@iconify-json/logos/icons.json` from the current working
directory upward. If no local package is found, it fetches
`https://unpkg.com/@iconify-json/logos/icons.json`.

Load an explicit prefix and source:

```sh
merman-cli -i diagram.mmd -o diagram.svg --iconPacksNamesAndUrls logos#icons.json
merman-cli -i diagram.mmd -o diagram.svg --iconPacksNamesAndUrls logos#file:///tmp/icons.json
merman-cli -i diagram.mmd -o diagram.svg --iconPacksNamesAndUrls logos#https://example.com/icons.json
```

The prefix before `#` overrides the JSON prefix, matching the useful part of upstream loader
registration while keeping rendering browserless.

## Rust Extensions

### ASCII/Unicode

ASCII/Unicode output is enabled in the default CLI binary:

```sh
printf "flowchart LR\nA --> B\n" | merman-cli -i - -o out.txt -e ascii
printf "classDiagram\nclass Animal\n" | merman-cli render --format unicode -
printf "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello\n" | \
  merman-cli render --format unicode --sequence-mirror-actors -
```

Terminal text rendering currently supports flowchart/graph, sequenceDiagram, classDiagram,
erDiagram, and xychart. Other diagram families still render to SVG/raster formats but return an
unsupported-diagram error for ASCII/Unicode until a typed text renderer is added.

ClassDiagram and erDiagram text output include class/entity boxes, layered relationship layouts,
same-endpoint lanes, simple spanning side lanes, and unrelated standalone components. Cyclic or
denser relationship graphs return explicit diagnostics instead of silently dropping edges.

### RaTeX Math

RaTeX math rendering is enabled by default:

```sh
printf "flowchart LR\nA[\"$$x^2$$\"] --> B\n" | merman-cli render --math-renderer ratex -
```

Use `--no-default-features` only when you intentionally want to exclude default binary capabilities
such as RaTeX and ASCII/Unicode. In that build, `--math-renderer ratex` remains unavailable unless
the `ratex-math` feature is enabled explicitly, and ASCII/Unicode output remains unavailable unless
the `ascii` feature is enabled explicitly.

### Developer Subcommands

Top-level mode is for `mmdc`-style export workflows. Developer subcommands remain available for
tooling, tests, and debugging:

```sh
merman-cli detect path/to/diagram.mmd
merman-cli parse --pretty --meta path/to/diagram.mmd
merman-cli layout --pretty path/to/diagram.mmd
merman-cli render path/to/diagram.mmd --out out.svg
merman-cli render --format png --out out.png path/to/diagram.mmd
merman-cli render --format jpg --out out.jpg path/to/diagram.mmd
merman-cli render --format pdf --out out.pdf path/to/diagram.mmd
merman-cli completion bash
```

`completion` emits shell completion scripts for `merman-cli`.

`render` writes SVG to stdout by default. Use `--out` for files, `--format ascii|unicode` for
terminal text, and `--format png|jpg|pdf` for raster or PDF export.

### Lint

`lint` analyzes Mermaid source and emits canonical diagnostics JSON by default:

```sh
merman-cli lint path/to/diagram.mmd
merman-cli lint --markdown path/to/README.md
printf "flowchart TD\nA -->\n" | merman-cli lint --format text - 
printf "```mermaid\nflowchart TD\nA -->\n```" | \
  merman-cli lint --markdown --stdin-file-name notes.md --format text -
```

Use `--format text` for a compact human-readable summary or `--format json` for machine
consumers. Markdown and MDX input files are scanned for Mermaid fences, and `--stdin-file-name`
provides a stable display path when linting from stdin.

## Common Options

- `-t, --theme <theme>` sets the Mermaid theme.
- `-w, --width <width>` and `-H, --height <height>` configure viewport-sensitive layouts.
- `-b, --backgroundColor <color>` sets SVG/raster background color.
- `-c, --configFile <file>` loads a Mermaid JSON object configuration file.
- `-C, --cssFile <file>` injects CSS into SVG output before export.
- `-I, --svgId <id>` sets the root SVG id and marker id prefix.
- `-s, --scale <n>` controls PNG/JPG raster scale.
- `--raster-fit-width <px>` and `--raster-fit-height <px>` fit PNG/JPG output to a
  browser-like preview box before applying `--scale`.
- `--raster-max-width <px>`, `--raster-max-height <px>`, and `--raster-max-pixels <n>` set the
  PNG/JPG pixmap budget. Defaults are `8192 x 8192` and `8192*8192` total pixels.
- `--raster-unbounded` disables the PNG/JPG pixmap budget for trusted oversized exports.
- `-f, --pdfFit` uses a chart-sized PDF page instead of the top-level default Letter-sized page.
- `-q, --quiet` suppresses non-error logs.
- `--text-measurer deterministic|vendored` controls text measurement.
- `--math-renderer none|ratex` controls math label rendering.
- `--flowchart-elk-backend source-ported|compat` selects the Flowchart ELK backend. The default
  source-ported backend follows the pinned Mermaid adapter and Eclipse ELK layered port; `compat`
  keeps the older lightweight alpha fallback available for diagnostics.
- `--suppress-errors` emits an error diagram instead of failing on parse errors.
- `--fixed-today <YYYY-MM-DD>` fixes the local "today" date for time-dependent diagrams such as
  Gantt.
- `--fixed-local-offset-minutes <minutes>` fixes the local timezone offset for deterministic
  local-time parsing and rendering.
- `--hand-drawn-seed <n>` stabilizes rough/hand-drawn rendering where supported.

## SVG Input Rasterization

`merman-cli render --format png|jpg|pdf` can rasterize existing SVG input when the input starts with
`<svg`. Treat raw SVG files as trusted input: this mode is for converting SVGs you already chose to
process, not for accepting arbitrary uploaded SVG from untrusted users.

```sh
merman-cli render --format png --out diagram.png diagram.svg
```

Raw SVG input uses a separate raster boundary from Mermaid source rendering. The CLI applies
merman's `resvg`-safe SVG cleanup before CLI background/CSS postprocessing, then the raster/PDF
converter applies its normal safety cleanup and size limits before conversion.

Large Mermaid SVGs can be valid and still unsafe to rasterize at their intrinsic viewBox size.
Browsers usually paint the vector SVG inside a visible container; they do not have to allocate one
full-size pixmap up front. For preview-like PNG/JPG output, pass `--raster-fit-width` and/or
`--raster-fit-height` plus `--scale` for device-pixel ratio. For export-like output, the default
pixmap budget prevents accidental oversized allocations. PDF export uses the same intrinsic SVG
size budget before vector conversion. Use `--raster-unbounded` only when that memory or conversion
cost is intentional.

## Compatibility Notes

`merman-cli` is browserless. It does not start Puppeteer, Chromium, or a Mermaid browser runtime.

For script compatibility with `mmdc`, `--puppeteerConfigFile` is accepted, the referenced file must
exist, and its contents must be valid JSON. The parsed values are intentionally ignored because this
renderer has no Puppeteer runtime to configure.

PDF output is generated through Rust SVG conversion rather than Chromium print-to-PDF, so it is not
intended to be pixel-identical to browser PDF output. The top-level default approximates the
upstream default page behavior; `--pdfFit` emits a chart-sized page.

The repository tracks the detailed `mmdc` compatibility matrix in
`docs/alignment/CLI_COMPATIBILITY.md`. For migration, replace the command name with
`merman-cli`; the repo does not install a second `mmdc` binary.
