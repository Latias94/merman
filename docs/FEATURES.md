# Choosing Merman capabilities

This guide describes the current source, including [Theme v2](rendering/custom-diagram-themes.md)
and Mermaid `12.3.0` compatibility. Use the [host guide](release/PREBUILT_PRODUCTS.md) for
published channel versions and the [upgrade index](release/README.md) for migration steps. Workspace
path snippets are for source development. See the [Rust embedding guide](rendering/RUST_EMBEDDING.md)
for host integration.

The source candidate selects Mermaid `12.3.0`, with the exact `elkjs 0.12.1` behavior oracle,
Cytoscape `3.34.4`, and reference CLI `12.2.0`. The selected
[reference graph](../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json) and its generated constants
own these reference identities; older artifacts retain their release baseline. See the
[Mermaid 12.3 migration](migrations/mermaid-12-3.md) for its Rust and SVG changes.

Choose the diagram families your application accepts and the operations it performs. Positive
`diagram-*` Cargo features select data, source parsers, or output handlers at the owning package. Selected
families preserve the pinned Mermaid semantics; lightweight detection, configuration namespaces,
and sanitization remain available independently. Output, layout, math, and host-adapter features
are separate choices.

There are four separate decisions:

1. **Diagram families** select the owning package's data, source parsers, or output handlers. Only source-parser selection admits a language.
2. **Compiled capabilities** decide which APIs and output backends exist.
3. **Runtime policy** decides whether an operation uses deterministic values or explicitly selected
   system adapters.
4. **Resource policy** bounds work performed by an available capability.

The canonical vocabulary is
[`capabilities/feature-surface-v1.json`](../capabilities/feature-surface-v1.json). Reproducible
release recipes are maintained separately in
[`capabilities/artifact-profiles-v2.json`](../capabilities/artifact-profiles-v2.json).
Artifact profiles are not Cargo features and are not part of an application dependency declaration.

## The short version

The public capability leaves are:

| Capability | Meaning | Global semantic implication |
| --- | --- | --- |
| `svg` | SVG rendering | None |
| `analysis` | Diagnostics, validation, and semantic analysis | None |
| `fixes` | Requested replacement generation for analysis projections | Implies `analysis` |
| `editor` | Parser-backed editor intelligence | None |
| `ascii` | Terminal text output | None |
| `png`, `jpeg`, `pdf` | Independent binary exports | None |
| `layout-dagre`, `layout-elk`, `layout-cytoscape` | Mermaid-compatible layout engines | Each implies `svg` |
| `layout-shoal` | Experimental Merman layered layout and orthogonal edge routing | Implies `svg` |
| `math` | Math-label rendering | Implies `svg` |
| `native-fonts` | Explicit font contexts for shared text measurement and native painting | Facade and CLI forward SVG; no codec requirement |
| `system-clock`, `system-timezone`, `system-random`, `system-timing` | Optional native adapters | None; compiled separately and selected explicitly at runtime |
| `icons`, `markdown`, `network-icons`, `parallel-markdown`, `rustdoc`, `shell-completions` | Compiled CLI tool commands | None or descriptor-declared workflow implications; the CLI Cargo manifest owns their forwarding |

This column describes only the repository-wide semantic contract recorded in the canonical
descriptor. A Cargo package or product surface may forward additional features to assemble an
operational workflow. Today the `merman` facade forwards `editor` to `analysis`, and forwards each
binary export to `svg`; the corresponding Web and CLI products make the same combinations where
their public workflow requires them. Those owner-specific compile combinations do not turn into
global capability implications.

