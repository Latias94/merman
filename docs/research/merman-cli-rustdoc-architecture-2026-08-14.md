# Merman Rustdoc Architecture: CLI-First versus Embedded Renderers

**Researched:** 2026-08-14

**Repository snapshot:** `24b0be325c5b30483dc42ef20b37b7c2bf1f22bb`

**Scope:** Decide whether `merman-rustdoc` should keep rendering inside a procedural macro, move
generation to `merman-cli`, or use an embedded WASM renderer. This report focuses on the dependency
closure and publishing workflow; the public-crate comparison is covered in the companion
[Rustdoc Mermaid ecosystem report](rustdoc-mermaid-ecosystem-2026-08-14.md).

**Decision:** Make a CLI-first, checked pre-generation lane the product direction for the strict
zero-renderer-dependency requirement. Run an isolated embedded-WASM spike only if retaining the
current one-command `cargo doc` and inline attribute experience is a non-negotiable requirement.
Do not keep native, WASM, CLI, and browser fallbacks inside one Cargo feature surface.

## Executive Summary

The complaint about large builds is a placement problem, not a missing feature gate. The current
`merman-rustdoc` package is a host-side procedural macro whose selected features link the Merman
renderer into every consumer that enables the documentation integration. The manifest declares the
proc-macro target and complete renderer aggregate ([`Cargo.toml`](../../crates/merman-rustdoc/Cargo.toml)),
and the crate documentation explicitly notes that `cfg_attr(doc, ...)` prevents expansion but does
not prevent Cargo from compiling the selected dependency ([`lib.rs`](../../crates/merman-rustdoc/src/lib.rs)).

At this snapshot, the locked package-closure proxy is:

| `merman-rustdoc` configuration | Unique normal packages | Normal + build packages | Macro usable |
| --- | ---: | ---: | --- |
| `--no-default-features` | 1 | 1 | No; emits a missing-`svg` error |
| `--no-default-features --features svg` | 112 | 116 | Yes |
| default `complete-svg` | 168 | 173 | Yes |

These counts are `cargo tree --locked` package/version counts after removing repeated `(*)`
markers. They are not clean-build time, peak RSS, or disk measurements, but they demonstrate why
adding more renderer features is not a structural fix.

The recommended target is:

```text
declared Markdown/.mmd sources
    -> merman-cli rustdoc build/check
    -> committed inline-SVG Markdown fragments + portable receipt
    -> Rust built-in include_str! in cargo doc
```

This makes the renderer cost explicit and amortizable in a developer/release tool. Ordinary
consumer builds, `cargo doc`, and docs.rs no longer need to compile or execute Merman. It also
supports crate-level documentation and avoids parsing arbitrary Rust syntax.

The current static-SVG advantages remain intact: pinned Mermaid semantics, deterministic output,
strict SVG validation, no JavaScript/CDN/Node/Chromium, and build-time diagnostics. The trade-off
is an explicit generation/check step and committed generated files.

If preserving the existing attribute syntax and one-step `cargo doc` is more important than strict
zero renderer dependencies, a separate spike may test a thin proc-macro host plus a release-built
pure-WASM Merman guest executed with `wasmi`. That is a measured option, not a reason to retain the
native renderer as a fallback feature.

## Evidence and Constraints

### Cargo and Rust behavior

