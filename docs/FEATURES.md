# Choosing Merman capabilities

This page documents the current repository source. The published `0.8.0-alpha.3` packages predate
this capability vocabulary; their release-specific feature names remain documented on that tag.
The Rust snippets below therefore use workspace path dependencies and cannot be mistaken for
features already available from crates.io.

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
| `icons`, `markdown`, `network-icons`, `parallel-markdown`, `shell-completions` | Compiled CLI tool commands | None; the CLI Cargo manifest owns their workflow forwarding |

This column describes only the repository-wide semantic contract recorded in the canonical
descriptor. A Cargo package or product surface may forward additional features to assemble an
operational workflow. Today the `merman` facade forwards `editor` to `analysis`, and forwards each
binary export to `svg`; the corresponding Web and CLI products make the same combinations where
their public workflow requires them. Those owner-specific compile combinations do not turn into
global capability implications.

There is one public convenience aggregate name: `complete-svg`, exposed by the `merman` facade and
the `merman-rustdoc` integration crate. It means `svg + layout-cytoscape + layout-elk + math`. It
does not include system adapters, analysis, ASCII, or binary exports.

There is deliberately no global `preset-*` feature lattice. Cargo features are additive and
cannot express “everything except X”; a large preset table would mix application workflows,
runtime policy, transport packages, and release recipes. Product packages and artifact profiles
select their own direct leaf set instead.

## Pick a workflow

| Workflow | Recommended dependency or package | Typical feature selection |
| --- | --- | --- |
| Deterministic SVG in Rust | `merman` | Default `complete-svg`, or `default-features = false, features = ["svg"]` for basic SVG |
| Full SVG semantics in Rust | `merman` | `default-features = false, features = ["complete-svg"]` |
| Lint and diagnostics | `merman-analysis` | No feature; the crate is default-empty |
| Editor library | `merman-editor-core` or `merman` | `merman` with `analysis, editor` |
| Standalone LSP server | `merman-lsp` | `--no-default-features --features stdio` |
| Complete CLI | `merman-cli` | Its default direct leaf set, or the exact `cli-release` recipe |
| Lean CLI lint | `merman-cli` | `--no-default-features --features analysis` |
| Browser rendering | `@mermanjs/web` or an admitted slim package | Select the npm package, not Cargo features |
| Typst rendering | `@preview/merman` | Select the Typst package; internal WASM profiles are maintainer-only |
| C/C++ embedding | `merman-ffi` | Build the source-only ABI 3 crate with its reproducible artifact recipe; no generic binary SDK is published |
| Flutter/Dart embedding | `merman` on pub.dev | Use the Flutter package, which consumes ABI 3 internally |
| Android/Kotlin embedding | `merman-android-<tag>.aar` | Use the direct JNI AAR from the matching GitHub Release; no remote Maven coordinate is published |
| Apple/Swift embedding | `Merman.xcframework` | Use the UniFFI XCFramework release asset or local SwiftPM package |
| Python embedding | `merman` on PyPI | Use the generated UniFFI wheel for the selected platform |

Node/SSG transport remains a private admission candidate, so there is no public
`@mermanjs/node` package to select yet. Browser WASM is not a supported Node transport.

## Rust examples

### Complete SVG

```toml
[dependencies]
merman = { path = "crates/merman", default-features = false, features = ["complete-svg"] }
```

```rust
use merman::svg::HeadlessRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let renderer = HeadlessRenderer::new().with_diagram_id("example");
    let svg = renderer
        .render_svg_sync("flowchart TD\n  A[Start] --> B[Done]")?
        .expect("diagram detected");
    println!("{svg}");
    Ok(())
}
```

The ordinary `merman = { path = "crates/merman" }` dependency uses the same `complete-svg`
aggregate.
The default operation remains deterministic; it does not read ambient time, time zone, randomness,
or timing state.

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
analysis, ASCII, PNG, JPEG, PDF, both optional layout engines, math, local Iconify loading,
Markdown conversion, native adapters, network icons, parallel Markdown, and shell completions.
Compiled native adapters never change the default runtime policy:

```sh
cargo install merman-cli --version '^0.8.0-alpha.3' --locked
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

For a native registry install, Python provides the shortest complete SDK path:

```sh
python -m pip install --pre merman
```

```python
import merman

engine = merman.MermanEngine()
svg = engine.render_svg("flowchart TD\n  A --> B", None)
```

Flutter uses `flutter pub add 'merman:^0.8.0-alpha.3'` and `Merman.open()`. Android consumes the
matching release AAR through `implementation(files(...))`; Apple consumes the matching
XCFramework through the local Swift package; C and C++ build the source-only `c-abi-native`
artifact profile. The [Flutter](../platforms/flutter/README.md),
[Android](../platforms/android/README.md), [Apple](../platforms/apple/README.md), and
[C ABI](../crates/merman-ffi/README.md) guides provide each transport's copyable first operation
and lifecycle rules; there is no interchangeable generic native binary SDK.

Typst users install one package:

```typst
#import "@preview/merman:0.2.0": mermaid

#mermaid(```mermaid
flowchart TD
  Source --> Document
```)
```

The published Typst profile has SVG, analysis, Cytoscape, and ELK. Math is not advertised until
its pure-WASM font, license, import, and parity admission is complete. Typst always enforces its
constrained resource policy; caller options may tighten it but cannot replace it with an
unbounded profile.

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
