# merman-cli

[![Crates.io](https://img.shields.io/crates/v/merman-cli.svg)](https://crates.io/crates/merman-cli) [![Documentation](https://docs.rs/merman-cli/badge.svg)](https://docs.rs/merman-cli) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

A browserless Mermaid command-line renderer for SVG, PNG, JPEG, PDF, ASCII, and Unicode output. The top-level command follows common `mmdc` input and output conventions; parser, layout, lint, and inspection subcommands support development tooling.

<!-- BEGIN GENERATED RELEASE README CLI_PACKAGE_NOTICE -->

> **Source candidate:** this README targets `0.8.0-alpha.4`, which is not published yet. Install it from the repository until the matching release reaches crates.io.

<!-- END GENERATED RELEASE README CLI_PACKAGE_NOTICE -->

## Install

Install the complete CLI:

<!-- BEGIN GENERATED RELEASE README CLI_PACKAGE_INSTALL -->

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-cli
```

<!-- END GENERATED RELEASE README CLI_PACKAGE_INSTALL -->

From a local checkout:

```sh
cargo install --path crates/merman-cli
```

The installed command is `merman-cli`, not `mmdc`.

## First Render

```sh
merman-cli -i diagram.mmd -o diagram.svg
merman-cli -i diagram.mmd -o diagram.png -t dark -b transparent
printf 'flowchart LR\n  Source --> Merman --> SVG\n' | merman-cli -i - -o -
```

`-` means stdin or stdout. Payload bytes go to stdout; diagnostics and progress go to stderr. When `-o` is omitted, file input produces `<input>.svg` and stdin produces `out.svg`.

## Common Workflows

| Task | Command |
| --- | --- |
| Mermaid to SVG | `merman-cli -i diagram.mmd -o diagram.svg` |
| Mermaid to PNG/JPEG | `merman-cli -i diagram.mmd -o diagram.png` |
| Mermaid to vector PDF | `merman-cli -i diagram.mmd -o diagram.pdf --pdfFit` |
| Terminal output | `merman-cli render --format unicode diagram.mmd` |
| Markdown fences to assets and links | `merman-cli -i README.md -o README.rendered.md --artifacts docs/assets` |
| Human-readable diagnostics | `merman-cli lint --format text diagram.mmd` |
| Machine-readable analysis | `merman-cli lint --format json diagram.mmd` |
| Inspect the compiled binary | `merman-cli capabilities --json` |

The output extension selects the format unless `-e`, `--outputFormat`, or `--format` overrides it.

## Choose A Build

The default binary includes rendering, all export formats, analysis, ASCII/Unicode, both optional layout engines, math, Markdown conversion, offline Iconify loading, opt-in network Iconify retrieval, parallel Markdown work, shell completions, and native runtime adapters.

Cargo features are additive. Any artifact that must exclude capabilities needs `--no-default-features` and one explicit leaf set.

| Build | Intended use |
| --- | --- |
| Default | Complete local `mmdc`-style CLI |
| `--no-default-features --features analysis` | Detection, parsing, linting, fixes, and rule metadata without rendering |
| `--no-default-features --features svg` | Basic deterministic SVG without optional layouts or math |
| `--no-default-features --features markdown` | Sequential Markdown conversion without analysis |
| `--no-default-features --features icons` | Offline local Iconify packs without an HTTP client |

Install a slim lint binary:

<!-- BEGIN GENERATED RELEASE README CLI_PACKAGE_LEAN_INSTALL -->

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-cli \
  --no-default-features --features analysis
```

<!-- END GENERATED RELEASE README CLI_PACKAGE_LEAN_INSTALL -->

Public capability leaves are `analysis`, `svg`, `ascii`, `png`, `jpeg`, `pdf`, `layout-cytoscape`, `layout-elk`, `math`, `icons`, `markdown`, `network-icons`, `parallel-markdown`, `shell-completions`, and the four `system-*` adapters. Output, layout, and math features imply `svg` where required. Use `capabilities --json` as the authoritative contract for the binary you actually built.

## Runtime And Resource Policy

Every command defaults to deterministic runtime state, even when the binary contains native adapters. Deterministic mode uses a fixed clock, UTC time zone, operation-owned randomness, and no timing instrumentation.

Select native clock, time-zone rules, and operating-system randomness explicitly:

```sh
merman-cli -i diagram.mmd -o diagram.svg --runtime native
merman-cli lint --runtime native diagram.mmd
```

Native mode requires `system-clock`, `system-timezone`, and `system-random`. Timing is separate and requires both `system-timing` and `--system-timing`.

Choose a resource profile according to input trust:

| Profile | Use |
| --- | --- |
| `constrained` | Untrusted, public, or multi-tenant input |
| `interactive` | Cooperative local editing |
| `trusted-native` | Local automation; CLI default |
| `unbounded-for-trusted-input` | Explicitly trusted workloads that own the cost |

PNG/JPEG allocation, PDF filter sampling, embedded image decoding, and parallel encoding memory have separate bounds. Inspect `merman-cli --help` before changing an `--*-unbounded` control; each disables only its named boundary.

## Output Contracts

| Format | Contract |
| --- | --- |
| SVG | Mermaid-parity SVG by default; `--svg-pipeline readable` and `resvg-safe` are available |
| PNG/JPEG | Bounded bitmap export through the `resvg-safe` SVG path |
| PDF | Vector SVG conversion with independent filter and embedded-image budgets |
| ASCII/Unicode | Typed terminal projection for supported families |

```sh
merman-cli -i diagram.mmd -o diagram.svg --svg-pipeline readable
merman-cli -i diagram.mmd -o diagram.jpg
merman-cli -i diagram.mmd -o diagram.pdf
merman-cli -i diagram.mmd -o diagram.txt -e unicode
```

Raster and PDF export are not Chromium screenshots. PNG/JPEG allocate a bounded pixmap; PDF keeps vector geometry and budgets only localized raster work. Use `--raster-fit-width`, `--raster-fit-height`, and `--scale` for preview-sized bitmap output.

ASCII support is family-specific and may be full, partial, or a deliberate text summary. See the [ASCII/Unicode support matrix](https://github.com/Latias94/merman/blob/main/docs/rendering/ASCII_SUPPORT_MATRIX.md) rather than assuming every SVG family has a terminal projection.

## Markdown Documents

`.md`, `.markdown`, and `.mdx` inputs activate Markdown mode when the `markdown` capability is compiled.

```sh
merman-cli -i README.md -o README.rendered.md --artifacts docs/assets
merman-cli -i docs/input.md -o docs/output.md --jobs 4
```

An SVG output template such as `README.svg` produces numbered assets (`README-1.svg`, `README-2.svg`, and so on). A Markdown output path also rewrites Mermaid fences to image links. Markdown mode cannot write to stdout because one document may produce multiple files.

`parallel-markdown` adds `--jobs`; results remain linked in source order.

## Analysis And Tooling

The `analysis` capability enables parser-backed diagnostics, fixes, and the governed lint catalog:

```sh
merman-cli detect diagram.mmd
merman-cli parse --pretty --meta diagram.mmd
merman-cli layout --pretty diagram.mmd
merman-cli lint --format text diagram.mmd
merman-cli lint --markdown README.md
merman-cli lint-rules --configurable --format json
```

The default `core` lint profile reports syntax, compatibility, resource, and internal diagnostics. `--lint-profile recommended` adds opt-in Merman authoring guidance. JSON output uses the versioned canonical diagnostic and rule metadata contracts shared with editor integrations.

The explicit `render` subcommand exposes Rust-native output controls:

```sh
merman-cli render --format svg --out out.svg diagram.mmd
merman-cli render --format png --out out.png diagram.mmd
merman-cli render --format unicode diagram.mmd
```

Builds with `shell-completions` can emit completion scripts through `merman-cli completion <shell>`.

## Icon Packs

The `icons` capability loads local Iconify JSON for Flowchart, Architecture, and TreeView without a browser:

```sh
merman-cli -i diagram.mmd -o diagram.svg --iconPacks @iconify-json/logos
merman-cli -i diagram.mmd -o diagram.svg --iconPacksNamesAndUrls logos#icons.json
```

Package names are resolved from `node_modules` upward from the current directory. Local paths and `file://` URLs stay offline. HTTP(S) requires a binary built with `network-icons` and an explicit `--allow-network`; the CLI never silently downloads a missing pack.

## Math And Host Overrides

The default binary includes RaTeX. A slim build must add `math` before selecting it:

```sh
printf 'flowchart LR\nA["$$x^2$$"] --> B\n' | \
  merman-cli render --math-renderer ratex -
```

Useful host overrides include `--fixed-today`, `--fixed-local-offset-minutes`, `--hand-drawn-seed`, `--text-measurer`, and `--svgId`. These preserve deterministic defaults while letting one operation supply explicit environment values.

## Existing SVG Input

`merman-cli render --format png|jpg|pdf` can convert an input beginning with `<svg`:

```sh
merman-cli render --format png --out diagram.png diagram.svg
```

Treat this as a trusted-input conversion boundary. Arbitrary uploaded SVG can contain expensive trees, images, filters, or resources even when the final output is a bitmap.

## mmdc Compatibility

Merman does not start Puppeteer, Chromium, or a Mermaid browser runtime. Common `mmdc` input, output, theme, background, config, CSS, sizing, icon, and PDF-fit workflows are available under the `merman-cli` command name.

`--puppeteerConfigFile` is accepted for script compatibility, but its valid JSON contents are ignored because no Puppeteer runtime exists. PDF output follows Merman's vector conversion and is not expected to be pixel-identical to Chromium print-to-PDF.

See the [CLI compatibility matrix](https://github.com/Latias94/merman/blob/main/docs/alignment/CLI_COMPATIBILITY.md) for the exact supported surface, or run:

```sh
merman-cli --help
merman-cli render --help
```

## License

Licensed under either Apache-2.0 or MIT at your option.
