# Rustdoc Design and Dependency Budget

Date: 2026-09-10. Status: dependency review; the default-feature change is accepted in
[ADR-0088](../adr/0088-lean-rustdoc-default-features.md) for the next release after `0.8.0-alpha.6`.

Snapshot: working tree on `refactor/rustdoc-document-pipeline`, based on `e6657b382`, including the
shared `merman-doc` refactor, before changing the macro defaults. Cargo.lock SHA-256:
`ae7cf8b44c86d3e3383eaddaea065d1feb430234734301e346163720130cf943`.

## Assessment

The current separation is sound for build-time static Mermaid diagrams: `merman-doc` owns Markdown
discovery and HTML embedding, the renderer owns SVG semantics and embedding safety, and the CLI
and procedural macro own their respective filesystem and Rust expansion behavior. The shared
library prevents fixes from diverging between integrations. It does not remove native renderer
compilation from the macro's consumer.

The native macro is suitable when keeping diagrams next to Rust items and running one `cargo doc`
command matters more than compilation cost. It should not be presented as a lightweight default
for every library. The existing CLI-generated fragment path is the appropriate interface for
users requiring no renderer dependency in their documentation build.

## Measured dependency graphs

These measurements preserve the before-change snapshot. The accepted default is
`svg + layout-cytoscape`, corresponding to the 117-package row below; `complete-svg` remains an
explicit 173-package selection in this snapshot. The published alpha.6 default is unchanged.

After implementation on 2026-09-10, the isolated consumer was switched to the macro's actual
default features and rechecked with the same locked Windows normal/build graph. It contains 117
packages excluding the consumer, retains `manatee`, and contains no RaTeX, `fontdb`, `system-fonts`,
or ELK packages. This verifies the dependency change without claiming a performance improvement.

The measurements use an isolated consumer, the current lockfile, Windows host
`x86_64-pc-windows-msvc`, and normal plus build dependency edges. Counts deduplicate package name
and version plus source, include `merman-rustdoc` itself, and exclude the synthetic consumer and dev
dependencies. They are package counts, not compilation units, cold-build timings, memory use,
archive size, or runtime binary size. No new cold-build benchmark was performed.

| Selected macro features | Unique packages | Registry packages | Usable renderer |
| --- | ---: | ---: | --- |
| None, defaults disabled | 1 | 0 | No; invocation reports the missing `svg` feature |
| `svg`, defaults disabled | 116 | 108 | Base SVG |
| `svg,layout-cytoscape`, defaults disabled | 117 | 108 | Base SVG and Cytoscape |
| `svg,math`, defaults disabled | 172 | 164 | Base SVG and math |
| Former default `complete-svg` | 173 | 164 | Base SVG, Cytoscape, and math |
| `complete-svg-elk` | 175 | 164 | Complete SVG plus ELK |

Selecting only `svg` removes 57 packages, about 33% of the former default graph. Math accounts for 56 of
those packages and Cytoscape for one workspace crate. Conversely, adding ELK adds only two
workspace crates; this does not imply a small source or compilation cost.

The math closure includes `ratex-svg/embed-fonts`, which in RaTeX 0.1.14 also enables its standalone
font path: `ratex-font-loader`, `ratex-unicode-font`, `fontdb`, `system-fonts`, and `ab_glyph`.
Embedded KaTeX fonts add `rust-embed` and its hashing/file-walking dependencies. Merman already
disables RaTeX default features; this is the selected capability's actual dependency graph, not
an accidentally enabled default. Separating embedded fonts from ambient font discovery is a
concrete upstream integration question to investigate.

The base SVG graph still includes URL/IDNA/ICU handling and `lol_html` HTML sanitization.
Their transitive graphs overlap; package counts are not additive deletion savings. These support
actual URL normalization and Mermaid label semantics rather than unrelated application features.

The former default renderer graph contains 165 packages. The macro adds the facade, `merman-doc`, the
macro itself, and five registry packages for `proc-macro-crate` and its manifest parsing chain.
`merman-doc` adds no registry package to this graph: `pulldown-cmark`, `quick-xml`, and `serde_json`
are already used by the renderer/core. Removing the facade would save one workspace crate and no
registry packages, while requiring the macro to take over higher-level render orchestration.
Neither shared-library extraction nor facade removal solves the central dependency concern.

