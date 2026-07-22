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

`-` reads from stdin or writes to stdout. stdout is reserved for requested payload bytes only;
warnings, progress, and other diagnostics are written to stderr.

```sh
printf "flowchart TD\nA[API] --> B[DB]\n" | merman-cli -i - -o -
printf "flowchart TD\nA[API] --> B[DB]\n" | merman-cli -o out.svg
```

When `-o` is omitted, top-level mode writes `<input>.svg` for file input and `out.svg` for stdin.
The output format is inferred from the output extension unless `-e, --outputFormat, --format` is
provided.

## Command Surfaces

The top-level command is an `mmdc` replacement and a strict superset. Existing calls such as
`merman-cli -i input.mmd -o output.svg` do not need any Merman-specific flag. Optional native
controls make resource policy and browserless behavior explicit without changing that common path.

| Surface | Contract | Help groups |
| --- | --- | --- |
| Top-level compatible export | Common `mmdc` input, output, theme, config, Markdown, icon-pack, and PDF-fit workflows under the `merman-cli` command name. | `mmdc-compatible export`, `Markdown batch export`, `Mermaid configuration`, `Accepted browser compatibility flags`, `Icon packs` |
| Optional Merman superset | Renderer selection plus independently scoped PNG/JPG, vector-PDF, embedded-image, and aggregate-memory budgets. | `Merman renderer controls`, `Merman raster controls`, `Merman PDF controls`, `Merman embedded-image controls`, `Merman resource controls` |
| `render` subcommand | An explicit Rust-native surface for selecting SVG/PNG/JPG/PDF/ASCII/Unicode output and developer-oriented controls. | `Render input and output` plus the applicable Merman control groups |

`Deterministic rendering` and `Text output` are additional Merman groups shared where their options
apply. Run `merman-cli --help` or `merman-cli render --help` for the exact current surface.

## Output Formats

| Format | Top-level extension | Status |
|---|---|---|
| SVG | `.svg` | Default, browserless renderer |
| PNG | `.png` | Rust raster output |
| PDF | `.pdf` | Rust vector PDF output through SVG conversion |
| JPG/JPEG | `.jpg`, `.jpeg` | Rust extension beyond upstream `mmdc` |
| ASCII | `.txt`, `.ascii` | Rust extension, enabled by default |
| Unicode | `.txt`, `.ascii` | Rust extension, enabled by default |

SVG output uses the Mermaid-parity contract. PNG, JPG, and PDF use the export contract: the CLI
applies the `resvg-safe` SVG pipeline before conversion so strict headless consumers do not have to
understand Mermaid HTML labels in `<foreignObject>`. PNG/JPG and PDF deliberately use different
sizing policies: PNG/JPG allocate a bounded pixmap, while PDF retains vector geometry and only
budgets localized filter bitmaps and embedded raster images. If you need SVG bytes with the same
export-safe cleanup, request it explicitly with `--svg-pipeline resvg-safe`.

Examples:

```sh
merman-cli -i diagram.mmd -o diagram.svg
merman-cli -i diagram.mmd -o diagram.svg --svg-pipeline resvg-safe
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

Iconify packs are loaded into a Rust SVG icon registry, so Flowchart, Architecture, and TreeView
nodes can embed real icon SVGs without a browser.

Load an Iconify package name:

```sh
merman-cli -i diagram.mmd -o diagram.svg --iconPacks @iconify-json/logos
```

`merman-cli` first looks for `node_modules/@iconify-json/logos/icons.json` from the current working
directory upward. The default is offline-first: if no local package is found, the command fails
with migration guidance instead of fetching from the network. Pass `--allow-network` when you
intentionally want the package to be fetched from `https://unpkg.com/@iconify-json/logos/icons.json`.

Load an explicit prefix and source:

```sh
merman-cli -i diagram.mmd -o diagram.svg --iconPacksNamesAndUrls logos#icons.json
merman-cli -i diagram.mmd -o diagram.svg --iconPacksNamesAndUrls logos#file:///tmp/icons.json
merman-cli -i diagram.mmd -o diagram.svg --allow-network --iconPacksNamesAndUrls logos#https://example.com/icons.json
```