Current Rust source builds also expose owner-local `presentation`, `compat-json`, and
`editor-facts` features on the facade and core. These are projection boundaries, not new
transport capability IDs. Defaults-off family selections and ASCII use canonical semantics;
`svg` forwards `presentation`, while the facade's legacy JSON API requires explicit
`compat-json`. Rich language tooling selects its required editor facts independently. See the
[semantic/full migration](migrations/render-reuse-host-integration.md#reuse-parsed-source-in-rust).

Benchmark executables have their own feature requirements. The full `pipeline` bench selects
`svg,compat-json,editor-facts` even for a filtered SVG timing group; its recipe does not define
the feature requirements of an SVG application. Use the
[benchmark guide](performance/BENCHMARKING.md#native-criterion-benchmarks) and the owning
[`[[bench]]` manifest entries](../crates/merman/Cargo.toml) for current commands.

`merman-model/terminal-text` owns shared terminal normalization and Unicode width helpers. ASCII and facade diagnostics explicitly select it; direct core semantic parsing does not. `merman-model` has no source ingestion or rendering dependency. ASCII, Render, and Export consume shared models or SVG without a production core dependency; the [model/source migration](migrations/model-source-boundaries.md) covers package selection and breaking Rust imports.

The current development branch retains `complete-svg = ["svg", "layout-cytoscape", "layout-dagre"]` and
`complete-svg-elk = ["complete-svg", "layout-elk"]` on `merman` and `merman-rustdoc`.
Neither aggregate selects diagram families or math. This changes their previously math-enabled
behavior: add `math` explicitly, for example `features = ["complete-svg-elk", "math"]`, when
upgrading a mathematical-label consumer. Direct output/layout leaves remain available. The facade defaults to
`all-diagrams + svg + layout-cytoscape + layout-dagre + layout-elk`, without math. Ordinary prebuilt SVG
products, including Web full/render and official CLI archives, select math explicitly.
CLI source defaults and the `rustdoc` leaf omit it; custom source builds choose their own leaves.
Installed `0.9.3` artifacts retain their published recipes.
The ELK notices and source provenance still accompany artifacts that include it. To omit ELK,
disable defaults and select required families plus `svg` and the required layout leaves.

Use `cargo tree` with the intended consumer features to inspect dependency costs. Package counts
include build inputs and are not executable-size or compilation-time measurements. Core syntax,
configuration, HTML and URL normalization remain part of the precise parser contract even when
output backends are disabled.

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

### Try the experimental Shoal layout

The hosted Playground includes Shoal for the Merman SVG preview. Enable `layout-shoal` on the Rust `merman` dependency or build the CLI with `cargo build --locked -p merman-cli --features layout-shoal`. Check `merman-cli capabilities` for `layout-shoal`, then select the engine in the diagram's Mermaid frontmatter.

```text
---
config:
  layout: shoal
---
flowchart LR
    A[Review] -->|approved| B[Publish]
```

Run `target/debug/merman-cli render diagram.mmd --theme studio --appearance dark -o diagram.svg` from the checkout. Select `dagre` or `elk` in the same field to compare drawings in a build containing those engines. Shoal uses the family's measured shapes and labels, including effective theme fonts and edge-label padding.

Set `{"layout":"shoal"}` in the Playground's **Config** tab to select Shoal for the workspace. A `layout` value in **Config** takes precedence over frontmatter. To make Shoal the CLI default for supported diagrams, save the same JSON to a file and pass `--config-file path/to/mermaid.json`. A diagram's frontmatter can override the CLI default. Rust hosts can set the same site configuration on `MermaidEngine`. [Rust embedding guide](rendering/RUST_EMBEDDING.md#replace-the-previous-renderer)

Shoal draws Flowcharts in all four directions, including cycles, self-loops, parallel labeled edges, nested subgraphs and group endpoints. Agentflow uses the same engine after projecting visible nodes, collapsed flows and connectors. Architecture uses the declared port faces on services, junctions and groups for orthogonal routing. C4 keeps its measured elements and routes relationships across nested boundaries. Class retains measured class boxes, namespaces and multiplicity labels. State supports explicit composite directions, concurrent regions and self-transitions. ER routes identifying and non-identifying relations with cardinality markers at both ends. Requirement supports relation labels and containment markers. Usecase routes relationships across system boundaries and clips them to actor and use-case contours.

Use a `swimlane-beta` header with `layout: shoal` to align work across lanes. Root lanes share rank positions and align their borders. Use `layout: shoal` in Block to try orthogonal edge routing on the authored grid. Block uses its usual route for an edge if the router cannot find a clear alternative.

A build without Shoal reports the missing capability when one of these diagrams requests it. Rust callers can also pass measured graphs directly to the standalone crate. [Shoal crate](../crates/merman-layout-shoal/README.md)

For supported diagrams, request a width with `merman-cli render diagram.mmd --fit-width --width 640 -o diagram.svg`.
In the Merman SVG preview, use **Tools → Fit preview width** when the diagram selects Shoal. In VS Code, select a
Shoal-enabled CLI with `merman.cli.path` and set `merman.preview.fitWidth` to the desired width in
CSS pixels. Use `0` for natural placement. The extension reuses hints across successive SVG
previews when the CLI advertises frame support. Swimlane can capture a placement frame with its
shared rank grid. Swimlane and Block use their natural geometry width. [Width and frame support](rendering/HEADLESS_LAYOUT.md#shoal-width-adaptation-and-preview-frames)
[Host integration](rendering/HEADLESS_LAYOUT.md#shoal-width-adaptation-and-preview-frames) ·
[Playground experiment](../platforms/web/README.md#try-shoal-in-playground) ·
[Layout benchmark](performance/2026-11-24-shoal.md)

Use `layout: tidy-tree` for Mindmap's compact two-sided tree layout. It is available with the
Mindmap SVG renderer and uses the diagram's measured node sizes and contours.

### Optional CLI Fonts

The current-source default CLI build and official release profile include
`native-fonts` and `system-fonts`. Mermaid SVG defaults to deterministic text
measurement without a native font context or system discovery. Mermaid PNG/JPEG/PDF
share one native context between measurement and painting. The current release
profile queries the platform on demand on Windows, macOS, and Linux. Ordinary
Cargo source builds use bounded discovery. These defaults apply to Mermaid
input in `render`, `batch`, and `mmdc`; raw SVG conversion only paints existing
geometry and is never relaid out. Host fonts can change wrapping, node sizes and
image/PDF snapshots.

CLI builds with `system-fonts` also use their default provider for raw SVG PNG,
JPEG, and PDF. System discovery uses the finalized SVG's text inventory, including
text inside nested `<svg>` elements. Path-only input skips discovery and can
render with `--font-system off` and no font files. Use explicit fonts to paint
text with `--font-system off`. `--font-max-bytes` applies to the conversion font
context. With only that option, raw SVG uses the build's default provider.

In the CLI `native_fonts` capability document,
`default_policy_scope: "mermaid-svg-input"` scopes the existing
`default_measurement: "deterministic"`, `default_font_source: "none"`, and
`default_system_discovery: "disabled"` values to SVG. The additive
`encoded_output_defaults` object reports `measurement: "native"` and the active
system-font provider. The bounded provider reports `bounded-system-discovery`
with `before-layout`. The platform provider reports `platform-managed-system-query`
with `platform-initialization-then-on-demand-families`.
With no CLI `system-fonts` capability, those three encoded values are
`deterministic`, `none`, and `disabled`. This discovery and runtime-default change
does not change an ABI.

Explicit `--font-file` or `--font-dir` sources automatically select native
measurement with a closed font set, even with `--font-measurement native`; add
`--font-system on` to allow system fallback. Without explicit sources,
`--font-measurement native` explicitly permits system discovery when `system-fonts`
is compiled. `--font-system on` explicitly selects system fonts and native
measurement. `--font-measurement deterministic` explicitly selects the default
SVG estimates or the previous encoded-output estimates without font sources;
encoded output still uses native painting, which can acquire system fonts through
the legacy codec path. `fontFamily` selects among available faces, not a file, and
does not change the output's measurement policy. `--runtime native` controls other
system adapters and does not select the font policy.

Fixed font files avoid full system catalogue discovery and provide the faster,
reproducible native route when font contents and Mermaid configuration are pinned.
System catalogue discovery is a separate cost from actual-font text measurement.
System-font output is host-dependent. Rust library, FFI, ASCII, and committed
Rustdoc generation defaults remain unchanged. For a minimal CLI build with
explicit fonts and SVG output:

```powershell
cargo run -p merman-cli --no-default-features --features diagram-flowchart,layout-dagre,native-fonts -- mmdc -i diagram.mmd -o diagram.svg --font-file fonts/MyFont-Regular.ttf --font-file fonts/MyFont-Bold.ttf --font-system off
```

Add `png`, `jpeg`, or `pdf` individually for binary output. Each CLI codec feature
also selects the CLI's `system-fonts` and `native-fonts` capabilities, so lean codec
builds receive the same native default and explicit font controls. `native-fonts`
alone does not enable codecs or system discovery. These CLI combinations do not
change library codec defaults. Cargo features are additive: compiled system-font
support remains available when an explicit-font operation uses `--font-system off`,
but that operation's font set stays closed.

The CLI `system-fonts` feature selects automatic native measurement for Mermaid
encoded output and permits explicit native/system font options for SVG. Minimal
builds with only `native-fonts` retain deterministic defaults and require explicit
fonts for native measurement. Dependency feature union alone does not grant the
CLI that permission. `capabilities --json` reports the compiled dependency
capability and the CLI runtime policy separately. If a selected native route cannot
load usable fonts, it fails instead of silently succeeding with deterministic
estimates.

The optional CLI feature `platform-system-font-query` adds an explicit system
font policy. Use `--font-system platform` to query the operating system's font
service. The backend uses DirectWrite on Windows, Core Text on macOS,
and Fontconfig on Linux and FreeBSD.
`--font-system bounded` explicitly selects bounded
discovery in builds with platform query support. `--font-system on` selects the
build's default system provider. Automatic Mermaid encoded output uses the same
provider when no explicit fonts or font policy are supplied. The official release
recipe selects `platform-system-font-default`, which enables both policy options
and chooses the platform provider.

On Linux/FreeBSD, platform discovery distinguishes an unavailable Fontconfig runtime
from an available runtime with an empty configured font set. In a build whose default
provider is platform, automatic selection and `--font-system on` fall back to bounded
discovery only when the runtime cannot load. Explicit `--font-system platform` reports
that error instead. An intentionally empty Fontconfig configuration does not trigger
fallback, and neither do font-data, resource-limit, or cancellation errors.

Fontconfig ranks the requested family names together, honoring strong and weak
aliases. For a weak `system-ui` alias, explicitly supplied fonts compete with
installed fonts under the same Fontconfig language and family ranking. Merman
verifies scan-time aliases against the physical face and keeps
alternatives with overlapping coverage. A text face can therefore supply a glyph
when a preferred face is pictorial or covers only a subset. If a font file
disappears, measurement and painting can reuse a snapshot already admitted. For
new selections, the resolver tries the remaining candidates and reports the first
missing path if none succeeds.

Operating system queries have their own runtime cost. Merman's table preflight
reads count toward a separate discovery budget. For Fontconfig variation sequences,
preflight checks the requested base and selector before reading a full character
map. Selected font bytes use bounded immutable snapshots shared by measurement and
painting, including the default system face acquired on demand. The resolver
verifies coverage on admitted data.

Workspace builds repair each missing shaping cluster with one physical face,
preserving glyphs in the surrounding clusters. Platform fallback tries the
authored font stack, then caller fonts in source order, then the platform's
fallback candidates. SVG text inherits `xml:lang` and a `<tspan>` can override it.
Rust hosts can set the default language for Mermaid and untagged SVG through
`NativeFontContext::with_language`. Windows applies DirectWrite's fallback scale
to both cluster metrics and painted glyphs.

On macOS, Core Text looks up families on demand and receives the missing cluster
and its language. An explicit base face can use its registered cascade after its
OpenType tables match. An unregistered face can use the Helvetica cascade when
system fallback is enabled. Most selected faces use a standalone snapshot at
index zero. If the collection fits the remaining budget and its members share an
outline table larger than half the collection, Merman can retain one TTC or OTC
and verify the selected member by its native tables. Measurement and painting
then share the same bytes and member index.

The default font-data budget per native context increases from 64 MiB to 256 MiB.
It covers explicit inputs, including whole files from `--font-dir`, and selected
system font data. A large color or bitmap table can exceed it. The CLI reports
the requested total so far and suggests a retry such as `--font-max-bytes 512MiB`.
Bounded discovery has a separate budget for scanning character maps. Native
measurement has its own per-diagram `--font-max-measurements` limit.

A context identity includes its policy, initial font bytes, and the system default
family name when applicable. Successful platform selections are fixed for measurement
and its derived exports. New operations query the current system again. Selection
metadata is bounded separately from the optional measurement cache. Platform discovery retains
selected OpenType variation coordinates for measurement and painting. Authored variation settings
take precedence over platform coordinates.
Platform fallback can choose different faces and change diagram geometry. For
fixed font selection, supply explicit font files with `--font-system off`.
In builds with platform queries, `on`, `bounded`, and `platform` all allow system
fallback for explicit font sources. `capabilities --json` lists those values in
`native_fonts.platform_system_query`. `system_resources_bounded` describes the default.

The CLI feature `experimental-system-font-query` is an alias for
`platform-system-font-default`. Rust callers select platform queries through
`NativeFontOptions::load_platform` with the library's
`experimental-system-font-query` feature. Ordinary `load` uses bounded discovery.

`--font-file` and `--font-dir` are repeatable and preserve argument order.
Selected directory roots, including system font roots such as `~/.fonts`, resolve
symlinks and ancestor aliases before scanning. Directory contents use bounded,
stable traversal and skip discovered symlinks, including recursive links. Earlier
equal family/style/weight faces take precedence. The loader reads TTF, OTF, and TTC
files from local sources only. Native measurement and painting share a resolver for missing families.
Use the family names registered from the files in Mermaid's `fontFamily` stack.
For example, if resvg registers your font as `Recursive`, use `Recursive` in the stack.
With only that file loaded, an external resvg can omit labels that request the web alias
`Recursive Variable`.
[Matching standalone SVG labels to resvg fonts](rendering/RUST_EMBEDDING.md#match-standalone-svg-labels-to-resvg-fonts)

Explicit sources select native measurement automatically. The shared backend
handles all 23 text-measurement protocol operations across layout, wrapping,
SVG bounds, and computed lengths. Diagram families use this backend through
Merman's existing measurement routes: it is not limited to titles or to one
diagram family. Font-driven node and label sizing, advances, horizontal/vertical
bounds, and wrapping probes use the selected font context; Merman retains its
existing whitespace and line-breaking rules. Fixed diagram geometry and CSS
line-height rules are not replaced with glyph bounds. Missing-glyph requests
explicitly fall back to deterministic measurement. This is not browser pixel
parity, and arbitrary HTML/CSS typography remains outside the native contract.
Explicit sources with `--font-measurement deterministic` are
rejected for Mermaid input. Custom `--css-file` and `themeCSS` are also rejected
with native Mermaid measurement because they can change typography after layout.
Parser-owned declarations such as letter spacing, CSS font shorthand and custom
line height are rejected when the measurement protocol cannot represent them.
Flowchart native mode requires an effective `px` theme font size (the equivalent
default numeric `16` is accepted); ambiguous numeric/unit spellings and a root
bold `fontWeight` are rejected where existing layout and CSS disagree. Set bold
or italic on supported node/class declarations instead.
For encoded output that needs this CSS or unsupported typography, use
`--font-measurement deterministic` without font sources to retain the previous
layout and CSS behavior; existing CSS security admission still applies. ASCII,
parser/analysis, library defaults and host-owned FFI measurement policies are
unchanged.

SVG output does not embed fonts or convert text to outlines. Consumers must have
the same fonts to reproduce measured geometry. Raw SVG conversion rejects
`--font-measurement` and `--font-max-measurements`. Native export font selection
honors `xml:lang`, including inherited tags and `<tspan>` overrides.
Respect font redistribution and embedding licenses.
Cousine fixtures are test-only and are not production assets.

Rust hosts load `merman::svg::export::NativeFontOptions` and install the resulting
context with `SvgEnvironment::with_native_font_context`. Typed binary rendering
reuses that context automatically. Direct exporters use the same context via
`RasterOptions::with_font_context` or `PdfOptions::with_font_context`. Hosts
implementing a custom low-level measurement route must call `context.check()`
at their fallible output boundary, since the host measurement protocol otherwise
turns errors into fallback. Font bytes and bounded caches belong to the requesting
operation or batch and never accumulate user fonts in a process-global cache.

| Workflow | Recommended dependency or package | Typical feature selection |
| --- | --- | --- |
| Deterministic SVG in Rust | `merman` | Defaults (`all-diagrams + svg + layout-cytoscape + layout-dagre + layout-elk`), or `default-features = false, features = ["diagram-sequence", "diagram-pie", "svg"]` for SVG without graph solvers |
| SVG, Dagre, Cytoscape, and math without ELK | `merman` | `default-features = false, features = ["all-diagrams", "svg", "layout-cytoscape", "layout-dagre", "math"]` |
| SVG, Dagre, Cytoscape, ELK, and math | `merman` | Add `math` to defaults, or select `all-diagrams,svg,layout-cytoscape,layout-dagre,layout-elk,math` with defaults disabled |
| Lint and diagnostics | `merman-analysis` | `all-diagrams`, or the required `diagram-*` selectors; defaults are empty |
| Editor library | `merman-editor-core` or `merman` | `merman` with `all-diagrams, analysis, editor` |
| Standalone LSP server with code actions | `merman-lsp` | `--no-default-features --features all-diagrams,fixes,stdio` |
| Complete diagram CLI | `merman-cli` | Source defaults: all families, Dagre, ELK, Cytoscape, exports, analysis, Markdown and `rustdoc`; `cli-release` additionally selects math explicitly |
| Lean CLI lint | `merman-cli` | `--no-default-features --features all-diagrams,analysis` |
| Checked Rustdoc fragments | `merman-cli rustdoc` | Included in the default and release CLI; documented crates consume committed files through native `include_str!` |
| One-step Rustdoc attributes | `merman-rustdoc` | Default `all-diagrams + svg + layout-cytoscape + layout-dagre + layout-elk`; add `math` for mathematical labels |
| Browser rendering | `@mermanjs/web` or an admitted slim package | Select the npm package, not Cargo features |
| Typst rendering | `@preview/merman` | Select the Typst package; internal WASM profiles are maintainer-only |
| C/C++ embedding | Native C SDK (`sdk-native-full`) or `merman-ffi` | Use the matching full SDK archive for a supported target; `c-abi-native` remains the reference/custom source recipe, with `native-runtime` for explicit native runtime policy |
| Flutter/Dart embedding | `merman` on pub.dev | Use the Flutter package, which consumes ABI 3 internally |
| Android/Kotlin embedding | `merman-android-<tag>.aar` | Use the direct JNI AAR from the matching GitHub Release; main prepares a Maven repository bundle, while public Central rollout remains separate |
| Apple/Swift embedding | `Merman.xcframework` with matching Swift facade | Main prepares a complete local SwiftPM package; published `0.9.3` uses the legacy matching-checkout route, and public remote SwiftPM hosting remains separate |
| Python embedding | `merman` on PyPI | Use the generated UniFFI wheel for the selected platform |

Node/SSG users can select the experimental `@mermanjs/node` alpha package. It installs a small
loader plus one exact-version N-API platform package. Main's next-release recipe selects
deterministic SVG with math, Dagre, ELK, and Cytoscape. The Node recipe omits analysis, ASCII, and binary export.
Published `0.9.2` retains its math-free recipe. `@mermanjs/node-wasm` is an explicit alternative
with the same SVG scope. Browser WASM is not a supported Node transport or fallback. See the
[Node guide](../platforms/node/README.md) for host requirements and publication boundaries.

## Select diagram families

These selectors were introduced in `0.8.0-alpha.7` and are included in the published `0.8.0` release. Packages at alpha.6 and earlier predate this source and typed-API migration. Default facade, CLI, and Rustdoc users retain all built-in families.
Low-level crates keep empty defaults. Existing `default-features = false` consumers must add
`all-diagrams` to retain the previous language surface, or list the families they require.
Output/layout/math leaves do not select diagram families.

### Flowchart and Gantt embedding

For an application checked out beside this repository:

```toml
[dependencies]
merman = { path = "../merman/crates/merman", default-features = false, features = ["svg", "diagram-flowchart", "diagram-gantt", "layout-dagre"] }
```

Use the `Renderer::render(RenderRequest::svg(...))` example below for either Flowchart or Gantt
input. Flowchart uses Dagre as the registered fallback for Mermaid 12’s default ELK request. Gantt
uses its own layout. Add optional engines or `math` as needed. Parser-only applications can instead use the following recipe.

```toml
[dependencies]
merman-core = { path = "../merman/crates/merman-core", default-features = false, features = ["diagram-gantt"] }
```

The shared `merman_model::family::diagram_family_selectors()` catalog maps logical family IDs to the
34 selectors. `all-diagrams` is their union. Aliases, such as Flowchart's `graph` header and
`flowchart-v2` ID, belong to their logical family. `diagram-flowchart` and `diagram-swimlane` remain
independent language selections despite sharing grammar, models, labels, and layout routes.

Family-exclusive public types and `RenderSemanticModel` variants are conditionally compiled.
Consumers importing those types must enable their owner, and reusable consumers should forward
the relevant selectors and gate their family-specific code. Infrastructure variants such as
Error and CustomJson remain available without built-in families.

Cargo unifies features additively. Another dependency can enable additional core parsers, and
`default-features = false` on one edge does not disable defaults selected elsewhere. Inspect the
actual consumer graph with `cargo tree -e features`; an isolated consumer build is the right
place to verify an intended subset. Facade family selectors weak-forward to optional output and
language-tool dependencies, so a language-only selection does not instantiate a renderer.

### Availability and errors

- `supported_diagrams()` and typed implementation lists describe compiled parsers.
- `diagram_family_capabilities()` retains every known family with truthful parser/editor/typed
  implementation flags. Detection and a known header alone do not promise an executable parser.
- SVG and ASCII require their own local family handlers. A dependency may widen core while the
  local renderer remains narrow; consult output planning or ASCII capabilities for that output.
- Actionable editor headers and templates are limited to compiled parsers. Unavailable editor
  facts retain the original parse diagnostic without inventing body symbols.

Strict parsing of a known disabled family returns `UnsupportedDiagram`. Suppressed semantic or
render parsing produces the existing Error diagram; suppressed unknown input returns `None`.
Cancellation is never suppressed, and custom registry overlays keep their existing precedence.
Rendering rejects a missing local handler after cancellation, provenance, and semantic/layout
pair validation, before optional backend planning. The layout plan records the selected backend after family fallback and reports any missing capability ID. A host that denies a compiled selection receives the same typed capability error. Architecture requires Cytoscape by default and `layout-shoal` when selected explicitly. See the [embedding guide](rendering/RUST_EMBEDDING.md#handle-unavailable-languages-and-backends) for family-specific behavior.

Existing distributed products retain their all-family recipes; this change adds no family-specific
prebuilt package. Artifact profile schema 2 records the exact compiled parser set in
`expected.diagram_families` as sorted logical IDs, excluding Error infrastructure. This set does
not certify renderer or ASCII availability. Runtime capability IDs remain in the unchanged
`feature-surface-v1.json` namespace.

See [ADR-0091](adr/0091-selectable-diagram-families.md) for ownership and compatibility decisions.
Native size evidence is tracked separately in the
[controlled measurement report](performance/diagram_selection_native_2026-09-26.md); measurements
must be completed before claiming a reduction.

## Rust examples

### Complete SVG

```toml
[dependencies]
merman = { path = "crates/merman", default-features = false, features = ["all-diagrams", "svg", "layout-cytoscape", "layout-dagre", "math"] }
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

The ordinary `merman = { path = "crates/merman" }` dependency enables
`all-diagrams,svg,layout-cytoscape,layout-dagre,layout-elk`. Add `math` explicitly for mathematical labels.
The default operation remains deterministic; it does not read ambient time, time zone, randomness,
or timing state.

When several SVGs share one HTML document, assign a unique
`SvgRequest.options.diagram_id` to every operation. Use `Renderer` plus typed targets for reusable
configuration, semantic/layout inspection, binary export, presentation, resource policy, or an
explicit SVG pipeline. The complete set of copyable task examples lives in
[`crates/merman/examples`](../crates/merman/examples/README.md).

### SVG without general-purpose layout engines

```toml
[dependencies]
merman = { path = "crates/merman", default-features = false, features = ["svg", "diagram-sequence", "diagram-pie"] }
```

Sequence and Pie have their own layouts. An application can also enable Flowchart parsing with
`diagram-flowchart`. Flowchart rendering requires a separately compiled graph engine, and the SVG
and layout JSON APIs report a typed error at admission when the selected engine is missing.

| Flowchart Cargo features in addition to `svg,diagram-flowchart` | Layout selection |
| --- | --- |
| `layout-dagre` | `layout: dagre`, or the Mermaid 12 fallback for an absent ELK loader |
| `layout-elk` | Mermaid 12’s default, `layout: elk`, or another registered ELK algorithm |
| `layout-shoal` | Set `layout: shoal` in the diagram or site config |
| No graph engine | The default request reports `layout-elk` missing. An explicit `layout: dagre` reports `layout-dagre` missing. |

Add `layout-dagre` to a previous defaults-off recipe that relied on the implicit Dagre backend.
Mindmap's default CoSE layout falls back to Dagre when Cytoscape is absent. An explicit ELK request
tries ELK, CoSE, then Dagre and reports `layout-elk` missing when none of them is compiled.
For a selective `RenderCapabilityPolicy`, explicitly allow `RenderCapability::LayoutDagre` if the
host should admit it. Graph ASCII can independently bring Dugong into the application. To check an
SVG-only dependency closure, use `cargo tree -e normal` in the final consumer build with `ascii`
disabled. [Configure an application layout default](rendering/RUST_EMBEDDING.md#set-an-application-layout-default)

### Analysis and editor support

```toml
[dependencies]
merman-analysis = { path = "crates/merman-analysis", default-features = false, features = ["all-diagrams"] }
merman = {
    path = "crates/merman",
    default-features = false,
    features = ["all-diagrams", "analysis", "editor"],
}
```

On the `merman` facade, enabling the `editor` Cargo feature also enables `analysis`. The canonical
`editor` capability has no global implication, so another package must declare the pair explicitly
when its workflow exposes both. Select families separately; neither capability enables a built-in parser by itself.

Fix generation is a separate positive `fixes` feature and defaults to `FixGeneration::Omit` even when compiled. Select `FixGeneration::Generate` on the requested analysis projection, or `AnalysisOptions::with_fix_generation` on a convenience analyzer. The full CLI, LSP, and editor/native SDK recipes select fixes; diagnostic-only analysis, CLI analysis, Web analysis, and Typst recipes omit it. LSP diagnostic publication remains Omit, while code actions request Generate from the same retained generation. Hosts must declare the capability independently of Cargo feature union. See the [migration example](migrations/model-source-boundaries.md#request-fixes-explicitly).

### Explicit system adapters

Compile only the adapters required by the host:

```toml
[dependencies]
merman = {
    path = "crates/merman",
    default-features = false,
    features = [
        "all-diagrams",
        "svg",
        "layout-cytoscape",
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
    features = ["all-diagrams", "svg", "native-runtime"],
}
```

The binding remains deterministic until `{"runtime_policy":"native"}` is selected. Its runtime
catalog reports `system-clock`, `system-timezone`, and `system-random`, not `native-runtime`, because
those concrete IDs describe callable adapters while the Cargo aggregate describes how the artifact
is assembled.

### Independent exports

PNG and PDF export with system fonts can use this selection:

```toml
[dependencies]
merman = {
    path = "crates/merman",
    default-features = false,
    features = ["all-diagrams", "svg", "png", "pdf", "system-fonts"],
}
```

`png`, `jpeg`, and `pdf` are separate capabilities. They are not silently included in the
SVG recipe and should not be added to every native SDK without a product reason.

## CLI

`merman-cli` is the browserless Mermaid CLI replacement. Its normal default includes all diagram families,
SVG, analysis, ASCII, PNG, JPEG, PDF, Cytoscape and ELK layouts, explicit native fonts, local Iconify loading,
Markdown conversion, native adapters, network icons, parallel Markdown, and shell completions. `rustdoc` is included in source defaults and the standard archive.
The `cli-release` artifact selects those capabilities plus `math` explicitly. Source defaults omit math;
use `cargo install merman-cli --locked --features math` when building a math-capable CLI from current source. Both closures include the
EPL-2.0 ELK implementation and its notices. Alpha.6 source defaults excluded ELK while its release archives included it; alpha.7, 0.8.0, and 0.9.x include ELK in both recipes.
Compiled native adapters never change the default runtime policy:

```sh
cargo install merman-cli --version 0.10.0 --locked
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
cargo run -p merman-cli --no-default-features --features all-diagrams,analysis -- lint diagram.mmd
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
Use `--no-default-features --features all-diagrams,markdown,layout-dagre` for sequential Markdown
rendering with Dagre. Add `layout-cytoscape` for Architecture's default layout. Select
`parallel-markdown` to enable Rayon.

## Rustdoc

Rustdoc has two independently distributed static-SVG workflows. Source defaults and release
archives retain the deterministic CLI `build/check` authoring workflow. The `rustdoc` leaf
enables SVG rendering with Cytoscape; mathematical labels require explicit `math`;
it does not make `merman-rustdoc` a dependency. Generated Markdown and its portable receipt are
committed, packaged, and consumed with Rust's standard `#[doc = include_str!(...)]` or
`#![doc = include_str!(...)]`. This gives the documented crate zero attributable Merman packages
in its normal/build Cargo graph, supports crate-level docs, and makes diagram updates reviewable.

The `merman-rustdoc` package remains the independent one-step attribute workflow. Its default
features compile all diagram families, SVG, Cytoscape, and ELK into the proc-macro host. Math is
optional: add `math` for mathematical labels. The facade also defaults to direct SVG and layout
leaves without math. Both packages retain the math-free convenience aggregates described above.

The macro default includes the EPL-2.0 ELK closure. Optional dependency gating keeps the renderer
out of ordinary builds while the dependency is disabled. To exclude ELK from documentation builds,
use `default-features = false` and select the required diagram families plus `svg` and the needed
layout leaves. Cargo features are additive: explicit leaves do not remove features enabled by
other dependencies.

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
| `@mermanjs/web` | SVG with math, analysis, editor, ASCII, Cytoscape and ELK | Complete browser SDK |
| `@mermanjs/web-render` | SVG with math, Cytoscape and ELK | Complete SVG package |
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

Main's Android, Apple, Python, and Flutter native products select
`all-diagrams,analysis,ascii,fixes,layout-cytoscape,layout-dagre,layout-elk,math,svg` in the artifact descriptor.
They include mathematical SVG, Dagre, ELK, Cytoscape, ASCII, analysis, fixes, validation, and
document analysis. They omit PNG, JPEG, PDF, ambient native runtime adapters, and the C/WASI
memory-font service. Math availability does not override constrained-policy rejection of opaque
math rendering.
Use an appropriate interactive/trusted policy for admitted formulas and retain host execution and
memory limits. Their package-specific math and ELK notices and source provenance remain part of
the artifact contract. Generated APIs keep optional operation names for custom current-contract
libraries; inspect the runtime catalog and handle typed missing-capability errors before exposing
optional output choices.

These expanded recipes target the `0.10.0` candidate. Published Android, Python, Flutter, and
legacy Apple `0.9.3` artifacts remain math-free; published Web full/render `0.9.3` includes math.
Current Rust source defaults and convenience aggregates still require explicit `math`. See
[prebuilt products](release/PREBUILT_PRODUCTS.md) for the per-channel installation boundaries.

Flutter uses `flutter pub add 'merman:0.10.0'` and `Merman.open()`; see the [Flutter guide](../platforms/flutter/README.md). Android resolves the matching Maven repository handoff or uses its AAR directly; its Kotlin surface is direct JNI
transport API 2 rather than C ABI 3. The Maven publication bundle does not
establish public Central availability. Apple delivery includes a complete local SwiftPM
package with matching sources, resources, and XCFramework; public remote SwiftPM installation
awaits a hosted package repository. C and C++ can use the published `0.9.3` native SDK, with
`sdk-native-full` recommended for SVG, PNG, JPEG, PDF, ASCII, math, Dagre, Cytoscape, ELK, and
caller-owned fonts on supported desktop targets. `c-abi-native` remains the complete source
reference build, not the downloadable SDK. The [SDK](../distribution/sdk/README.md),
[Flutter](../platforms/flutter/README.md),
[Android](../platforms/android/README.md), [Apple](../platforms/apple/README.md), and
[C ABI](../crates/merman-ffi/README.md) guides provide each transport's copyable first operation
and lifecycle rules; keep each transport's headers, facade, and binaries paired.

Typst is independently versioned. Use the `0.6.0` wrapper with Typst `0.15.0` or later:

````typst
#import "@preview/merman:0.6.0": mermaid

#mermaid(```mermaid
flowchart TD
  Source --> Document
```)
````

The source build of wrapper `0.7.0` bundles Merman `0.11.0` with Mermaid `12.3.0` compatibility,
SVG, analysis, Cytoscape, Dagre, ELK, and Theme v2 preset selection and authoring. It uses deterministic
text measurement; Typst's actual glyph metrics are not measured automatically, and math
rendering is unavailable. The package ships ELK's EPL-2.0 notices and defaults to
14,100,000 layout work units while retaining the other constrained ceilings. Explicit finite
layout limits may lower or raise that allowance.
Use the registry package for normal installs and local `--package-path` builds for source testing.
See the [Typst package guide](../distribution/typst/merman/README.md) for examples and its
[version mapping](../distribution/typst/merman/README.md#version-mapping) for bundled Merman versions.

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
| `full`, `core-full`, `tiny`, registry profiles | Low-level crates are default-empty; choose family selectors and observable capability leaves |
| `render` | `svg` |
| `raster` | One or more of `png`, `jpeg`, `pdf` |
| `cytoscape-layout` | `layout-cytoscape` |
| `elk-layout` | `layout-elk` |
| `ratex-math` | `math` |
| `host-*`, `core-host` | `system-clock`, `system-timezone`, `system-random`, `system-timing` |
| `preset-*`, `*-no-elk` | Direct positive leaf features; use an artifact profile for exact recipes |

Add logical family selectors through the parser-owned family catalog and existing forwarding
rules. Keep them separate from runtime capability IDs. A capability leaf is justified by a
user-visible API, output, reusable engine, host adapter, or compiled CLI tool with a meaningful
closure boundary.
