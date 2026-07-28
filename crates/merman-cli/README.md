# merman-cli

[![Crates.io](https://img.shields.io/crates/v/merman-cli.svg)](https://crates.io/crates/merman-cli) [![Documentation](https://docs.rs/merman-cli/badge.svg)](https://docs.rs/merman-cli) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

Render, inspect, and lint Mermaid without Node.js, Puppeteer, Chromium, or another JavaScript runtime. The default binary includes SVG, PNG, JPEG, vector PDF, ASCII/Unicode, analysis, Markdown batch rendering, optional layout engines, math, icons, completions, and native runtime adapters.

The command line has three explicit workflows:

| Workflow | Command | Use it when |
| --- | --- | --- |
| One native render | `merman-cli render` | You want concise Rust-native defaults and strict option validation |
| Native Markdown batch | `merman-cli batch` | You want a recoverable, tool-owned multi-file generation |
| Pinned compatibility | `merman-cli mmdc` | You are migrating an `mmdc@11.16.0` command or need its naming and scanner rules |

## Install

Install the complete CLI:

<!-- BEGIN GENERATED RELEASE README CLI_PACKAGE_INSTALL -->

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-cli
```

<!-- END GENERATED RELEASE README CLI_PACKAGE_INSTALL -->

Homebrew users can install the stable formula:

```sh
brew install merman-cli
```

The formula follows stable releases and may trail this pre-release documentation.

From a local checkout:

```sh
cargo install --path crates/merman-cli
```

The executable is named `merman-cli`; Merman does not install an `mmdc` alias.

## First Render

For a named input, native rendering writes a sibling file and replaces the source extension:

```sh
merman-cli render diagram.mmd
# writes diagram.svg

merman-cli render diagram.mmd --format png
# writes diagram.png
```

Piped input writes the payload to stdout:

```sh
printf 'flowchart LR\n  Source --> Merman --> SVG\n' | merman-cli render -
```

Use `--output -` to request stdout explicitly or `--output PATH` to choose a file. Payload bytes go only to stdout; diagnostics and progress go to stderr.

## Common Workflows

| Task | Command |
| --- | --- |
| Mermaid to SVG | `merman-cli render diagram.mmd` |
| Mermaid to PNG | `merman-cli render diagram.mmd --format png` |
| Mermaid to JPEG | `merman-cli render diagram.mmd --format jpg` |
| Mermaid to vector PDF | `merman-cli render diagram.mmd --format pdf` |
| Terminal output | `merman-cli render diagram.mmd --format unicode --output -` |
| Markdown fences to a managed generation | `merman-cli batch README.md` |
| Human-readable diagnostics | `merman-cli lint diagram.mmd --format text` |
| Check whether fixes are needed | `merman-cli fix diagram.mmd --check` |
| Inspect the compiled binary | `merman-cli capabilities --json` |

Run `merman-cli --help` to see only the commands compiled into your binary, then use `<command> --help` for command-owned options.

## Migrating From The Old Root Syntax

Root-level render flags were removed. They were ambiguous with native subcommands, exposed options that silently did nothing for some formats, and made compatibility behavior impossible to version independently. The break gives each workflow its own parser, defaults, validation, help, completion output, input rules, and publication guarantees.

| Before | Now |
| --- | --- |
| `merman-cli -i diagram.mmd -o diagram.svg` | `merman-cli mmdc -i diagram.mmd -o diagram.svg` |
| `merman-cli -i diagram.mmd -o diagram.png -t dark` | `merman-cli mmdc -i diagram.mmd -o diagram.png -t dark` |
| `merman-cli -i README.md -o README.rendered.md --artifacts docs/assets` | `merman-cli mmdc -i README.md -o README.rendered.md --artifacts docs/assets` |
| Native single render through shared flags | `merman-cli render diagram.mmd --output diagram.svg` |
| Native Markdown through extension inference | `merman-cli batch README.md --output-dir README.merman` |

Passing a removed root render flag exits `2` with a targeted message pointing to `mmdc`, `render`, and `batch`; it does not execute a hidden compatibility path.

The `mmdc` subcommand is a release-pinned compatibility snapshot. This release follows the supported command behavior of `@mermaid-js/mermaid-cli@11.16.0`; future changes are tied to an explicit Mermaid baseline update. See the [compatibility register](https://github.com/Latias94/merman/blob/main/docs/alignment/CLI_COMPATIBILITY.md) for exact coverage and deliberate browserless divergences.

## Markdown Batches

Native batch rendering owns one output directory:

```sh
merman-cli batch README.md
# writes README.merman/README.md, numbered assets, a manifest, and a stable lock

merman-cli batch docs/guide.md --output-dir generated --format pdf
```

All charts render into staging before publication. The rewritten document is published last, stale files are removed only when named by the prior validated manifest, and an interrupted commit is recovered under the output lock before new work starts. A document with no eligible charts is a valid generation.

Stdin requires an explicit logical name and output directory:

```sh
cat README.md | merman-cli batch - \
  --stdin-file-name README.md \
  --output-dir README.merman
```

The `parallel-markdown` Cargo feature adds Rayon-backed bounded scheduling and the `--jobs` option. It implies `markdown`; it does not affect single renders. Without it, `batch` remains fully supported and renders charts serially.

Strict `mmdc` Markdown uses the pinned upstream fence scanner and output naming. To keep recovery honest, its rewritten document and artifacts must remain below one transaction root on one filesystem; split-root layouts are rejected before output creation or network access.

## Analysis And Fixes

The `analysis` capability enables parser-backed diagnostics, deterministic fix selection, and rule metadata:

```sh
merman-cli detect diagram.mmd
merman-cli parse diagram.mmd --pretty --meta
merman-cli layout diagram.mmd --pretty
merman-cli lint diagram.mmd --format json
merman-cli lint README.md --markdown --format text
merman-cli lint-rules --configurable --format json
```

`fix` has one explicit output mode:

```sh
merman-cli fix diagram.mmd                  # fixed source on stdout
merman-cli fix diagram.mmd --check          # exit 1 when source would change
merman-cli fix diagram.mmd --diff           # print diff; exit 1 when it changes
merman-cli fix diagram.mmd --output fixed.mmd
merman-cli fix diagram.mmd --write
```

Use repeatable `--rule RULE_ID` or `--fix STABLE_FIX_ID` selectors when automation needs a narrower edit plan. Duplicate edit sets are applied once, alternative fixes remain alternatives, and conflicting selections fail before publication. `--write` atomically replaces the canonical input target after checking that its identity and complete bounded contents still match the acquired snapshot.

## Choose A Build

The default feature set is the complete local product. Cargo features are additive capabilities, not diagram-family switches. For a slim binary, disable defaults and select only the required leaves:

| Build | Capabilities |
| --- | --- |
| `--no-default-features` | `detect`, `parse`, and `capabilities` |
| `--no-default-features --features analysis` | Lint, fixes, and rule metadata without render dependencies |
| `--no-default-features --features svg` | Basic deterministic SVG |
| `--no-default-features --features ascii` | ASCII/Unicode without SVG |
| `--no-default-features --features markdown` | Sequential native Markdown batch and SVG |
| `--no-default-features --features icons` | SVG plus bounded local Iconify packs |
| `--no-default-features --features png` | SVG plus PNG only |
| `--no-default-features --features pdf` | SVG plus vector PDF only |

Install a lint-only binary:

<!-- BEGIN GENERATED RELEASE README CLI_PACKAGE_LEAN_INSTALL -->

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-cli \
  --no-default-features --features analysis
```

<!-- END GENERATED RELEASE README CLI_PACKAGE_LEAN_INSTALL -->

Additional leaves are `jpeg`, `layout-cytoscape`, `layout-elk`, `math`, `network-icons`, `parallel-markdown`, `shell-completions`, `system-clock`, `system-timezone`, `system-random`, and `system-timing`. Implications such as `png -> svg` and `network-icons -> icons` are intentional.

Use `merman-cli capabilities --json` as the machine-readable authority for the installed artifact. It reports the CLI contract version, package and pinned compatibility versions, descriptor digest, compiled commands, capabilities, and outputs.

## Rendering And Runtime Policy

Native `render` rejects options that are irrelevant to the selected output before reading input or creating output. Examples:

```sh
merman-cli render diagram.mmd --format svg --svg-pipeline readable
merman-cli render diagram.mmd --format png --raster-fit-width 1600
merman-cli render diagram.mmd --format pdf --pdf-filter-scale 4
merman-cli render diagram.mmd --format unicode --ascii-color auto
```

PNG/JPEG use a bounded Rust raster pipeline. PDF keeps vector geometry and bounds localized filter and embedded-image raster work. They are not Chromium screenshots. ASCII/Unicode support is family-specific; see the [support matrix](https://github.com/Latias94/merman/blob/main/docs/rendering/ASCII_SUPPORT_MATRIX.md).

Runtime behavior is deterministic by default even when system adapters are compiled. This also applies to `mmdc` and is a deliberate divergence from Chromium's ambient date, time zone, and randomness. A complete default binary can opt into upstream-like host state:

```sh
merman-cli render diagram.mmd --runtime native
merman-cli mmdc -i diagram.mmd -o diagram.svg --runtime native
```

Each adapter is also independently selectable with `--system-clock`, `--system-timezone`, `--system-random`, or `--system-timing` when its feature is compiled. `--runtime native` is shown only when the clock, time-zone, and random adapters are all available. Timing remains separately opt-in.

## Resource And Network Policy

`--resource-profile` derives one complete budget for source/config/CSS/icon acquisition, chart count, staging, render working set, jobs, redirects, and network duration:

| Profile | Intended input |
| --- | --- |
| `constrained` | Untrusted, public, or multi-tenant |
| `interactive` | Cooperative editor-like work |
| `trusted-native` | Controlled local automation; CLI default |
| `unbounded-for-trusted-input` | Explicitly trusted work that owns its cost |

Use repeatable `--resource-limit STABLE_ID=POSITIVE_U64` only for a scoped override. The unbounded profile retains hard protocol guards, finite network timeouts, redirect limits, overflow checks, and backend capabilities.

Local icon packs stay offline:

```sh
merman-cli render diagram.mmd \
  --icon-pack @iconify-json/logos

merman-cli render diagram.mmd \
  --icon-pack-source logos#icons.json
```

HTTP(S) sources require `network-icons` plus `--allow-network`. Loopback, private, link-local, multicast, and unspecified destinations additionally require `--allow-private-network`. Every redirect is resolved and authorized again; diagnostics redact URL credentials, paths, queries, and fragments.

## Existing SVG Input

PNG, JPEG, and PDF builds can convert a named `.svg` file directly:

```sh
merman-cli render diagram.svg --format png --output diagram.png
```

For SVG read from stdin, add `--input-kind svg`; named `.svg` files are inferred by extension. Raw SVG conversion is native-only and passes through the same bounded sanitizer/export pipeline.

## Exit And Output Contracts

| Exit | Meaning |
| ---: | --- |
| `0` | Success, including a closed downstream stdout pipe |
| `1` | Invalid Mermaid/content/render result, or `fix --check/--diff` would change source |
| `2` | Invalid invocation, conflicting options, unavailable capability, or configuration |
| `3` | Local/remote operational failure, lock contention, incomplete recovery, or publication failure |

stdout contains only the requested SVG, image, text, JSON, diff, fixed source, or completion payload. `--quiet` suppresses informational and timing diagnostics where supported; errors remain visible.

## Completions And Man Pages

A binary built with `shell-completions` generates completions that contain only its compiled commands:

```sh
source <(merman-cli completion bash)
merman-cli completion zsh > _merman-cli
merman-cli completion fish > merman-cli.fish
merman-cli completion powershell > merman-cli.ps1
```

Release archives also carry deterministic completion snapshots and manual pages so downstream package definitions can install shell integration without executing a foreign-target binary during packaging. These assets are generated from the same Clap command tree and checked for drift in CI. Homebrew stable integration is monitored by this repository; Scoop and WinGet manifests are not currently published.

## License

Licensed under either Apache-2.0 or MIT at your option.