With the consumer's optional documentation feature disabled, its active tree contains only the
consumer: zero Merman dependencies are compiled. The lockfile still contains the optional macro,
math, and layout packages. Cargo resolves optional features for lockfile consistency separately
from selecting the active compilation graph. [Cargo resolver documentation](https://doc.rust-lang.org/cargo/reference/resolver.html#features)

To reproduce the package counting, create a standalone consumer outside the workspace with this
manifest and an empty `src/lib.rs`. Seed its lockfile from this snapshot and let an initial offline
Cargo invocation register the consumer. The measured consumer retained the same dependency
versions and sources as the repository lockfile.

```toml
[package]
name = "rustdoc-dependency-probe"
version = "0.0.0"
edition = "2024"

[workspace]
resolver = "3"

[features]
default = ["merman-rustdoc/complete-svg"]
svg = ["merman-rustdoc/svg"]
elk = ["merman-rustdoc/complete-svg-elk"]
dependency = ["dep:merman-rustdoc"]

[dependencies]
merman-rustdoc = { path = "E:/Rust/merman/crates/merman-rustdoc", default-features = false, optional = true }
```

Count unique rows, removing Cargo's repeated-subtree markers:

```powershell
$tree = cargo tree --manifest-path <consumer>/Cargo.toml --offline --locked --target x86_64-pc-windows-msvc --edges normal,build --prefix none --format '{p}'
$packages = $tree | ForEach-Object { $_ -replace ' \(\*\)$', '' } | Sort-Object -Unique
$packages.Count - 1
```

For other rows, append `--no-default-features` and `--features svg`, `merman-rustdoc/math`,
`merman-rustdoc/layout-cytoscape`, `elk`, or `dependency` as appropriate. Omitting `--features`
with defaults disabled measures the optional dependency being off. Windows measurements do not
establish a Linux/docs.rs host count; changing `--target` alone does not move host-built macros
and build dependencies to another host.

Do not compare these numbers directly with the older August report: its source, lockfile, default
ELK membership, and measurement conditions differ.

## User-visible costs

| Integration | Ordinary build | Documentation build | Authoring cost |
| --- | --- | --- | --- |
| Non-optional macro with `cfg_attr(doc, ...)` | Compiles selected renderer | Compiles and executes renderer | Inline diagrams |
| Optional macro, documentation feature disabled | Does not compile renderer | Leaves diagram source unchanged | Explicit feature selection |
| Optional macro, feature enabled | Compiles renderer even if the macro does not expand | Renders diagrams | One `cargo doc` command |
| CLI-generated, committed fragments | No renderer dependency | Reads packaged fragments | Generate, review, and check freshness |

`--all-features` enables an optional documentation feature too. A source-level `cfg(doc)` cannot
make an already-selected Cargo dependency lazy. Host procedural-macro dependencies also should
not be described as renderer code linked into the consumer's runtime binary.
[Cargo features](https://doc.rust-lang.org/cargo/reference/features.html),
[Rust procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html)

The README and public API installation examples previously disagreed: the README recommended an
optional dependency, while the API page led with a normal dependency and used only `cfg_attr(doc)`.
This review aligns both with `doc-diagrams` and `cfg_attr(all(doc, feature = "doc-diagrams"), ...)`,
and explains the `--all-features` and lockfile behavior. The subsequent default change keeps this
optional dependency guidance and makes math explicit.

Both current integrations fit docs.rs's offline, largely read-only build environment: the macro
returns documentation strings, and the CLI path consumes pre-generated files included in the
package. Neither requires docs.rs to install or launch the CLI. Macro builds still consume the
service's finite resources; compatibility is not a guarantee that every workload fits them.
[docs.rs build environment](https://docs.rs/about/builds)

Inline Mermaid fences require no build script. External `include_mmd!` inputs require the
documented consumer `build.rs` directory watch for reliable incremental rebuilding on stable Rust.
This is real integration overhead, but the standard Cargo mechanism is preferable to a custom
cache, extra build-helper crate, or simulated compiler dependency tracking.
[Rust tracked-path API](https://doc.rust-lang.org/proc_macro/tracked/fn.path.html),
[Cargo rebuild tracking](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed)

## Accepted scope and remaining design questions

1. **Keep the two existing integration paths and make cost visible at entry.** Published libraries
   with strict budgets should use CLI fragments. Inline item docs can use the optional macro.
   The documentation inconsistency found during this review is fixed in this working tree.
2. **Make math explicit while retaining Cytoscape in the macro default.** This is the accepted
   change in ADR-0088: `svg + layout-cytoscape` replaces the default `complete-svg` selection,
   while both complete aggregates retain their meanings. Keeping Cytoscape preserves the former
   default layout capability; this review does not establish a reason to remove it. Migration
   notes, feature contracts, actionable missing-capability errors, and representative diagram
   checks accompany the change. The snapshot's corresponding 117-package graph remains substantial.
3. **Keep font dependency separation as a future integration question.** The RaTeX and font-loading
   feature graph may contain capabilities unnecessary to deterministic documentation output.
   Retain actual math behavior and pinned font semantics; do not replace working parsers,
   sanitizers, or URL handling with local approximations merely to lower a package count.
4. **Leave repeated rendering unchanged in this scope.** The macro currently constructs
   a renderer and runs the full render path separately for light and dark variants. Two complete
   SVGs are emitted. Source-level fixed themes can make these variants equivalent except for IDs.
   Context reuse or effective-theme-aware deduplication would need separate evidence of benefit
   and correctness. They do not reduce the dependency closure. Do not infer themes with string
   matching or assume that changing CSS preserves theme-sensitive layout.
5. **Keep the macro's scope bounded.** Deferred rendering after `cfg`, explicit tree traversal, and
   local option overrides serve real users. Avoid growing external-module loading, Rust name
   resolution, or evaluation of arbitrary `doc` expressions. Crate-level and external Markdown
   documents already have the standard `include_str!` path through CLI generation.

The accepted implementation deliberately excludes a performance benchmark suite and repeated-render
optimization. Existing dependency-closure and behavior checks validate the feature change. This
review makes no claim that the measured package reduction yields a proportional speedup.

## Alternatives and existing evidence

Aquamarine injects Mermaid JavaScript for rendering in the reader's browser. Its lighter Rust
dependency surface reflects a different execution location. It is not evidence that a native
renderer can provide the same build-time SVG contract at that dependency cost.
[Aquamarine implementation](https://github.com/mersinvald/aquamarine/blob/master/src/lib.rs),
[manifest](https://github.com/mersinvald/aquamarine/blob/master/Cargo.toml)

An embedded WASM renderer has already been investigated in this repository. The historical U7
experiment reduced the host graph to 19 packages but measured warm rendering at 28.101 times the
native oracle, failing its predeclared maximum of 2 times. These are historical experiment
results, not measurements rerun for this review. Reopening that direction needs new evidence;
adding a native fallback or downloading a renderer during compilation does not address the
existing product contract. See the [U7 report](rustdoc-mermaid-ecosystem-2026-08-14.md#u7-bounded-full-capability-wasm-spike).

The accepted feature-default change is tracked separately in ADR-0088. It changes neither the
published alpha.6 artifact nor the facade default, and introduces no release version or backend.
