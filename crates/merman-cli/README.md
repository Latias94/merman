# merman-cli

[![Crates.io](https://img.shields.io/crates/v/merman-cli.svg)](https://crates.io/crates/merman-cli) [![Documentation](https://docs.rs/merman-cli/badge.svg)](https://docs.rs/merman-cli) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](https://github.com/Latias94/merman/blob/main/LICENSE-MIT)

Render, inspect, and lint Mermaid without Node.js, Puppeteer, Chromium, or another JavaScript runtime. The default binary includes SVG, PNG, JPEG, vector PDF, ASCII/Unicode, analysis, Markdown batch rendering, committed Rustdoc fragment generation, Cytoscape layout, math, icons, completions, and native runtime adapters. ELK layout remains an explicit opt-in because it adds the EPL-2.0 ELK closure.

The command line has four explicit workflows:

| Workflow | Command | Use it when |
| --- | --- | --- |
| One native render | `merman-cli render` | You want concise Rust-native defaults and strict option validation |
| Native Markdown batch | `merman-cli batch` | You want a recoverable, tool-owned multi-file generation |
| Static Rustdoc fragments | `merman-cli rustdoc` | You want Mermaid in crate or item docs without adding a renderer to the crate's Cargo graph |
| Pinned compatibility | `merman-cli mmdc` | You are migrating an `mmdc@11.16.0` command or need its naming and scanner rules |

## Install

This README describes the current source revision. For a published release, prefer the complete prebuilt binary, with a source-build fallback when no official archive is available for the current target:

```sh
cargo binstall merman-cli@0.8.0-alpha.5
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
cargo install --git https://github.com/Latias94/merman --rev FULL_COMMIT_SHA --locked merman-cli
```

Replace `FULL_COMMIT_SHA` with a reviewed 40-character commit from the repository.

From a local checkout:

```sh
cargo install --path crates/merman-cli
```

The standard source-install command uses the default capability set, which deliberately omits ELK. Project release artifacts select the complete `cli-release` capability set, including ELK, and ship the matching notices. Cargo-dist and, beginning with `0.8.0-alpha.5`, cargo-binstall consume the project-built `dist` artifact; source channels build the default features unless you select an explicit profile. Channels also differ in which support files they place on disk and who publishes them:

| Channel | Binary source | Completion and man pages | Availability |
| --- | --- | --- | --- |
| `cargo binstall merman-cli@VERSION` | `0.8.0-alpha.5` and later: project release archive, with source fallback | Not installed | Selected registry version; its own metadata governs |
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
| Build committed Rustdoc fragments | `merman-cli rustdoc build` |
| Verify committed Rustdoc fragments | `merman-cli rustdoc check --quiet` |
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

## Rustdoc Fragments

`merman-cli rustdoc` is the CLI-first integration for crates that want static Mermaid diagrams in
Rustdoc without placing Merman, layout engines, math rendering, or a procedural macro in the
consuming crate's normal or build dependency graph. The CLI is an authoring and CI tool: it renders
the complete bundle ahead of `cargo doc`, and the crate consumes only committed Markdown through
Rust's built-in `include_str!`.

Place `merman-rustdoc.toml` at the crate root so its fixed managed output root is
`docs/generated/merman-rustdoc/`:

```toml
schema = 1

[[fragments]]
id = "crate-overview"
source = "docs/rustdoc-src/crate-overview.md"

[[fragments]]
id = "render-module"
source = "docs/rustdoc-src/render-module.md"
source_display = "details"
```

`source` is relative to the directory containing the configuration. Markdown sources may contain
backtick or tilde Mermaid fences and standalone `include_mmd!("diagrams/architecture.mmd")` lines;
include paths are also relative to the configuration root. Raw `.mmd` and `.mermaid` files are
accepted as one-diagram fragments. `source_display` is optional and is either `hide` (the default)
or `details`, which adds the Mermaid source below each rendered diagram in a collapsed block.
Fragment IDs are portable ASCII identifiers and become `<id>.md` filenames. The output directory
cannot be redirected by configuration.
Rename an ID through a temporary, distinctly spelled ID in two builds when only its ASCII case
changes; direct case-only renames are rejected so the same history behaves on every filesystem.
The fixed managed root has one owner: a receipt created by one configuration filename cannot be
replaced or cleaned by a different configuration in the same package directory. Move or remove
the old managed root explicitly before adopting it with a different configuration.

Build or verify the bundle:

```sh
merman-cli rustdoc build
merman-cli rustdoc check --quiet

# A workspace member can keep its own configuration and managed root.
merman-cli rustdoc build --config crates/my-crate/merman-rustdoc.toml
merman-cli rustdoc check --config crates/my-crate/merman-rustdoc.toml --quiet
```

`build` renders and validates the complete expected bundle before publishing it transactionally.
It writes `receipt.json` last, removes only stale files owned by the previous valid receipt, leaves
unknown neighboring files alone, and preserves unchanged files on a no-op rebuild. It never
overwrites the configuration, Markdown sources, or included Mermaid files. `check` performs the
same bounded generation in memory, compares the exact receipt-owned file set, receipt, and fragment
bytes, and never writes. A stale or missing generation exits `1`, invalid invocation or
configuration exits `2`, and an invalid receipt, unfinished publication, or other operational
failure exits `3`.

Consume generated fragments at crate or item scope without a documentation dependency:

```rust
#![doc = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/generated/merman-rustdoc/crate-overview.md"
))]

#[doc = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/generated/merman-rustdoc/render-module.md"
))]
pub mod render {}
```

Include each generated fragment at most once on one rendered Rustdoc page. Its SVG DOM IDs are
deterministic within the fragment, so including the same fragment twice would duplicate those IDs.

Commit the configuration, authoring sources, generated fragments, and `receipt.json` together.
Review an intentional regeneration before staging it:

```sh
merman-cli rustdoc build
git diff -- merman-rustdoc.toml docs/rustdoc-src docs/generated/merman-rustdoc
```

To abandon the whole local documentation change, restore the configuration, source paths, and
managed output paths with Git. To keep the source change but discard only generated edits, restore
the managed directory and run `rustdoc build` again; `rustdoc check` will deliberately report stale
output until the generated bundle matches the retained sources.

```sh
# Substitute the source paths used by your configuration.
git restore -- merman-rustdoc.toml docs/rustdoc-src docs/generated/merman-rustdoc
```

Run freshness verification before Rustdoc in CI:

```sh
merman-cli rustdoc check --quiet
cargo doc --no-deps
```

Pin the same CLI release or reviewed source revision in authoring and CI. The receipt binds the
generator version, Mermaid baseline, capability descriptor, configuration, sources, and output
hashes, so an intentional tool upgrade requires one reviewed `rustdoc build` regeneration.

docs.rs does not run `merman-cli`; it documents the uploaded Cargo package. Ensure the generated
fragments are committed and included in the package, especially when `Cargo.toml` has an explicit
`include` whitelist. Inspect the package boundary before publishing:

```sh
cargo package --list --allow-dirty
```

The generated-fragment path and the independent
[`merman-rustdoc`](https://github.com/Latias94/merman/tree/main/crates/merman-rustdoc#readme)
attribute macro are peer products. Neither invokes, discovers, or falls back to the other:

| Concern | `merman-cli rustdoc` | `merman-rustdoc` attribute macro |
| --- | --- | --- |
| Cargo dependency cost | No Merman renderer dependency in the consuming crate | Compiles the selected procedural-macro and renderer closure |
| Authoring loop | Explicit `build`; CI uses read-only `check` | One-step rendering while `cargo doc` runs |
| Rustdoc scope | Crate and item docs through `include_str!` | Item docs and recursive inline item trees; not crate-level `//!` or `#[doc = include_str!(...)]` content |
| Generated ownership | Markdown fragments and receipt are committed and reviewable | Source comments stay unchanged; SVG exists only in generated Rustdoc output |
| docs.rs | Uses packaged static files with no documentation feature | Requires the optional macro dependency and docs.rs feature to be enabled |
| Failure timing | Authoring build or CI freshness gate, before `cargo doc` | During macro expansion in `cargo doc` |
| Rollback | Restore source/config/output as one Git change, or rebuild the retained source | Revert the annotated Rust source or feature selection |

Choose the CLI path for published libraries, workspaces, reproducible packaging, crate-level docs,
or any project sensitive to Cargo dependency closure. Choose the macro when one-step `cargo doc`
ergonomics outweigh the compile cost and item-level attribute placement is the desired ownership
model. A migration is additive and reversible: move the annotated prose to an external Markdown
source, declare a fragment, run `rustdoc build`, replace the attribute with `#[doc =
include_str!(...)]`, add the CI check, and remove the optional macro dependency only after no
annotated items remain.

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
| `--no-default-features --features rustdoc` | Static Rustdoc fragment build/check with deterministic SVG, Cytoscape layout, and math |
| `--no-default-features --features icons` | SVG plus bounded local Iconify packs |
| `--no-default-features --features png` | SVG plus PNG only |
| `--no-default-features --features pdf` | SVG plus vector PDF only |

Install a lint-only binary:

```sh
cargo install merman-cli --version 0.8.0-alpha.6 --locked \
  --no-default-features --features analysis
```

Additional leaves are `jpeg`, `layout-cytoscape`, `layout-elk`, `math`, `network-icons`, `parallel-markdown`, `shell-completions`, `system-clock`, `system-timezone`, `system-random`, and `system-timing`. `layout-elk` is the explicit EPL-2.0 boundary; add it only when the resulting artifact will distribute the corresponding notices and provenance. Implications such as `png -> svg` and `network-icons -> icons` are intentional.

Use `merman-cli capabilities --json` as the machine-readable authority for the installed artifact. The current document keeps `schema_version: 2` and reports `cli_contract_version: 4`, package and pinned compatibility versions, descriptor digest, compiled commands, capabilities, and outputs. Contract 4 records the native `-f` spelling, text-first `lint`, narrowed `detect` surface, and the feature-gated top-level `rustdoc` workflow; automation that depends on CLI behavior should version-check this field independently from the JSON schema.

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

## Operation Control

`render`, `batch`, `mmdc`, and `rustdoc build` / `rustdoc check` own one cooperative operation
control from input acquisition through the final publication boundary. Pressing Ctrl-C requests
cancellation through that control instead of relying only on abrupt process termination; pressing
Ctrl-C again before cooperative shutdown restores an immediate exit. Use
`--operation-timeout-ms MILLISECONDS` to add a relative monotonic deadline:

```sh
merman-cli render diagram.mmd --operation-timeout-ms 5000
merman-cli batch README.md --operation-timeout-ms 30000
merman-cli mmdc -i diagram.mmd -o diagram.svg --operation-timeout-ms 5000
merman-cli layout diagram.mmd --operation-timeout-ms 5000
merman-cli rustdoc check --operation-timeout-ms 30000
```

The deadline is operation-wide, including stdin and bounded file acquisition. It can expire while a
stdin producer keeps its pipe open without sending data. A Markdown batch does not reset it for
each chart, and parallel chart workers observe the same cancellation state. Cancellation is
cooperative: a blocking host or backend call returns to a checkpoint before the request is
observed. Once observed, cancellation emits no partial rendered document; file and Markdown
generation paths check the control while staging output and preserve their existing atomic or
recoverable transaction guarantees.

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
| `1` | Invalid Mermaid/content/render result, cooperative cancellation or deadline expiry, a source-required layout or math capability is unavailable, or `fix --check/--diff` would change source |
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

Merman's own code is licensed under either Apache-2.0 or MIT at your option. The ordinary Cargo
default intentionally excludes the optional EPL-2.0 ELK implementation, but the `cli-release`
archive includes ELK and the math/font closure selected by its profile. Release archives include
the matching `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES/`; source-built distributions must
carry the notices for the features they select.