Cargo enables an optional dependency when the feature that names it is enabled, and default
features are enabled unless every dependency edge opts out ([optional dependencies](https://doc.rust-lang.org/cargo/reference/features.html#optional-dependencies),
[default features](https://doc.rust-lang.org/cargo/reference/features.html#the-default-feature)).
Feature unification selects the union of enabled features for a package; resolver v2 does not make
an enabled proc-macro dependency lazy ([feature unification](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification),
[resolver v2](https://doc.rust-lang.org/cargo/reference/features.html#feature-resolver-version-2)).

Rustdoc sets `cfg(doc)` during documentation compilation, but that is a Rust conditional
compilation condition rather than Cargo dependency selection ([Rustdoc `cfg(doc)`](https://doc.rust-lang.org/rustdoc/advanced-features.html#cfgdoc-documenting-platform-specific-or-feature-specific-information)).
There is no supported dependency table that means "select this dependency only when the final
compiler invocation is rustdoc"; platform-specific dependency tables are for target/platform
selection, not source-level `cfg` modes ([platform-specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies)).

Procedural macros execute in the compiler process and have compiler-process file access
([Rust Reference: procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html)).
Moving the same renderer to another ordinary library crate does not remove the host-side build
closure. Moving it to a build dependency also does not remove the cost: Cargo still compiles and
runs selected build dependencies ([build dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#build-dependencies)).

### docs.rs and package behavior

docs.rs accepts selected Cargo options and rustdoc flags, but its metadata does not provide a
general hook for running a custom Cargo subcommand ([docs.rs metadata](https://docs.rs/about/metadata)).
Its builds run with network restrictions, resource limits, and a largely read-only source tree
([docs.rs build environment](https://docs.rs/about/builds)); JavaScript is not a dependable static
documentation contract ([docs.rs JavaScript policy](https://docs.rs/about#javascript)). A
pre-generated file included in the `.crate`, or an in-process dependency, is therefore required
for docs.rs.

Cargo package inclusion is governed by the package manifest's include/exclude rules
([Cargo manifest include/exclude](https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields)).
The release gate must inspect the actual package with `cargo package --list`; a custom CLI should
not attempt to emulate Cargo's packaging implementation. crates.io's package-size limit is a hard
constraint for any embedded guest ([Cargo packaging](https://doc.rust-lang.org/cargo/reference/publishing.html#packaging-a-crate)).

## Current Modules and the Correct Reuse Point

### `merman-rustdoc`

The current crate combines four responsibilities in one host artifact:

1. `syn` parses an annotated Rust item and optionally traverses an inline item tree
   ([`expand.rs`](../../crates/merman-rustdoc/src/expand.rs)).
2. A document scanner recognizes Mermaid fences and `include_mmd!`
   ([`doc.rs`](../../crates/merman-rustdoc/src/doc.rs)).
3. A native `HeadlessRenderer` constructs a deterministic environment and renders one or two SVGs
   ([`render.rs`](../../crates/merman-rustdoc/src/render.rs)).
4. HTML wrapping and strict SVG validation produce rustdoc-ready text
   ([`html.rs`](../../crates/merman-rustdoc/src/html.rs), [`svg.rs`](../../crates/merman-rustdoc/src/svg.rs)).

The private `MermaidRenderer` and `IncludeResolver` traits are useful test seams, but the
production Adapter is immediately the native renderer. They do not change Cargo resolution.

### `merman-cli` and existing batch behavior

`merman-cli` already owns the native renderer and the heavy capabilities as an explicit tool
([`Cargo.toml`](../../crates/merman-cli/Cargo.toml)). Its batch Module does useful work before
the first filesystem mutation: scan, resolve targets, prepare the renderer, validate containment,
stage all artifacts, publish metadata, and commit through the recovery transaction
([`batch.rs`](../../crates/merman-cli/src/batch.rs)). The Markdown Module has bounded fence
scanners and deterministic replacement helpers ([`markdown.rs`](../../crates/merman-cli/src/markdown.rs)).

That batch output contract is not the rustdoc contract:

- batch emits a rewritten document plus numbered external image files;
- rustdoc needs self-contained inline SVG/HTML fragments that survive Cargo packaging;
- batch's `GenerationManifest` tracks an owned path namespace, not source/config/renderer content
  freshness ([`format.rs`](../../crates/merman-cli/src/transaction/format.rs));
- the existing manifest uses native path encoding, while a committed receipt must be portable;
- adding a third `BatchDialect` would mix image-link semantics with rustdoc fragment semantics.

Reuse the renderer preparation, Markdown scanner, resource accounting, and transaction
implementation. Add a separate `RustdocBundle` Module and a separate portable receipt. The
transaction code can be deepened behind a generic `ManagedGenerationPublisher` Interface shared
by batch and rustdoc; its current type-state lock/recovery implementation is valuable, but callers
should not each understand all stage-slot details.

## Options Compared

| Option | Consumer Cargo closure | Static/offline output | One-step `cargo doc` | docs.rs | Primary risk | Decision |
| --- | --- | --- | --- | --- | --- | --- |
| Keep native proc-macro; tune features | High whenever enabled | Yes | Yes | Works only within selected resource budget | Base SVG closure remains broad | Reject as final; temporary mitigation only |
| CLI-first checked pre-generation | Zero renderer closure; tool pays cost once | Yes | No; explicit build first | Strong if files are packaged | Generated-file freshness and review diffs | Recommended for strict zero-cost lane |
| Thin proc-macro + embedded pure-WASM guest | Host + `wasmi` closure | Yes | Yes | Potentially strong | Guest size, interpreter latency, ABI/provenance | Bounded spike only |
| Browser Mermaid.js | Tiny Rust closure | No; runtime JS | Page loads/render later | Possible but CSP/network/runtime dependent | Loses static Merman guarantees | Optional web mode, not default |
| `build.rs`/build-dependency renderer | High whenever enabled | Yes | Usually | Execution is environment-sensitive | Relocates rather than removes cost | Reject |
| Proc-macro spawns installed CLI | Tiny Cargo closure | Usually | Yes locally | Tool is unavailable | PATH/version/non-hermetic behavior | Reject |
| `cargo doc`/HTML postprocessor | Zero consumer renderer closure | Yes after rewrite | No | docs.rs cannot run it as the only path | Rustdoc DOM/toolchain drift | Local/CI adjunct only |

### Native proc-macro retained

This is the only option that preserves the current source locality and build-time failure behavior
without a migration. It is also the option causing the complaint. `default = complete-svg` maps
directly to `svg`, Cytoscape, ELK, and math ([`Cargo.toml`](../../crates/merman-rustdoc/Cargo.toml)).
Changing the default to `svg` is a reasonable release stopgap (168 normal packages to 112 in the
current proxy), but it still leaves the broad base renderer in every enabled proc-macro build.
More diagram-family features increase interface and matrix cost without moving the Seam.

### Browser JavaScript

Browser-oriented crates are cheap because they emit a marker and let the reader's page load
Mermaid.js. That is a valid web-documentation product, but it changes the contract: rendering is
browser-dependent, can require network or a packaged JS asset, and errors happen after `cargo doc`.
It discards Merman's differentiators: deterministic build-time SVG, offline operation, strict
post-render validation, and stable light/dark variants. Browser JS can be an explicit adapter for a
web playground, not the Rustdoc default.

### `build.rs`

`build.rs` changes the execution owner, not the dependency location. An enabled build dependency
still builds the renderer for the consumer package. A `DOCS_RS` branch can skip execution but
cannot remove Cargo's selected dependency graph; it also makes a documentation concern run during
ordinary package builds. Downloading a renderer in `build.rs` is additionally non-hermetic and
blocked by docs.rs network policy.

### External CLI invoked by the macro

Calling `Command::new("merman-cli")` from a proc macro is not a real dependency boundary. It
depends on PATH, host architecture, installation order, and an unrecorded CLI version. It also
cannot work reliably on docs.rs. If a CLI is used, it must be an explicit developer/release step,
never an implicit compiler fallback.

### HTML postprocessing

Postprocessing `target/doc` can support local/CI workflows and preserve inline fences, but it cannot
be the only docs.rs solution. Rustdoc HTML is a toolchain output, not a stable application
interface; selectors, page grouping, and escaping can change. A postprocessor should parse only a
versioned marker it owns, fail visibly on unknown shapes, and remain an adjunct to the checked
fragment lane.

## Recommended CLI-First Module

### External Interface

Use two explicit commands with one internal Interface:

```text
merman-cli rustdoc build [--manifest-path Cargo.toml] [--quiet]
merman-cli rustdoc check [--manifest-path Cargo.toml] [--quiet]
```

The internal Module can be:

```rust
struct RustdocRequest {
    crate_root: PathBuf,
    mode: RustdocMode,
}

enum RustdocMode {
    Build,
    Check,
}

fn generate_rustdoc(request: RustdocRequest) -> Result<RustdocReport, RustdocError>;
```

Do not expose renderer, filesystem, HTML, or transaction traits to callers. The CLI Clap parser is
the process Adapter; tests can invoke the process for contract coverage and call the private
Module through focused integration fixtures.

### Configuration and generated files

Keep one declarative file at the package root:

```toml
schema = 1

[defaults]
pipeline = "readable"
theme = "rustdoc"
source = "hide"

[[fragment]]
id = "crate_overview"
source = "docs/rustdoc/crate.md"

[[fragment]]
id = "architecture"
source = "docs/diagrams/architecture.mmd"
source_display = "details"
```

The important constraints are that IDs and output names are fixed by the tool and that arbitrary
output paths are not part of the public Interface.

The tool-owned directory should contain:

```text
docs/generated/merman-rustdoc/<fragment-id>.md
docs/generated/merman-rustdoc/receipt.json
```

Each Markdown fragment contains prose plus inline static SVG/HTML. A diagram-only `.mmd` source is
wrapped as a fragment. Consumers use only Rust built-ins:

```rust
#[cfg_attr(
    doc,
    doc = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/generated/merman-rustdoc/architecture.md"
    ))
)]
pub mod architecture {}
```

Crate-level docs become possible without an item-level proc macro:

```rust
#![cfg_attr(
    doc,
    doc = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/generated/merman-rustdoc/crate_overview.md"
    ))
)]
```

Keeping a tiny helper package is optional. If ergonomics require one, publish a separate
`merman-rustdoc-sidecar` normal library with no renderer dependency. Do not retain the current
proc-macro crate as both a heavy backend and a supposedly light facade.

### Interface invariants and ordering

The `RustdocBundle` Module must enforce:

- all paths are UTF-8, portable, relative to the package root, and remain inside it;
- input and output trees cannot overlap;
- fragment IDs are unique under portable Unicode normalization and case folding;
- output names are deterministic and cannot be selected by arbitrary user paths;
- deterministic runtime, fixed Mermaid baseline, no network, no system fonts/time/randomness;
- strict SVG validation is always enabled;
- non-Mermaid Markdown bytes and source order are preserved;
- output IDs include logical fragment identity and occurrence order, not absolute paths or time;
- a failed render never publishes a partial bundle;
- unknown files in the tool-owned output directory are never silently removed;
- stale deletion is authorized only by a valid previous receipt for the same configuration/output
  owner;
- generated files and receipt are committed as one generation.

`build` order:

1. Resolve `Cargo.toml`, package root, fixed config path, and schema.
2. Validate all IDs, paths, options, limits, and capability requirements.
3. Read every source and local include with bounded input accounting.
4. Scan all Markdown fragments and record source locations.
5. Prepare one deterministic native Merman renderer and render all diagrams.
6. Strictly validate every SVG and compose complete fragment bytes.
7. Build a portable receipt and desired file set.
8. Acquire the existing publication lock, recover only an interrupted owned transaction, and
   revalidate source/target identities.
9. Stage all files, publish the receipt last, and commit atomically.

`check` performs steps 1-7 but does not publish. It re-renders and compares bytes, receipt fields,
and the complete expected file set. This is intentionally stronger than trusting a source hash or
tool version. A missing/stale/tampered output is a normal check failure; a malformed config or
filesystem failure is a configuration/operational error.

Suggested exit classes are `0` fresh/success, `1` content or stale-output failure, `2` invocation
or configuration failure, and `3` lock, I/O, recovery, or publication failure. Diagnostics should
name fragment ID, source path, fence line, and a short source preview.

### Receipt and stale artifacts

Do not reuse the current `merman-generation` v2 receipt as the rustdoc freshness contract. It is
designed for numbered artifact ownership and encodes host-native paths
([`GenerationManifest`](../../crates/merman-cli/src/transaction/format.rs)). Define a
portable `merman-rustdoc-bundle` receipt with:

- schema and format version;
- Merman version, pinned Mermaid baseline, and capability descriptor digest;
- normalized config digest;
- each fragment's ID, source/include paths, source hashes, options, and output hash;
- complete expected file set and generator provenance.

The receipt is for provenance, audit, and safe stale authorization. `check` still recomputes the
rendered bytes. If the receipt is malformed, belongs to another root/config, or names an unsafe
target, `build` must refuse deletion and require explicit cleanup rather than guessing ownership.

### Dependency categories and Adapters

| Dependency | Category | Placement | Adapter/testing strategy |
| --- | --- | --- | --- |
| Config, scanner, HTML, hashes, receipt | In-process | Inside `RustdocBundle` | Interface-level tests |
| Native Merman renderer | In-process | CLI implementation | Real renderer plus private fake for ordering tests |
| Source filesystem | Local-substitutable | Internal implementation | Temporary package fixtures; no public filesystem port |
| Publication transaction | Local-substitutable | Deep `ManagedGenerationPublisher` Module | Real temp filesystem and crash checkpoints |
| Clap process arguments | Process Adapter | Outside Module | CLI contract tests |
| Rust `include_str!` | Rustdoc artifact Adapter | Consumer source | No runtime dependency |
| Network/browser/Node | True external | Forbidden in static lane | No production Adapter |

There are two real callers for a generic publisher (native batch and rustdoc), so a shared Seam is
justified. There is no reason to expose a general renderer trait to downstream crates.

## docs.rs and Cargo Package Workflow

The release workflow should be explicit:

```text
merman-cli rustdoc build
merman-cli rustdoc check
cargo package --list
cargo package
unpack .crate and run cargo doc --offline in a docs.rs-shaped fixture
```

Generated fragments, the configuration, and declared source/include files must be included in the
package. Lock files and transaction evidence must be ignored or excluded. docs.rs then compiles
the consumer crate with ordinary Rustdoc and reads the already packaged fragments; it does not need
the CLI, a Mermaid renderer, a custom Cargo subcommand, or network access.

Do not put a `merman-cli` invocation in `build.rs` and do not rely on docs.rs to run the generation
step. A release/CI gate is the correct owner of freshness. A package consumer can still use
`cargo doc` without having Merman installed.

## Migration and Deletion

### User migration

1. Move inline Mermaid fences into declared `.md` or `.mmd` sources under `docs/`.
2. Move theme/pipeline/source-display choices into the fragment configuration.
3. Replace `#[cfg_attr(doc, merman_rustdoc::merman(...))]` with `#[doc = include_str!(...)]`.
4. Replace `scope = "tree"` with explicit fragment inclusion on each item; no Rust AST traversal is
   required.
5. Move crate-level `//!` diagrams into a declared crate fragment, gaining support that the current
   macro deliberately lacks.
6. Run `build`, commit generated files, and make `check` a CI/release requirement.

The migration deliberately removes `fail = "keep-source"` as a generation policy. A failed
release generation must fail; authors can keep a source-only Markdown fragment in the repository.
Strict SVG sanitation is an invariant, not a consumer option.

### Code and metadata deletion

After a deprecation release, remove:

- the native renderer proc-macro implementation (`doc.rs`, `expand.rs`, `render.rs`, `html.rs`,
  `svg.rs`, `options.rs`, `error.rs`);
- `proc-macro = true`, `merman`, `proc-macro2`, `quote`, `syn`, `roxmltree`, and renderer feature
  edges from `merman-rustdoc`;
- attribute/complete-SVG proc-macro tests, replacing them with generator, receipt, package, and
  docs.rs-shaped tests;
- the `rustdoc-static-svg` artifact profile and feature-matrix rules that require host-side
  complete SVG;
- workspace dependency/member entries and documentation for `scope`, `fail`, and `sanitize`;
- ADR statements that require rustdoc to mirror `complete-svg` in the consumer Cargo graph.

Do not delete the existing batch scanner or transaction implementation. Extract shared behavior,
then keep batch's image-link dialect and rustdoc's inline-fragment dialect separate.

## Embedded-WASM Spike: When It Is Worth Doing

WASM is attractive only for a different success criterion: preserve the current attribute API and
one-command `cargo doc` while shrinking the host closure. The repository already has a pure-WASM
Typst transport and `wasmi` smoke coverage ([`typst_plugin_smoke.rs`](../../crates/xtask/src/cmd/typst_plugin_smoke.rs));
that proves transport feasibility, not a full Rustdoc guest's size, math support, or parity.

The spike should be a separate host/guest artifact, not an immediate replacement in the published
crate:

```text
thin rustdoc proc-macro host
    -> versioned request (source, ID, pipeline, theme)
    -> embedded release-built Merman WASM guest via wasmi
    -> host-side strict SVG validation and HTML wrapping
```

The guest must have no filesystem, network, time, randomness, browser, or WASI imports. Its
manifest must bind protocol version, Merman/Mermaid baselines, capabilities, SHA-256, toolchain,
and legal closure. Host fuel, memory, input, and output limits are mandatory; a trap or ABI
mismatch invalidates the instance.

Admission must be measured on the same host and fixtures:

- host normal+build package closure target at most 30 (proposal, not current evidence);
- cold/warm `cargo doc` wall time, peak RSS, and target growth;
- 1/10/100 diagram latency and repeated-source behavior;
- ordinary, Cytoscape, ELK, math, light/dark, and source-config fixtures;
- normalized SVG DOM and documented browser-dependent residuals;
- raw/stripped/compressed guest size and final `.crate` size;
- offline, read-only, no-CLI docs.rs-shaped build;
- ABI mismatch, corrupt guest, trap, fuel, memory/output limits, and sanitizer failures.

Use crates.io's package limit and an explicit latency/RSS bar as hard gates. One bounded optimization
pass is reasonable; if the guest remains too large, too slow, or too fragile, stop and ship the
CLI-first sidecar lane. Do not add native and CLI fallback features to the same host: additive
feature unification would recreate the original closure and make environment-dependent behavior
hard to diagnose.

## Implementation Phases

1. Add the private `RustdocBundle` Module and a fixture-only `rustdoc build/check` command.
2. Reuse renderer preparation and bounded Markdown scanning; add inline-SVG composition and a
   portable receipt.
3. Deepen publication behind a generic publisher while preserving the existing batch dialect.
4. Add package-list and docs.rs-shaped offline tests; measure generated bundle and `.crate` sizes.
5. Migrate Merman's own Rustdoc examples to external fragments and built-in `include_str!`.
6. Publish a deprecation release for the heavy proc macro, document the migration, and remove it
   after the agreed window.
7. In parallel, run the WASM spike without changing the default path. Promote it only if every gate
   passes and retaining one-step `cargo doc` justifies the additional artifact/ABI maintenance.

## Final Recommendation

If the primary user promise is "enabling Rustdoc diagrams must not impose a large renderer build
on consumers," choose CLI-first checked pre-generation. It is the only option that makes the zero
renderer dependency property structural, works with docs.rs without custom execution, and keeps
Merman's static/offline advantages.

If the primary promise is instead "existing annotated Rust source must render during one ordinary
`cargo doc` command," run the embedded-WASM spike before deleting the API. Treat it as a separate,
versioned artifact experiment with hard closure, performance, package-size, and docs.rs gates.

In either case, do not solve the problem with more feature leaves, `build.rs`, an implicit CLI
fallback, or browser JavaScript as the static default. Move the Seam to the generation artifact,
keep the external Interface small, and let one deep Module own source acquisition, rendering,
validation, receipts, and publication.

This report is research/design only. No production code was modified; existing user and agent
changes were left untouched.
