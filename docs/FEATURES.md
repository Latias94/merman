# Choosing Merman capabilities

This page documents the current development source. Registry channels publish independently, so
verify the exact package version and provenance before copying an install command; workspace path
snippets are for source-tree development.

Choose Merman by the operation you need, not by Mermaid diagram family or implementation
dependency. Every parser-capable build uses the same Mermaid 11.16 language model, detector,
configuration, sanitizer, source spans, and family vocabulary. Cargo features only add
user-visible capabilities, output backends, or host adapters.

There are three separate decisions:

1. **Compiled capabilities** decide which APIs and output backends exist.
2. **Runtime policy** decides whether an operation uses deterministic values or explicitly selected
   system adapters.
3. **Resource policy** bounds work performed by an available capability.

The canonical vocabulary is
[`capabilities/feature-surface-v1.json`](../capabilities/feature-surface-v1.json). Reproducible
release recipes are maintained separately in
[`capabilities/artifact-profiles-v1.json`](../capabilities/artifact-profiles-v1.json).
Artifact profiles are not Cargo features and are not part of an application dependency declaration.

## The short version

The public capability leaves are:

| Capability | Meaning | Global semantic implication |
| --- | --- | --- |
| `svg` | SVG rendering | None |
| `analysis` | Diagnostics, validation, and semantic analysis | None |
| `editor` | Parser-backed editor intelligence | None |
| `ascii` | Terminal text output | None |
| `png`, `jpeg`, `pdf` | Independent binary exports | None |
| `layout-cytoscape`, `layout-elk` | Mermaid-compatible layout engines | Each implies `svg` |
| `math` | Math-label rendering | Implies `svg` |
| `system-clock`, `system-timezone`, `system-random`, `system-timing` | Optional native adapters | None; compiled separately and selected explicitly at runtime |
| `icons`, `markdown`, `network-icons`, `parallel-markdown`, `rustdoc`, `shell-completions` | Compiled CLI tool commands | None or descriptor-declared workflow implications; the CLI Cargo manifest owns their forwarding |

This column describes only the repository-wide semantic contract recorded in the canonical
descriptor. A Cargo package or product surface may forward additional features to assemble an
operational workflow. Today the `merman` facade forwards `editor` to `analysis`, and forwards each
binary export to `svg`; the corresponding Web and CLI products make the same combinations where
their public workflow requires them. Those owner-specific compile combinations do not turn into
global capability implications.

The repository-wide result aggregate is `complete-svg`, exposed by the `merman` facade and the
`merman-rustdoc` integration crate. It means `svg + layout-cytoscape + math`; it deliberately does
not include the optional EPL-2.0 ELK implementation. Add the explicit `complete-svg-elk` aggregate
when that closure is intended and its notices/provenance will accompany the artifact. Neither
aggregate includes system adapters, analysis, ASCII, or binary exports.

Native binding crates (`merman-bindings-core`, `merman-ffi`, `merman-uniffi`, and the internal
`merman-android-jni` transport) additionally expose the owner-local `native-runtime` feature. It
atomically compiles the system clock, time-zone, and random adapters because the binding
`runtime_policy: "native"` contract is callable only with the complete set. `native-runtime` is not
a capability ID or a global Cargo preset: runtime discovery continues to report the concrete
`system-clock`, `system-timezone`, and `system-random` adapter IDs, and lower-level Rust crates plus
the CLI retain their granular leaves.

There is deliberately no global `preset-*` feature lattice. Cargo features are additive and
cannot express “everything except X”; a large preset table would mix application workflows,
runtime policy, transport packages, and release recipes. Product packages and artifact profiles
select their own direct leaf set instead.

## Pick a workflow

