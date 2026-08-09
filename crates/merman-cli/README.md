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

This README describes the current source revision. For a published release, prefer the complete prebuilt binary, with a source-build fallback when no official archive is available for the current target:

```sh
cargo binstall merman-cli
```

Starting with `0.8.0-alpha.5`, Merman's cargo-binstall metadata uses the repository's cargo-dist GitHub Release archive for the current target, disables third-party QuickInstall artifacts, and preserves `cargo install` as the fallback when an official archive is unavailable. Published binary channels can trail this development branch; check `merman-cli --version` first, then use `merman-cli capabilities --json` on `0.8.0-alpha.5` and later. Pin a source revision when you need the exact contract documented here.

Homebrew users can install the stable formula:

```sh
brew install merman-cli
```

The formula follows stable releases and may trail this pre-release documentation.

Starting with `0.8.0-alpha.5`, version-specific [GitHub Releases](https://github.com/Latias94/merman/releases) also provide `merman-cli-installer.sh` and `merman-cli-installer.ps1`. Download an installer from the chosen release rather than a moving URL; it installs only the binary and fails closed if the archive SHA-256 cannot be verified.

Install the complete CLI from source:

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-cli
```

The Git command follows the repository's default branch at install time. Add `--rev FULL_COMMIT_SHA` to pin a remote revision.

From a local checkout:

```sh
cargo install --path crates/merman-cli
```

The standard commands above and project release artifacts select the complete `cli-release` capability set. Cargo-dist and, beginning with `0.8.0-alpha.5`, cargo-binstall consume the project-built `dist` artifact; source channels build the same features with their package manager's release profile. Channels also differ in which support files they place on disk and who publishes them:

| Channel | Binary source | Completion and man pages | Availability |
| --- | --- | --- | --- |
| `cargo binstall merman-cli` | `0.8.0-alpha.5` and later: project release archive, with source fallback | Not installed | Registry-selected version; its own metadata governs |
| GitHub shell or PowerShell installer | Project release archive | Not installed | `0.8.0-alpha.5` and later |
| Direct GitHub archive | Project release archive | Bundled under `completions/` and `man/` | `0.8.0-alpha.5` and later |
| Homebrew formula | Formula source build or Homebrew bottle | `0.8.0` and later: Bash, Zsh, Fish, PowerShell, and man pages installed | External stable channel; selected formula version governs |
| Repository Nix package | Built from locked repository source | Bash, Zsh, Fish, PowerShell, Elvish, and man pages installed | First-party source interface, not a registry package |
| `cargo install` | Built from crates.io, Git, or a checkout | Not installed | Registry or source revision selected by the user |
| Scoop and WinGet | Verified Windows x86_64 release archive | Not installed | Stable candidates are generated; external submission is pending |

Nix users can run `nix run . -- --version` or `nix profile install .` from a checkout. An exact remote revision can be used as `nix run "github:Latias94/merman?rev=FULL_COMMIT_SHA" -- --version`. This source package is separate from the precompiled Linux archive compatibility claim.

Older cargo-binstall releases follow the metadata and fallback policy embedded in that selected release; the current official-archive mapping and QuickInstall prohibition do not apply retroactively.

A complete binary, or a custom build that includes `shell-completions`, can generate completion at runtime:

```sh
merman-cli completion bash
merman-cli completion zsh
merman-cli completion fish
merman-cli completion powershell
merman-cli completion elvish
```

Release archives beginning with `0.8.0-alpha.5` also include legal notices. Cargo-binstall installs only the executable. The cargo-dist installers likewise omit completion and man files, but may create an environment file and update shell startup configuration to expose their install directory on `PATH`. Direct archive users should verify the adjacent checksum and the GitHub attestation constrained to the release workflow and tag; the [CLI release contract](https://github.com/Latias94/merman/blob/main/docs/releasing/CLI.md) provides the exact command and trust boundary.

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
| Human-readable diagnostics | `merman-cli lint diagram.mmd` |
| Machine-readable diagnostics | `merman-cli lint diagram.mmd --format json` |
| Check whether fixes are needed | `merman-cli fix diagram.mmd --check` |
| Inspect the compiled binary | `merman-cli capabilities --json` |

Run `merman-cli --help` to see only the commands compiled into your binary, then use `<command> --help` for command-owned options.

`lint` defaults to stable human-readable text. Automation should request `--format json` explicitly; `lint-rules` remains JSON by default, and `--pretty` is valid only with JSON output.

## Command Dialects And Root Compatibility

Root-level render flags no longer belong to the advertised command tree. Keeping native and compatibility arguments in separate parsers gives each workflow its own defaults, validation, help, completion output, input rules, and publication guarantees without breaking existing root-level `mmdc` scripts.

| Existing or upstream syntax | Preferred explicit syntax |
| --- | --- |
| `mmdc -i diagram.mmd -o diagram.svg` | `merman-cli mmdc -i diagram.mmd -o diagram.svg` |
| `merman-cli -i diagram.mmd -o diagram.svg` | `merman-cli mmdc -i diagram.mmd -o diagram.svg` |
| `merman-cli -i diagram.mmd -o diagram.png -t dark` | `merman-cli mmdc -i diagram.mmd -o diagram.png -t dark` |
| `merman-cli -i README.md -o README.rendered.md --artifacts docs/assets` | `merman-cli mmdc -i README.md -o README.rendered.md --artifacts docs/assets` |
| Native single render through shared flags | `merman-cli render diagram.mmd --output diagram.svg` |
| Native Markdown through extension inference | `merman-cli batch README.md --output-dir README.merman` |
| `merman-cli render diagram.mmd -e png` | `merman-cli render diagram.mmd -f png` |
| `merman-cli batch README.md -e png` | `merman-cli batch README.md -f png` |

An invocation whose first argument is an option owned by `mmdc`, such as `-i` or `--input`, is permanently forwarded to the same parser and execution path as `merman-cli mmdc`. This silent compatibility alias is intentionally absent from help and completions. Explicit `merman-cli mmdc` remains the preferred compatibility spelling because it makes the selected contract visible to readers and tooling.

Bare root inputs and native-only root options exit `2` with a targeted message pointing to `mmdc`, `render`, and `batch`. New scripts should choose an explicit subcommand.

Native `render -e` and `batch -e` are hidden aliases for `-f/--format` during `0.8.x`. Their bounded migration warnings remain visible even with `--quiet`, and the aliases are removed in `v0.9.0`. This does not affect `merman-cli mmdc -e/--outputFormat`, which remains part of the pinned compatibility interface.

The `mmdc` subcommand is a release-pinned compatibility snapshot. This release follows the supported command behavior of `@mermaid-js/mermaid-cli@11.16.0`; future changes are tied to an explicit Mermaid baseline update. See the [compatibility register](https://github.com/Latias94/merman/blob/main/docs/alignment/CLI_COMPATIBILITY.md) for exact coverage and deliberate browserless divergences.

## Markdown Batches

Native batch rendering owns one output directory:

```sh
merman-cli batch README.md
# writes README.merman/README.md, numbered assets, a manifest, and a stable lock

merman-cli batch docs/guide.md --output-dir generated --format pdf
```

All charts render into staging before publication. The rewritten document is published last, stale files are removed only when named by the prior validated manifest, and an interrupted commit is recovered under the output lock before new work starts. Switching among supported output formats migrates the same managed generation and removes its prior-format artifacts. A document with no eligible charts is a valid generation.

Stdin requires an explicit logical name and output directory:

```sh
cat README.md | merman-cli batch - \
  --stdin-file-name README.md \
  --output-dir README.merman
```

The `parallel-markdown` Cargo feature adds Rayon-backed bounded scheduling and the `--jobs` option. It implies `markdown`; it does not affect single renders. Without it, `batch` remains fully supported and renders charts serially.

Strict `mmdc` Markdown uses the pinned upstream fence scanner and output naming. To keep recovery honest, its rewritten document and artifacts must remain below one transaction root on one filesystem; split-root layouts are rejected before output creation or network access. Changing `-e` or `--artefacts` publishes the new namespace but leaves files from the previous namespace untouched.

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

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-cli \
  --no-default-features --features analysis
```

Additional leaves are `jpeg`, `layout-cytoscape`, `layout-elk`, `math`, `network-icons`, `parallel-markdown`, `shell-completions`, `system-clock`, `system-timezone`, `system-random`, and `system-timing`. Implications such as `png -> svg` and `network-icons -> icons` are intentional.

Use `merman-cli capabilities --json` as the machine-readable authority for the installed artifact. The current document keeps `schema_version: 2` and reports `cli_contract_version: 3`, package and pinned compatibility versions, descriptor digest, compiled commands, capabilities, and outputs. Contract 3 records the native `-f` spelling, text-first `lint`, and narrowed `detect` surface; automation that depends on CLI behavior should version-check this field independently from the JSON schema.

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

Use repeatable `--resource-limit STABLE_ID=POSITIVE_U64` only for a scoped override. The unbounded profile retains hard protocol guards, finite network timeouts, redirect limits, overflow checks, renderer-owned icon-registry ceilings, and backend capabilities.

Local icon packs stay offline:

```sh
merman-cli render diagram.mmd \
  --icon-pack @iconify-json/logos

merman-cli render diagram.mmd \
  --icon-pack-source logos#icons.json
```

Icon acquisition and renderer admission are separate bounded stages. The CLI can accept at most 16
packs, 16 MiB for one local or remote pack, and 32 MiB in aggregate; constrained profiles are
tighter. No profile or override can exceed those renderer-owned constructor capabilities. Each
acquired body is then passed as raw bytes to the shared transactional IconifyJSON builder, which
validates JSON structure, identifiers, geometry, aliases, SVG bodies, retained memory, and build
work before publishing an immutable registry. A failed pack publishes no partial registry.

Local package/path lookup and optional HTTP acquisition belong only to the CLI `icons` and
`network-icons` features. The `merman` and `merman-render` Rust library surfaces include icon
registry construction in their existing `svg` capability and do not pull CLI lookup, URL, DNS,
HTTP, Tokio, or filesystem-acquisition dependencies.

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
| `1` | Invalid Mermaid/content/render result, a source-required layout or math capability is unavailable, or `fix --check/--diff` would change source |
| `2` | Invalid invocation, conflicting options, unavailable statically requested option/output capability, or configuration |
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

Release archives beginning with `0.8.0-alpha.5` also carry deterministic completion snapshots and manual pages so downstream package definitions can install shell integration without executing a foreign-target binary during packaging. These assets are generated from the same Clap command tree and checked for drift in CI. Homebrew stable integration is monitored by this repository; Scoop and WinGet manifests are not currently published.

The checked-in completion and manual assets represent the canonical `cli-release` complete profile.
For a custom slim build with `shell-completions`, generate completion from that binary at runtime so
omitted commands, options, and values stay omitted. A build without that feature has no
`completion` subcommand.

## License

Licensed under either Apache-2.0 or MIT at your option.