The prefix before `#` overrides the JSON prefix, matching the useful part of upstream loader
registration while keeping rendering browserless. Any HTTP(S) icon pack source requires
`--allow-network`; local paths and `file://` sources do not.

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
the `math` feature is enabled explicitly, and ASCII/Unicode output remains unavailable unless
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
merman-cli lint-rules --format json --pretty
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

The default lint profile is `core`, which reports syntax, compatibility, resource, and internal
diagnostics without enabling Merman authoring recommendations. Use `--lint-profile recommended` or
`--enable-rule <RULE_ID>` to opt into authoring hints such as
`merman.authoring.config.prefer_init_directive`,
`merman.authoring.config.prefer_frontmatter_config`, and
`merman.authoring.flowchart.explicit_direction`.

`lint-rules` lists the governed rule catalog used by the analyzer:

```sh
merman-cli lint-rules
merman-cli lint-rules --format json --pretty
merman-cli lint-rules --configurable --format json
```

JSON output is a versioned response object with `{ "version": 1, "rules": [...] }`. Each rule
exposes its id, evidence references, default severity, profile, origin, configurability, and
fixability so CLI, editor, and LSP integrations can present the same rule facts. The `origin` field
is intentional: Mermaid syntax and compatibility rules are separated from Merman authoring
recommendations, and the default `core` profile does not enable Merman authoring rules.

## Common Options

`--help` keeps compatible and Merman-specific controls in the groups described above.

- `-t, --theme <theme>` sets the Mermaid theme.
- `-w, --width <width>` and `-H, --height <height>` configure viewport-sensitive layouts.
- `-b, --backgroundColor <color>` sets SVG/PNG/JPG/PDF background color.
- `-c, --configFile <file>` loads a Mermaid JSON object configuration file.
- `-C, --cssFile <file>` injects CSS into SVG output before export.
- `-I, --svgId <id>` sets the root SVG id and marker id prefix.
- `-s, --scale <n>` controls PNG/JPG raster scale.
- `--raster-fit-width <px>` and `--raster-fit-height <px>` fit PNG/JPG output to a
  browser-like preview box before applying `--scale`.
- `--raster-max-width <px>`, `--raster-max-height <px>`, and `--raster-max-pixels <n>` set the
  PNG/JPG pixmap budget. Defaults are 4096 pixels per side and 16,777,216 total pixels.
- `--raster-unbounded` disables the PNG/JPG pixmap budget for trusted oversized exports.
- `--pdf-filter-scale <n>` sets localized SVG-filter sampling for vector PDF output; the default is
  `4`.
- `--pdf-max-filter-pixels <n>` sets the aggregate PDF filter bitmap budget; the default is
  `33,554,432` pixels.
- `--pdf-filter-unbounded` disables only the PDF filter bitmap budget for trusted input.
- `--embedded-image-max-pixels <n>` and `--embedded-image-max-total-pixels <n>` set embedded image
  decode budgets used by PNG/JPG and PDF. Defaults are `16,777,216` per image and `33,554,432`
  total pixels.
- `--embedded-images-unbounded` disables only embedded raster image decode budgets for trusted
  input.
- `--encoding-memory-budget-mib <mib>` bounds aggregate in-flight image encoding memory for
  Markdown jobs; the default is `512` MiB. Scheduling weights include the native SVG backend's
  bounded 8 MiB worker stack.
- `-f, --pdfFit` (alias `--pdf-fit`) replaces the top-level 612-by-792-point fixed page with CSS
  viewport sizing. The responsive chart width is limited by `--width` (800 CSS pixels by default),
  then converted at 72 PDF points per 96 CSS pixels while preserving aspect ratio.
- When a PNG/JPG request is automatically constrained, the CLI prints its requested and final
  pixel dimensions to stderr. It similarly reports any automatic reduction in PDF filter sampling;
  `--quiet` suppresses both informational messages.