| Workflow | Recommended dependency or package | Typical feature selection |
| --- | --- | --- |
| Deterministic SVG in Rust | `merman` | Default `complete-svg`, or `default-features = false, features = ["svg"]` for basic SVG |
| Full SVG semantics in Rust | `merman` | `default-features = false, features = ["complete-svg"]` |
| Full SVG semantics plus ELK | `merman` | `default-features = false, features = ["complete-svg-elk"]` |
| Lint and diagnostics | `merman-analysis` | No feature; the crate is default-empty |
| Editor library | `merman-editor-core` or `merman` | `merman` with `analysis, editor` |
| Standalone LSP server | `merman-lsp` | `--no-default-features --features stdio` |
| Complete CLI | `merman-cli` | Default direct leaves without ELK, or the exact `cli-release` recipe with ELK |
| Lean CLI lint | `merman-cli` | `--no-default-features --features analysis` |
| Checked Rustdoc fragments | `merman-cli rustdoc` | CLI `rustdoc`; documented crates consume committed files through native `include_str!` |
| One-step Rustdoc attributes | `merman-rustdoc` | Default `complete-svg`, or an explicit smaller renderer closure |
| Browser rendering | `@mermanjs/web` or an admitted slim package | Select the npm package, not Cargo features |
| Typst rendering | `@preview/merman` | Select the Typst package; internal WASM profiles are maintainer-only |
| C/C++ embedding | `merman-ffi` | Build the source-only ABI 3 crate with its reproducible artifact recipe; source builds use `native-runtime` when native runtime policy is required |
| Flutter/Dart embedding | `merman` on pub.dev | Use the Flutter package, which consumes ABI 3 internally |
| Android/Kotlin embedding | `merman-android-<tag>.aar` | Use the direct JNI AAR from the matching GitHub Release; no remote Maven coordinate is published |
| Apple/Swift embedding | `Merman.xcframework` | Use the UniFFI XCFramework release asset or local SwiftPM package |
| Python embedding | `merman` on PyPI | Use the generated UniFFI wheel for the selected platform |

Node/SSG users can select the experimental `@mermanjs/node` alpha package. It installs a small
loader plus one exact-version N-API platform package and uses the deterministic static-SVG recipe:
SVG and both layout backends, but not math, analysis, ASCII, or binary export. Browser WASM is not
a supported Node transport or fallback.

## Rust examples

### Complete SVG

```toml
[dependencies]
merman = { path = "crates/merman", default-features = false, features = ["complete-svg"] }
```

```rust
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = Renderer::new().render(RenderRequest::svg(
        "flowchart TD\n  A[Start] --> B[Done]",
        OperationControl::new(),
        SvgRequest::default(),
    ))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };
    std::fs::write("diagram.svg", svg.svg())?;
    Ok(())
}
```

The ordinary `merman = { path = "crates/merman" }` dependency uses the same `complete-svg`
aggregate.
The default operation remains deterministic; it does not read ambient time, time zone, randomness,
or timing state.

When several SVGs share one HTML document, assign a unique
`SvgRequest.options.diagram_id` to every operation. Use `Renderer` plus typed targets for reusable
configuration, semantic/layout inspection, binary export, presentation, resource policy, or an
explicit SVG pipeline. The complete set of copyable task examples lives in
[`crates/merman/examples`](../crates/merman/examples/README.md).

### Basic SVG without optional engines

```toml
[dependencies]
merman = { path = "crates/merman", default-features = false, features = ["svg"] }
```

If an input requires a compiled-out layout engine or math renderer, the operation returns a typed
`missing-capability` error. It does not silently substitute a different semantic result.

### Analysis and editor support

```toml
[dependencies]
merman-analysis = { path = "crates/merman-analysis", default-features = false }
merman = {
    path = "crates/merman",
    default-features = false,
    features = ["analysis", "editor"],
}
```

On the `merman` facade, enabling the `editor` Cargo feature also enables `analysis`. The canonical
`editor` capability has no global implication, so another package must declare the pair explicitly
when its workflow exposes both. Neither capability adds or removes a Mermaid diagram family.

### Explicit system adapters

Compile only the adapters required by the host:

```toml
[dependencies]
merman = {
    path = "crates/merman",
    default-features = false,
    features = [
        "complete-svg",
        "system-clock",
        "system-timezone",
        "system-random",
    ],
}
```

Compilation makes an adapter available; it does not select it. Use
`RenderEnvironment::try_native()` or the binding option
`{"runtime_policy":"native"}` explicitly. If the required adapter is absent, the request returns
`missing-capability`.

Binding crates intentionally do not expose those three Cargo leaves separately. Select the atomic
binding feature instead:

```toml
[dependencies]
merman-ffi = {
    path = "crates/merman-ffi",
    default-features = false,
    features = ["svg", "native-runtime"],
}
```

The binding remains deterministic until `{"runtime_policy":"native"}` is selected. Its runtime
catalog reports `system-clock`, `system-timezone`, and `system-random`, not `native-runtime`, because
those concrete IDs describe callable adapters while the Cargo aggregate describes how the artifact
is assembled.

### Independent exports

Binary output is opt-in and additive:

```toml
[dependencies]
merman = {
    path = "crates/merman",
    default-features = false,
    features = ["svg", "png", "pdf"],
}
```

`png`, `jpeg`, and `pdf` are separate capabilities. They are not silently included in the
complete SVG aggregate and should not be added to every native SDK without a product reason.

## CLI

`merman-cli` is the browserless Mermaid CLI replacement. Its normal default includes SVG,
analysis, ASCII, PNG, JPEG, PDF, Cytoscape layout, math, local Iconify loading,
Markdown conversion, checked Rustdoc fragment generation, native adapters, network icons, parallel
Markdown, and shell completions.
The separately assembled `cli-release` artifact additionally includes the ELK layout engine and
its EPL-2.0 notices; a source install with ordinary defaults does not imply ELK availability.
Compiled native adapters never change the default runtime policy:

```sh
cargo install --git https://github.com/Latias94/merman --locked merman-cli
printf 'flowchart TD\n  A --> B\n' | merman-cli render - --output diagram.svg

merman-cli render --runtime deterministic diagram.mmd
merman-cli render --runtime native diagram.mmd
merman-cli parse --system-timing diagram.mmd
```

`--runtime` defaults to `deterministic`. Native mode explicitly requests system clock, complete
time-zone rules, and randomness; `--system-timing` is a separate opt-in. A missing requested
adapter returns the CLI's invalid-configuration exit status instead of falling back.

For a lean lint executable:

```sh
cargo run -p merman-cli --no-default-features --features analysis -- lint diagram.mmd
```

For a complete release build, use the repository's `cli-release` artifact profile. Do not use a
bare `cargo build -p merman-cli` as a release proof: Cargo feature unification can change the
effective closure.

`icons` enables local Iconify JSON, `node_modules`, and `file://` sources. `network-icons` adds
the HTTP client and `--allow-network`; network access remains an explicit runtime permission.
Likewise, `markdown` enables serial document conversion without analysis commands.
`parallel-markdown` implies `markdown` and adds only the Rayon worker pool and `--jobs` to
`batch` and Markdown-mode `mmdc`. Disabling it does not remove Markdown support or change chart
numbering, source order, diagnostics ordering, resource admission, or transaction semantics.
Use `--no-default-features --features markdown` for the smallest sequential Markdown CLI and
`--no-default-features --features parallel-markdown` when measured throughput justifies Rayon.

## Rustdoc

Rustdoc has two independently distributed static-SVG workflows. The CLI `rustdoc` tool leaf
enables deterministic SVG rendering with Cytoscape and math for an explicit `build/check` authoring
workflow; it does not make `merman-rustdoc` a dependency. Generated Markdown and its portable receipt are committed,
packaged, and consumed with Rust's standard `#[doc = include_str!(...)]` or
`#![doc = include_str!(...)]`. This gives the documented crate zero attributable Merman packages
in its normal/build Cargo graph, supports crate-level docs, and makes diagram updates reviewable.