- `-q, --quiet` suppresses non-error logs.
- Runtime failures use categorized exit statuses: `1` for render/runtime failures, `2` for
  invalid input/config/output CLI contracts, and `3` for direct I/O failures. Broken stdout pipes
  are treated as normal pipeline termination and do not print a generic I/O diagnostic.
- `--text-measurer deterministic|vendored` controls text measurement.
- `--resource-profile interactive|constrained|trusted-native|unbounded-for-trusted-input`
  selects semantic/SVG work budgets. The CLI defaults to `trusted-native` for local `mmdc`
  workloads; use `interactive` only for cooperative local editing and `constrained` for untrusted,
  public, or multi-tenant input. This does not alter PNG/JPG, PDF-filter, embedded-image, or
  aggregate encoding budgets.
- `--math-renderer none|ratex` controls math label rendering.
- `--svg-pipeline parity|readable|resvg-safe` selects the SVG output contract for SVG files.
  Raster/PDF formats keep the built-in `resvg-safe` export path.
- `--suppress-errors` emits an error diagram instead of failing on parse errors.
- `--fixed-today <YYYY-MM-DD>` fixes the local "today" date for time-dependent diagrams such as
  Gantt.
- `--fixed-local-offset-minutes <minutes>` fixes the local timezone offset for deterministic
  local-time parsing and rendering.
- `--hand-drawn-seed <n>` stabilizes rough/hand-drawn rendering where supported.

## SVG Input Export

`merman-cli render --format png|jpg|pdf` can convert existing SVG input when the input starts with
`<svg`. Treat raw SVG files as trusted input: this mode is for converting SVGs you already chose to
process, not for accepting arbitrary uploaded SVG from untrusted users.

```sh
merman-cli render --format png --out diagram.png diagram.svg
```

Raw SVG input uses a separate export boundary from Mermaid source rendering. The CLI applies
Merman's `resvg`-safe SVG cleanup before CLI background/CSS postprocessing, then prepares the
format-specific allocation plan before conversion.

Large Mermaid SVGs can be valid and still unsafe to rasterize at their intrinsic viewBox size.
Browsers usually paint the vector SVG inside a visible container; they do not have to allocate one
full-size pixmap up front. For preview-like PNG/JPG output, pass `--raster-fit-width` and/or
`--raster-fit-height` plus `--scale` for device-pixel ratio. For export-like output, the default
pixmap budget prevents accidental oversized allocations. SVG itself has no global width or height
cap, and vector PDF pages do not share the PNG/JPG pixel budget. PDF instead applies the independent
filter and embedded-image budgets listed above. Each `--*-unbounded` flag disables only its named
resource boundary and should be used only when that cost is intentional. Resvg-safe SVG, PNG/JPG,
and PDF also retain a non-optional recursive-tree capability: 256 resolved levels on native builds
and 64 on WebAssembly. Raw SVG output remains available for valid diagrams beyond that backend
boundary.

## Compatibility Notes

`merman-cli` is browserless. It does not start Puppeteer, Chromium, or a Mermaid browser runtime.

For script compatibility with `mmdc`, `--puppeteerConfigFile` is accepted, the referenced file must
exist, and its contents must be valid JSON. The parsed values are intentionally ignored because this
renderer has no Puppeteer runtime to configure.

PDF output is generated through Rust vector SVG conversion rather than Chromium print-to-PDF, so it
is not intended to be pixel-identical to browser PDF output. The top-level default uses a
612-by-792-point Letter approximation. `--pdfFit` follows the upstream CSS viewport concept: it
uses the responsive SVG width inside the configured CSS-pixel container and converts CSS pixels to
PDF points at the standard 96-to-72 ratio.

The repository tracks the detailed `mmdc` compatibility matrix in
`docs/alignment/CLI_COMPATIBILITY.md`. For migration, replace the command name with
`merman-cli`; the repo does not install a second `mmdc` binary.