The `merman-rustdoc` package remains the independent one-step attribute workflow. Its default
`complete-svg` feature intentionally compiles SVG, Cytoscape, and math into the proc-macro host.
The explicit `complete-svg-elk` feature adds the EPL-2.0 ELK closure when a documentation artifact
needs it. Optional dependency gating can keep that closure out of ordinary builds, but selecting
the explicit ELK feature, `--all-features`, or an artifact profile that lists `layout-elk` compiles
it.

| Concern | Checked CLI generation | Attribute macro |
| --- | --- | --- |
| Cargo cost for documented crate | No renderer or proc-macro dependency | Selected native renderer closure |
| Trigger | `merman-cli rustdoc build/check` before `cargo doc` | `cargo doc` macro expansion |
| Owned files | Committed Markdown fragments and `receipt.json` | Annotated Rust source; generated HTML only |
| docs.rs | Reads files already present in the uploaded crate | Builds the enabled optional macro dependency |
| Rollback | Git restores or regenerates the managed bundle | Git restores source/feature selection |

Neither path invokes or falls back to the other. Include a generated fragment at most once on one
Rustdoc page because its static SVG IDs are deterministic within that fragment. See the
[`merman-cli` guide](../crates/merman-cli/README.md#rustdoc-fragments) and
[`merman-rustdoc` guide](../crates/merman-rustdoc/README.md) for runnable configuration, Rust, CI,
and package examples.

## Browser, Typst, and native packages

Browser package names are the user-facing selection mechanism:

| Package | Compiled workflow | Status |
| --- | --- | --- |
| `@mermanjs/web` | SVG, analysis, editor, ASCII, Cytoscape, ELK, and math | Complete browser package |
| `@mermanjs/web-render` | SVG, Cytoscape, ELK, and math | Complete SVG-only package |
| `@mermanjs/web-analysis` | Analysis | Slim package |
| `@mermanjs/web-editor` | Analysis and editor | Slim package |
| `@mermanjs/web-ascii` | ASCII | Slim package |

Browser packages require a browser realm or worker. They are not Node or SSR transports.

Install the complete browser package and render after initializing its single WASM artifact:

```sh
npm install @mermanjs/web@alpha
```

```ts
import { initMerman, renderSvg } from "@mermanjs/web";

await initMerman();
const svg = renderSvg("flowchart TD\n  A --> B");
```

For a native registry install, Python provides the shortest default SDK path:

```sh
python -m pip install --pre merman
```

```python
import merman

engine = merman.MermanEngine(None, None)
svg = engine.render_svg("flowchart TD\n  A --> B", None)
```

The default Android, Apple, Python, and Flutter native packages include SVG, both supported layout
engines, ASCII, analysis, validation, and document analysis. They omit math, PNG, JPEG, PDF, and
native runtime adapters to reduce distributed binary size. Because these are prebuilt ELK artifacts,
their package-specific notices and source provenance are part of the release contract. Their
generated APIs keep those operation names for custom current-contract libraries; inspect the runtime catalog and handle typed
missing-capability errors before exposing optional output choices.

Flutter's current published baseline uses `flutter pub add 'merman:^0.8.0-alpha.5'` and `Merman.open()`. The workspace source candidate is `0.8.0-alpha.6`; do not present that candidate as a pub.dev installation until its registry evidence exists. Android consumes the
matching release AAR through `implementation(files(...))`; its Kotlin surface is direct JNI
transport API 1 rather than C ABI 3. Apple consumes the matching
XCFramework through the local Swift package; C and C++ build the source-only `c-abi-native`
artifact profile. The [Flutter](../platforms/flutter/README.md),
[Android](../platforms/android/README.md), [Apple](../platforms/apple/README.md), and
[C ABI](../crates/merman-ffi/README.md) guides provide each transport's copyable first operation
and lifecycle rules; there is no interchangeable generic native binary SDK.

Typst is an independently released package. Verify the registry version before installing; the
source tree currently prepares the `0.3.0` wrapper:

```typst
#import "@preview/merman:0.3.0": mermaid

#mermaid(```mermaid
flowchart TD
  Source --> Document
```)
```

The source tree prepares the `0.3.0` Typst wrapper from Merman `0.8.0-alpha.6`, requiring Typst `0.15.0`, with SVG, analysis, Cytoscape, and ELK. This package rebuild removes ICU4X collation data and generated font-metric tables from the production WASM closure while retaining the deterministic Unicode-aware measurement fallback. Build it locally and use Typst's `--package-path` until registry publication is verified. The package includes the ELK dependency closure and its accompanying EPL-2.0 notices; Math is not advertised until its pure-WASM font, license, import, and parity admission is complete. The source package always enforces its constrained resource policy; caller options may tighten it but cannot replace it with an unbounded profile. See the [Typst package guide](../distribution/typst/merman/README.md) for the published/source version boundary.

Native bindings expose the same flat runtime catalog. The catalog contains stable
`capability_ids`, `operation_ids`, and `output_ids`. Do not infer capabilities from exported
symbols, a Cargo feature name, or a package name.

## Runtime and resource policy

Runtime policy is independent from compiled capabilities:

- deterministic clock, UTC time zone, and fixed randomness are the default;
- native policy is an explicit operation choice;
- `system-timing` is an independent diagnostic adapter;
- text measurement is selected by the operation environment, not by a diagram feature.

Resource profiles bound source bytes, model cardinality, nesting, layout work, SVG size, and
embedded media:

| Profile | Intended use |
| --- | --- |
| `interactive` | General applications and public binding defaults |
| `constrained` | Public or untrusted submissions |
| `trusted-native` | Controlled local CLI or batch work |
| `unbounded-for-trusted-input` | Explicitly trusted input with outer isolation |

These profiles are policy starting points, not Mermaid semantic limits. Hosts handling hostile
input must also enforce process memory, timeout, and concurrency boundaries. Query the loaded
runtime catalog for the exact limits.

## Artifact profiles and measurements

An artifact profile records an exact Cargo package, target, direct feature set, default-feature
setting, and expected semantic IDs. Product owners separately define runtime policy, resource
policy, package contents, evidence receipts, and distribution gates. Exclusion claims require both
the exact artifact recipe and its owner-specific dependency or size evidence.

For a `target-set` recipe, dependency evidence is checked independently for every declared target.
For a `host` recipe, the build still uses the executing host, while the normal-dependency probe uses
`x86_64-unknown-linux-gnu` as its reference target and excludes build and proc-macro edges. Cargo
resolves proc-macro normal dependencies for the executing host, so complete package/version evidence
is authoritative only on that Linux reference host. Other hosts still enforce required-package,
forbidden-package, and forbidden-feature claims. The verifier deliberately does not freeze every
transitive package behind an opaque digest; `Cargo.lock`, dependency policy, legal reports, and
artifact measurements retain their natural ownership.

When comparing builds, record the target, compiler, lockfile, direct feature set, uncompressed and
compressed sizes, dependency closure, licenses, and advisory results. A feature name alone is not a
size guarantee because Cargo unifies features across the dependency graph.

## Removed feature names

The old names are intentionally not aliases. Update manifests once rather than carrying two
vocabularies:

| Removed name | Replacement |
| --- | --- |
| `full`, `core-full`, `tiny`, registry profiles | Low-level crates are default-empty; choose observable leaves |
| `render` | `svg` |
| `raster` | One or more of `png`, `jpeg`, `pdf` |
| `cytoscape-layout` | `layout-cytoscape` |
| `elk-layout` | `layout-elk` |
| `ratex-math` | `math` |
| `host-*`, `core-host` | `system-clock`, `system-timezone`, `system-random`, `system-timing` |
| `preset-*`, `*-no-elk` | Direct positive leaf features; use an artifact profile for exact recipes |

Do not add one feature per diagram. A diagram family belongs to the shared language contract; a
public feature is justified only by a user-visible API, output, reusable engine, host adapter,
or compiled CLI tool with a meaningful closure boundary.
