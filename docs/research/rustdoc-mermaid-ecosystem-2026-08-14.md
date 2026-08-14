# Rustdoc Mermaid Ecosystem and Merman Integration Architecture

**Researched:** 2026-08-14

**Merman snapshot:** `24b0be325c5b30483dc42ef20b37b7c2bf1f22bb`

**Registry snapshot:** 2026-08-14T04:32:39Z

**Decision (updated by U7):** ship and retain two explicit Rustdoc paths: CLI pre-generation with
checked static fragments, and the native `merman-rustdoc` macro for users who explicitly choose
one-step in-process rendering. The bounded full-capability WASM spike cleared closure, package-size,
parity, sandbox, reproducibility, and offline-package gates, but its warm render median was
`28.101x` the native oracle, against the frozen `<=2x` gate. WASM is therefore future-only, not a
current backend, feature, or fallback. Browser-side Mermaid.js remains outside the primary product.

## Executive decision

The reported build-cost problem is real and architectural. `merman-rustdoc` is a procedural macro
whose default feature set links the complete Merman renderer into a host-side compiler plugin. A
consumer-side `#[cfg_attr(doc, ...)]` prevents the attribute from expanding during an ordinary Rust
compilation, but it does not remove a dependency that Cargo has already selected. Once the optional
dependency feature is enabled, Cargo must build the selected procedural macro and its renderer
closure even if no invocation survives conditional compilation.

At this repository snapshot, the locked closure contains:

| `merman-rustdoc` configuration | Unique normal packages | Normal + build packages | Usable macro |
| --- | ---: | ---: | --- |
| `--no-default-features` | 1 | 1 | No; the macro emits a feature error |
| `--no-default-features --features svg` | 112 | 116 | Yes |
| Default `complete-svg` | 168 | 173 | Yes |

These are `cargo tree --locked` package/version counts after removing Cargo's repeated `(*)`
markers. They are dependency-closure proxies, not clean-build time, peak-memory, or disk-size
measurements. Even so, reducing 168 normal packages to 112 does not turn an in-process renderer
into a lightweight documentation annotation.

The current product is two explicit paths:

1. **CLI pre-generation (recommended cheap path):** `merman-cli rustdoc build/check` renders
   declared inputs to checked static fragments and receipts before `cargo doc`. Ordinary consumer
   builds and Rustdoc do not compile or execute the renderer.
2. **Native macro (explicit one-step path):** retain `merman-rustdoc` for users who deliberately
   accept its host renderer closure in exchange for in-process fence and attribute expansion.

The embedded WASM host/guest remains a possible future internal implementation of the second path,
not a third user-visible mode. U7 did not justify shipping it: the measured interpreter path was
far outside the frozen latency budget even though the other gates passed.

An optional explicit HTML postprocessor can support inline fences for local and CI-hosted
documentation without putting Merman in the consuming crate's graph. It cannot be the sole docs.rs
solution because docs.rs metadata cannot run a Cargo subcommand.

This is a fearless refactor of the integration boundary, not a retreat to a weaker renderer. The
current crate has a defensible and uncommon advantage: it produces deterministic, sanitized,
build-time SVG without Node.js, a browser, runtime JavaScript, or a network fetch. The CLI path
preserves that contract without imposing the renderer on documentation builds, while the native
macro keeps the one-step experience explicit. A future WASM attempt must clear the same frozen
measurement gates before it can replace either shipped path.

## Scope and evidence boundary

This report compares Rustdoc integrations, adjacent mdBook integrations, Cargo/Rustdoc-supported
injection mechanisms, and native build-time rendering. It uses only first-party sources:

- immutable upstream repository source and release commits;
- crates.io and docs.rs metadata;
- the Cargo Book, Rust Reference, Rustdoc Book, and docs.rs operator documentation; and
- this repository at the revision above.

The crates.io counters below are registry counters, not unique users, active installations, or
endorsements. `recent_downloads` is reported exactly as the API field; this report does not infer a
time window that the cited response does not define.

No public crate or repository named `mermaid-rs-doc` could be verified. The exact
[crates.io API endpoint](https://crates.io/api/v1/crates/mermaid-rs-doc) returned `404`, and the
[GitHub repository search](https://api.github.com/search/repositories?q=%22mermaid-rs-doc%22+in%3Aname%2Cdescription%2Creadme&per_page=30)
returned no repository. The name may be a conflation of `simple-mermaid`,
`mermaid-rs-renderer`, or a private/unpublished project. This report analyzes the two public
projects separately and does not invent a contract for the missing name.

The public registry search found three direct Rustdoc-oriented Mermaid crates with verifiable
source: `aquamarine`, `simple-mermaid`, and `merman-rustdoc`. The
[`rustdoc-mermaid` repository](https://github.com/Notgnoshi/rustdoc-mermaid/tree/ed0461bdb4d414a6e582edf32be12dd9dad708d5)
is an HTML-injection recipe, not a published crate.

## Current Merman Rustdoc architecture

### Call path

The current path is entirely in the compiler process:

```text
consumer Cargo feature selects merman-rustdoc
  -> Cargo builds a host proc-macro dylib
  -> proc macro parses the annotated Rust item with syn
  -> doc comments are scanned for Mermaid fences/include_mmd!
  -> Merman HeadlessRenderer parses, lays out, and renders SVG synchronously
  -> default rustdoc theme renders both light and dark SVG variants
  -> strict SVG validation rejects active or remote content
  -> generated #[doc = "..."] HTML embeds the SVG into rustdoc output
```

The package manifest makes `complete-svg` the default and maps it to `svg`, Cytoscape layout, ELK
layout, and math ([manifest](../../crates/merman-rustdoc/Cargo.toml)). The renderer constructs a
deterministic `RenderEnvironment`, selects parity/readable/resvg-safe output, and renders two SVGs
for the default Rustdoc theme ([renderer](../../crates/merman-rustdoc/src/render.rs)). The source
rewriter and renderer are already separated behind `MermaidRenderer` and `IncludeResolver` traits,
which is a useful testing boundary, but both implementations still ship in one proc-macro artifact.

The integration has more product behavior than a fence substitution:

- item and recursive inline-tree scopes ([expansion](../../crates/merman-rustdoc/src/expand.rs));
- backtick and tilde fences, multiple diagrams, and `include_mmd!`;
- error or source-preserving failure policy with document-line context
  ([document rewrite](../../crates/merman-rustdoc/src/doc.rs));
- readable, parity, and resvg-compatible pipelines;
- fixed Mermaid themes, source-controlled theme config, or static Rustdoc light/dark variants;
- optional collapsed source disclosure; and
- strict validation against scripts, event attributes, unsafe links, and remote resources
  ([SVG validation](../../crates/merman-rustdoc/src/svg.rs)).

The repository targets Mermaid `11.16.1` and describes source-backed support for 35 diagram
families ([README](../../README.md), [changelog](../../CHANGELOG.md)). This matters: Merman is not
merely a Rust wrapper around a browser runtime.

### Why `cfg_attr(doc, ...)` does not control Cargo resolution

Three different mechanisms are easy to conflate:

1. **Cargo feature/dependency selection.** An optional dependency is absent only while the feature
   that names it remains disabled. The Cargo Book states that enabling the feature includes the
   dependency, and that default features are automatically enabled unless every dependency edge
   opts out ([optional dependencies](https://doc.rust-lang.org/cargo/reference/features.html#optional-dependencies),
   [default features](https://doc.rust-lang.org/cargo/reference/features.html#the-default-feature)).
2. **Rust conditional compilation.** Rustdoc sets `cfg(doc)` while it builds documentation; this
   controls which Rust items or attributes survive compilation
   ([Rustdoc advanced features](https://doc.rust-lang.org/rustdoc/advanced-features.html#cfgdoc-documenting-platform-specific-or-feature-specific-information)).
   It does not retroactively alter Cargo's resolved dependency graph.
3. **Feature unification.** Cargo uses the union of enabled features for a package. Resolver v2
   avoids some unwanted unification between normal, build, and proc-macro contexts, but it does not
   make an enabled proc-macro dependency lazy
   ([feature unification](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification),
   [resolver v2](https://doc.rust-lang.org/cargo/reference/features.html#feature-resolver-version-2)).

Procedural macros execute during compilation and have compiler-process file access, with the same
class of security concerns as build scripts
([Rust Reference](https://doc.rust-lang.org/reference/procedural-macros.html)). Cargo treats proc
macros as host artifacts; moving their renderer behind another ordinary library crate does not
change that host build
([Cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags)).

The following consumer configurations therefore behave differently:

| Consumer setup | Ordinary `cargo build` | `cargo doc` | `--all-features` |
| --- | --- | --- | --- |
| Non-optional `merman-rustdoc`; only invocation uses `cfg_attr(doc, ...)` | Builds renderer | Builds and runs renderer | Builds renderer |
| Optional dependency behind `doc-diagrams`, feature disabled | Does not build it | Does not render | Enables and builds renderer |
| Optional dependency; docs.rs metadata enables `doc-diagrams` | Does not build it locally by default | Builds when explicitly enabled | Builds renderer |
| Documentation feature included in the consumer's default features | Builds renderer | Builds renderer | Builds renderer |

Cargo explicitly documents `--all-features` as activating every feature of selected packages
([Cargo features](https://doc.rust-lang.org/cargo/reference/features.html#command-line-feature-options)).
Many CI matrices run this command, so a feature named "docs only" is still part of the cost whenever
the matrix deliberately activates it.

Placing the dependency under `[target.'cfg(doc)'.dependencies]` is not a sound escape hatch. Cargo
documents that dependency tables cannot use compilation-mode cfgs such as `feature`, `test`,
`debug_assertions`, or `proc_macro` as source-level conditional compilation does, and that there is
no general dependency-selection mechanism for those modes
([platform-specific dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#platform-specific-dependencies)).
`cfg(doc)` is set by Rustdoc for the final compilation, after dependency resolution.

### What the existing feature split does and does not solve

The current leaves are useful capability boundaries:

- `svg` excludes the optional Cytoscape, ELK, and math additions;
- `layout-cytoscape`, `layout-elk`, and `math` communicate expensive optional behavior; and
- disabling every feature proves that all implementation dependencies are optional.

They do not solve the base problem because `svg` still links the broad parser, semantic model,
layout, text, and SVG renderer into the proc macro. Adding dozens of diagram-family Cargo features
would create a large additive-feature matrix and still leave every selected renderer inside the
consumer build. The Cargo Book warns that feature combinations multiply the configurations that
need testing
([feature combinations](https://doc.rust-lang.org/cargo/reference/features.html#feature-combinations)).
Merman should not turn its manifest into a second diagram registry merely to preserve the wrong
integration boundary.

## Ecosystem comparison

### Snapshot

| Project | Latest verified release | Registry total / recent | Integration | Render phase | Client JS/network | Consumer Rust closure |
| --- | --- | ---: | --- | --- | --- | --- |
| `aquamarine` | 0.6.0, 2024-10-08 | 19,081,372 / 3,795,490 | Attribute proc macro | Browser | Bundled local ESM with unpkg fallback | Small proc-macro closure |
| `simple-mermaid` | 0.2.0, 2024-12-04 | 7,251,631 / 1,101,908 | Zero-dependency `macro_rules!` | Browser | jsDelivr, floating Mermaid 11 major | One tiny crate |
| `merman-rustdoc` | 0.8.0-alpha.5, 2026-08-09 | 242 / 242 | Attribute proc macro | Compile time | None | 112 normal packages minimum; 168 default |
| `mdbook-mermaid` | 0.17.0, 2025-11-18 | 539,904 / 70,008 | External mdBook preprocessor | Browser | Vendored JavaScript copied into book | Tool installed outside book crate graph |
| `mdbook-mermaid-mmdr` | 0.1.3, 2026-07-01 | 433 / 228 | External mdBook preprocessor | Build time | None | Native renderer belongs to installed tool |

Registry records:
[`aquamarine`](https://crates.io/api/v1/crates/aquamarine),
[`simple-mermaid`](https://crates.io/api/v1/crates/simple-mermaid),
[`merman-rustdoc`](https://crates.io/api/v1/crates/merman-rustdoc),
[`mdbook-mermaid`](https://crates.io/api/v1/crates/mdbook-mermaid), and
[`mdbook-mermaid-mmdr`](https://crates.io/api/v1/crates/mdbook-mermaid-mmdr).

The dates and counters establish project age and registry activity only. They do not establish
correctness, compatibility, or retention.

### Aquamarine 0.6.0

Aquamarine is the closest direct UX comparator. It is a procedural attribute macro that rewrites
Mermaid fences and supports an `include_mmd!` marker. Its published manifest has six lightweight
normal dependencies: `quote`, `proc-macro2`, `proc-macro-error2`, `itertools`, `syn`, and
`include_dir`
([Cargo.toml at v0.6.0](https://github.com/mersinvald/aquamarine/blob/42d1267c53da9039e48009005e958eb1dce7d58b/Cargo.toml),
[docs.rs dependency list](https://docs.rs/aquamarine/0.6.0/aquamarine/)).

It does not render SVG in Rust. The macro embeds a Mermaid module directory in its own artifact,
attempts to extract that directory to `target/doc/static.files.mermaid`, inserts a module script in
each transformed document block, imports the local module at page load, and falls back to
`https://unpkg.com/mermaid@11.1/...` if local import fails
([implementation](https://github.com/mersinvald/aquamarine/blob/42d1267c53da9039e48009005e958eb1dce7d58b/src/attrs.rs)).
The browser chooses a theme and calls `mermaid.run()`.

The 0.6.0 crates.io archive is 2,334,879 bytes because it packages the Mermaid module assets even
though its Rust dependency closure is small
([registry version metadata](https://crates.io/api/v1/crates/aquamarine)). Its latest release and
latest source commit are both the 2024-10-08 v0.6.0 commit
([release commit](https://github.com/mersinvald/aquamarine/commit/42d1267c53da9039e48009005e958eb1dce7d58b)).

Aquamarine's trade is coherent: compile a small source transformer, defer parsing/layout/rendering
to the reader's browser, and accept JavaScript/module-loading behavior. It is not evidence that a
native build-time renderer can have the same dependency closure.

### `simple-mermaid` 0.2.0

`simple-mermaid` makes the opposite ergonomic choice: diagrams must live in external files and are
included through a declarative macro used inside `#[doc = ...]`. Its manifest has no dependencies,
and the library is `no_std`
([manifest](https://github.com/glueball/simple-mermaid/blob/51cb8d02e717718541682ed202b1213c90ef4b54/Cargo.toml),
[source](https://github.com/glueball/simple-mermaid/blob/51cb8d02e717718541682ed202b1213c90ef4b54/src/lib.rs)).
The crates.io archive is only 5,438 bytes
([registry metadata](https://crates.io/api/v1/crates/simple-mermaid)).

The emitted HTML imports `https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs` in the
reader's browser. The major version is fixed but the minor and patch are not. This provides tiny
Rust builds at the cost of online executable content, runtime rendering, and output that can change
without publishing a new Rust crate. Its last source and release commit is 2024-12-04
([v0.2.0 commit](https://github.com/glueball/simple-mermaid/commit/51cb8d02e717718541682ed202b1213c90ef4b54)).

The reusable lesson is not the CDN. It is that external diagram files plus a declarative
`#[doc = ...]` macro can make the Rust integration essentially free. Merman can apply that shape to
pre-rendered, provenance-checked SVG rather than remote JavaScript.

### Rustdoc HTML injection

Rustdoc officially supports files inserted into the HTML `<head>`, before content, and after
content through `--html-in-header`, `--html-before-content`, and `--html-after-content`
([Rustdoc command-line arguments](https://doc.rust-lang.org/rustdoc/command-line-arguments.html#--html-in-header-include-more-html-in-head)).
`cargo rustdoc -- ...` passes extra arguments only to the final Rustdoc invocation and does not
document dependencies, while `RUSTDOCFLAGS` or Cargo configuration passes flags to every Rustdoc
process
([cargo rustdoc](https://doc.rust-lang.org/cargo/commands/cargo-rustdoc.html)).

The `rustdoc-mermaid` reference repository demonstrates the thin recipe: inject Mermaid.js in the
header, then run it over Rustdoc's Mermaid code-block selector after content. Its README also records
the practical workspace/path issue caused by applying relative fragment paths to multiple Rustdoc
invocations
([immutable README](https://github.com/Notgnoshi/rustdoc-mermaid/blob/ed0461bdb4d414a6e582edf32be12dd9dad708d5/README.md)).
The repository has two commits and no published crate, so it is architectural evidence rather than
an adopted package.

This route has no Rust dependency at all and naturally handles Markdown loaded with `include_str!`,
which item-level procedural macros cannot inspect. Its weaknesses are invocation setup, reliance on
Rustdoc HTML selectors that are not a documented stable API, runtime JavaScript, and asset delivery.

docs.rs supports additional `rustdoc-args`, but its `cargo-args` metadata accepts options only, not a
subcommand
([docs.rs metadata](https://docs.rs/about/metadata)). docs.rs also states that inline JavaScript is
allowed but not guaranteed to keep working
([docs.rs JavaScript policy](https://docs.rs/about#javascript)). HTML injection can therefore be an
explicit low-cost web mode, but it should not replace Merman's static contract.

### `mdbook-mermaid` 0.17.0

`mdbook-mermaid` is not a Rustdoc plugin, but it demonstrates a healthier tool boundary. mdBook
starts an external preprocessor executable. The preprocessor rewrites fenced Mermaid Markdown into
`<pre class="mermaid">`, while its `install` command copies `mermaid.min.js` and an initialization
script into the book and registers them as `additional-js`
([preprocessor source](https://github.com/badboy/mdbook-mermaid/blob/18966ac0c6706b863cd0c68d478e043e26001170/src/lib.rs),
[installer source](https://github.com/badboy/mdbook-mermaid/blob/18966ac0c6706b863cd0c68d478e043e26001170/src/bin/mdbook-mermaid.rs)).

The vendored Mermaid file is 2,667,011 bytes at v0.17.0
([immutable asset metadata](https://api.github.com/repos/badboy/mdbook-mermaid/contents/src/bin/assets/mermaid.min.js?ref=18966ac0c6706b863cd0c68d478e043e26001170)).
The asset was updated to Mermaid 11.6.0 in the 2025-03-28 source commit
([upgrade commit](https://github.com/badboy/mdbook-mermaid/commit/d34f2691978ba19815b2385f0c1aae3971baf4c7)).
Version 0.17.0 was released on 2025-11-18. The latest verified source change is a theme-listener
fix authored on 2026-02-04 and committed on 2026-02-05
([release](https://github.com/badboy/mdbook-mermaid/releases/tag/v0.17.0),
[latest commit](https://github.com/badboy/mdbook-mermaid/commit/25c2b56daed067db36fc224e1d93054c5ca6531c)).

The important property is ownership: the book does not depend on the preprocessor as a Rust
library. Users install one tool, and that tool participates only in documentation builds.

### Native build-time precedent: `mdbook-mermaid-mmdr`

`mdbook-mermaid-mmdr` applies the same external-preprocessor shape but calls
`mermaid-rs-renderer` during `mdbook build` and inserts static SVG. The installed binary owns the
native renderer closure; the documented project does not
([README](https://github.com/adamcavendish/mdbook-mermaid-mmdr/blob/1daaea6411afe69a5638db3679d6214cb4a934dc/README.md),
[render implementation](https://github.com/adamcavendish/mdbook-mermaid-mmdr/blob/1daaea6411afe69a5638db3679d6214cb4a934dc/src/renderer.rs)).

The project is recent and lightly adopted: 0.1.3 was published 2026-07-01, with 433 total registry
downloads at the snapshot
([registry record](https://crates.io/api/v1/crates/mdbook-mermaid-mmdr)). Its manifest depends on
the 0.2 line of `mermaid-rs-renderer`, not the latest 0.3 line
([manifest](https://github.com/adamcavendish/mdbook-mermaid-mmdr/blob/1daaea6411afe69a5638db3679d6214cb4a934dc/Cargo.toml)).
It is not strong compatibility evidence, but it proves the distribution shape: native build-time
SVG does not require placing the renderer in every documented package's dependency graph.

The latest `mermaid-rs-renderer` release is 0.3.1 from 2026-07-06. Its own default features include
CLI and PNG support, while an SVG-only embedder can disable those defaults
([v0.3.1 manifest](https://github.com/1jehuang/mermaid-rs-renderer/blob/2f993bd79a55235eb59a34d807852276ba25bea7/Cargo.toml),
[registry metadata](https://crates.io/api/v1/crates/mermaid-rs-renderer)). This is another reminder
to measure the exact selected closure rather than treating "pure Rust" as synonymous with "small
build".

## Merman's actual competitive advantage

Merman should not compete on the same axis as `simple-mermaid`. Its defensible differentiation is:

| Capability | Aquamarine / simple-mermaid | `merman-rustdoc` today |
| --- | --- | --- |
| Render location | Reader's browser | Rust documentation build |
| Executable web dependency | Mermaid.js | None |
| Network fallback | Aquamarine: yes; simple-mermaid: required | None |
| Output when page loads | Source placeholder until JS runs | Complete inline SVG |
| Determinism | Depends on browser, assets, and possibly floating CDN version | Pinned Merman and deterministic render environment |
| Invalid source feedback | Browser console/page behavior | Build error with doc line and source preview, or explicit keep-source policy |
| SVG security policy | Mermaid/browser output used directly | Strict post-render validation by default |
| Rustdoc theme | Browser render/reload logic | Prebuilt light and dark SVG with CSS switching |
| Render variants | Mermaid.js browser contract | Parity, readable, and resvg-compatible SVG |
| Runtime prerequisites | JavaScript/module loading | None after documentation is generated |

The costs are equally explicit:

- the renderer is compiled for the host as part of a compiler plugin;
- the default selects optional layout engines and math for every user;
- the default Rustdoc theme renders each diagram twice;
- repeated macro expansion performs synchronous rendering in the compiler process; and
- item-level macro inspection cannot cover crate inner docs, external module files, or arbitrary
  `#[doc = include_str!(...)]` Markdown without a wider source-processing design.

The conclusion is not that Merman has no advantage. It is that the current crate charges every
selected consumer the full implementation cost to obtain that advantage.

## Architecture options

| Option | Consumer build cost | Static/offline result | docs.rs | Main risk | Verdict |
| --- | --- | --- | --- | --- | --- |
| Keep current proc macro; tune features | High whenever selected | Yes | Works within resource limits | Base SVG closure remains large | Immediate mitigation only |
| Aquamarine-style browser JS proc macro | Low | No | JS allowed, not guaranteed | Loses Merman's primary differentiation | Optional web mode only |
| Rustdoc `--html-*` injection | None | No | Can inject through args | Setup and unstable selectors/assets | Useful explicit web mode |
| Move renderer to `build.rs`/build-dependency | High whenever selected | Yes | Possible | Same closure, broader build-script execution | Reject |
| Thin proc macro spawning an installed renderer | Low Cargo closure | Yes | Tool unavailable | Non-hermetic builds and PATH/version failures | Reject as default |
| Thin proc macro + embedded pure-WASM Merman guest | Measured low host closure | Yes | Self-contained | Interpreter latency and font-policy provenance | Future-only; U7 latency FAIL at `28.101420x` |
| Postprocess `target/doc` with external Merman | Tool installed once | Yes | Cannot run custom subcommand | Rustdoc DOM drift | Good local/CI lane |
| CLI-generated checked fragments and receipts | None attributable during consumer docs | Yes | Works with packaged files | Generated-file freshness and diffs | Chosen cheap path |
| Client-side Merman WASM | Low Rust closure | No | Asset/JS integration required | Large runtime asset and async failure surface | Not needed for Rustdoc |

### Why `build.rs` is not a fix

Cargo build dependencies exist to compile and run build scripts
([Cargo build dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#build-dependencies)).
Moving Merman from a proc-macro dependency to a build dependency changes the owner but not the
fundamental cost: Cargo still builds the host renderer whenever that package target is built with
the dependency selected. A `DOCS_RS` environment branch can skip *execution* in docs.rs, but cannot
make an already selected build dependency disappear. It also makes a documentation concern run in
ordinary package builds.

### Why an external tool cannot be the only docs.rs path

docs.rs permits Cargo options and Rustdoc flags but explicitly rejects a Cargo subcommand in
`cargo-args` ([metadata](https://docs.rs/about/metadata)). Its build sandbox blocks network access,
limits RAM and Rustdoc time, and makes most source directories read-only
([build environment](https://docs.rs/about/builds)). Therefore docs.rs cannot install or invoke an
unlisted `cargo merman-doc` workflow on behalf of a crate. A pre-generated artifact included in the
published package, or the existing in-process macro dependency, is required.

## Future-only candidate architecture: embedded static guest

The repository already builds a pure-WASM Typst renderer and exercises its closed import/export
surface through `wasmi` ([Typst guest smoke](../../crates/xtask/src/cmd/typst_plugin_smoke.rs),
[WASM evidence](../workstreams/wasm-feature-surface-slimming/EVIDENCE_AND_GATES.md)). This is
source-backed precedent for the transport shape, not proof that a full-capability Rustdoc guest
will be fast, small, or output-equivalent enough. It establishes only that an admitted Merman
profile can return SVG through a pure-WASM protocol without browser glue.

Use a dedicated Rustdoc guest rather than silently inheriting Typst's capability and resource
policy:

```text
merman-rustdoc proc-macro host
  -> parse item docs, fences, includes, and options
  -> versioned batch render request
  -> embedded release-built merman-rustdoc guest via wasmi
  -> one or two deterministic SVG variants
  -> host-side strict SVG validation
  -> existing Rustdoc HTML wrapper
```

The external interface remains `#[merman_rustdoc::merman(...)]`. Renderer capability selection is
no longer a consumer Cargo feature surface: the guest artifact carries the admitted complete
Rustdoc capability set. The host keeps only syntax rewriting, local include resolution, request
encoding, bounded guest execution, SVG validation, and HTML generation.

The guest protocol must be versioned and batch light/dark variants for one macro expansion. Its
manifest must bind the ABI, Merman version, Mermaid baseline, capability set, exact artifact hash,
build recipe, and legal closure. The host must reject incompatible manifests before execution,
apply fuel/memory/input/output limits, and validate returned SVG rather than trusting guest bytes as
HTML. The package must never download the guest or recursively run Cargo from `build.rs`.

Do not retain the native renderer as another feature in the embedded proc-macro host after
migration. Cargo features are additive, so `--all-features` would select it and recreate the
original problem. If a source-built legacy adapter is temporarily necessary, give it a companion
package and a removal date.

### Embedded-guest admission gates

Before changing the published default, compare the guest against the current native macro on the
same host and fixtures:

- unique normal + build package closure, with a target of at most 30 for the host;
- clean and warm `cargo doc` wall time, peak RSS, and target-directory growth;
- raw/compressed guest and final `.crate` size, with the U7 candidate rejected at `>=8 MiB`, a
  stricter planning gate than crates.io's current 10 MB limit
  ([Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html#packaging-a-crate));
- normalized SVG DOM and representative screenshot parity across ordinary, Cytoscape, ELK, and
  math diagrams;
- 1, 10, and 100-diagram expansion latency, including repeated-source cache behavior;
- offline/read-only docs.rs-shaped execution; and
- ABI mismatch, corrupt guest, trap, fuel exhaustion, memory limit, oversized output, and unsafe
  SVG failures.

The package-closure and package-size values above were proposals when this research was first
written. U7 froze them at `<=30` unique normal-plus-build host packages and `<8 MiB` for the final
`.crate`, then measured them as recorded below. Package size was a material risk: the repository's
Typst release measurement records a 9,893,522-byte optimized/stripped WASM and a 3,791,109-byte
gzip form ([WASM size budgets](../release/WASM_SIZE_BUDGETS.json)). Gzip size is not a substitute
for measuring Cargo's final `.crate` archive.

### U7 bounded full-capability WASM spike

**Verdict:** `REJECT` for the current refactor and keep WASM future-only. The candidate passed every
frozen correctness, closure, size, isolation, package-shape, and reproducibility check, but failed
the warm-render admission gate by a wide margin: `35,744,416 ns / 1,271,979 ns = 28.101420x`, where
the preregistered maximum was `2.0x`. No gate was changed after observing the result.

This was a disposable feasibility harness, not product implementation. It used the existing Typst
minimal-protocol transport idea but built a dedicated Rustdoc guest with `svg`, Cytoscape, ELK, and
math enabled. The guest ABI was `rustdoc-wasm-spike-abi-1`; its reported capability string was
`svg,layout-cytoscape,layout-elk,math,light-dark`. The host accepted exactly the two protocol
function imports and applied ABI, fuel, memory, and output bounds. It did not reuse the constrained
Typst policy guest.

#### Frozen gate result

| Frozen gate | Result | Evidence |
| --- | --- | --- |
| Dedicated full-capability pure guest | **PASS in the bounded experiment** | All five fixture families rendered; the final artifact imported only the two `typst_env` protocol functions. A temporary target-specific RaTeX font-discovery patch was required, as disclosed below. |
| Host unique normal + build packages `<=30` | **PASS** | 19 normal packages and 19 normal-plus-build packages. |
| Projected final `.crate` `<8 MiB` | **PASS** | Actual package-shaped archive: 4,542,972 bytes (`4.333 MiB`), 3,845,636 bytes below the frozen ceiling. |
| Flowchart, sequence, architecture/Cytoscape, ELK, math, light/dark parity | **PASS** | 10/10 native/WASM outputs were byte-identical; every light/dark pair differed. |
| Warm render `<=2x` native oracle | **FAIL** | Native median 1,271,979 ns; WASM median 35,744,416 ns; ratio `28.101420x`. |
| Workloads 1/10/100 with wall time and peak RSS | **PASS: evidence complete** | Three process samples per lane and workload are below. No separate absolute RSS limit was preregistered; this row does not override the failed latency gate. |
| Malformed source, ABI mismatch, forbidden import, trap, fuel, memory, output limits | **PASS** | Every negative probe failed closed with the expected diagnostic class. |
| Repeated clean artifact hash | **PASS** | Raw and optimized/stripped artifacts were byte-identical across the two builds. |
| Offline/read-only/no CLI/no build-time-generation package shape | **PASS** | Unpacked package and consumer were read-only; isolated offline `cargo doc` rendered SVG with no `merman` executable and no `build.rs`. |
| No retained guest, harness, binary, or fallback feature | **PASS** | After evidence capture, the ignored experiment tree contained only the preregistered `experiment.yaml`; repository searches found no temporary implementation outside this report. |

The single latency failure rejects the backend. Passing package size, parity, or sandbox checks does
not compensate for it.

#### Measurement identity and host

The experiment began at `2026-08-14T11:45:13Z` from branch
`refactor/rustdoc-cli-generation` at source revision
`31cb42cf7ba22e3d400cee9bca6b2fcc2b3d80d0`. The working tree was already dirty and the branch was
three commits ahead of `origin/main`; the SHA-256 of the initial porcelain status was
`14433700f4ea0d17c414073dfadf7abc30c2c7b5229bfc16ae08f0ae66c27c70`. The experiment did not use
the unrelated tracked edits as inputs.

| Identity | Recorded value |
| --- | --- |
| Host | Mac16,7, Apple M4 Pro, arm64, 14 logical CPUs |
| OS | macOS 26.5.1 build 25F80; Darwin 25.5.0 |
| Physical memory | 51,539,607,552 bytes (48 GiB) |
| Rust | `rustc 1.95.0 (59807616e 2026-04-14)`, LLVM 22.1.2 |
| Cargo | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| Target/profile | installed `wasm32-unknown-unknown`; release, `opt-level="z"`, fat LTO, one codegen unit, panic abort |
| WASM tools | `wasm-opt version 131`; `wasm-tools 1.253.0` |
| Interpreter | locked `wasmi 1.1.0` |
| Repository `Cargo.lock` SHA-256 | `ee22330d16c7bc79ad45121be6fe0c8bc2e789150372b4d7390a83d282ded7e2` |
| Disposable experiment `Cargo.lock` SHA-256 | `f4f308da60a5b011ba1820eec486b52a7c00579725b01b48744b93dcf5ea5f9a` |

Cargo builds were serialized with `CARGO_BUILD_JOBS=1` and excluded from render timings. Registry
access was disabled after the disposable lockfile was established; measured build, tree, package,
and documentation commands used `--locked --offline`.

#### Artifact and package measurements

The first unpatched full-capability guest compiled, but its math closure reached
`ratex-unicode-font -> system-fonts -> web-sys` and introduced four wasm-bindgen/externref imports
in addition to the two protocol imports. Exact `wasm-bindgen-cli 0.2.127` post-processing still left
two JavaScript imports, so that artifact failed the pure import surface. The one permitted bounded
optimization was a disposable target-specific patch to `ratex-unicode-font`: system-font and emoji
discovery return `None` only on `wasm32`, while native behavior remains unchanged. This removed the
browser imports without removing math. It is experimental evidence, not a product patch.

| Artifact | Bytes | MiB | SHA-256 |
| --- | ---: | ---: | --- |
| Unpatched raw guest | 15,157,563 | 14.455 | `136cae7b1c59e18fbf4bcaefd4b76d06bf9cf63b79ec1f8ade68c2ce6e36262e` |
| Unpatched raw, stripped | 14,929,745 | 14.238 | `ae7dd77d01688220d3b7fd282b77bf60069ee7158549f8e408897015d42b9d8b` |
| Unpatched `wasm-opt -Oz` | 12,160,290 | 11.597 | `3dd86e291a1b3d680e7a8ce09ab58e2bea877288ed527d5d34b924dc4f2723db` |
| Unpatched optimized, stripped | 11,932,472 | 11.380 | `0780c84af5c5f1783cfc3953ff7fcad90c28b8fb01743060a231d749bb24369d` |
| Unpatched optimized, stripped, gzip `-9` | 4,573,263 | 4.361 | `ab5640bd0692cb1da46398714b662b046505066a3d6d74e3ecbff997bbe904be` |
| Final pure guest, raw | 14,661,349 | 13.982 | `ae0ac2684bd39411d647b74f81bd2b74c2aaa5187b7e3599223665f1357fa2d0` |
| Final pure guest, raw stripped | 13,096,286 | 12.490 | `9284094c42bcf984b4609a26e66a26cdfaa387b883432ed1f3c6dbddcbd20676` |
| Final pure guest, `wasm-opt -Oz` | 11,786,621 | 11.241 | `a7098270c161dab8e53fad45d2c3e1e0212cdca49776f9f30ac2304924267496` |
| Final pure guest, optimized and stripped | 11,786,391 | 11.240 | `b6f871067d6c3bd3062a893446be5cf3ec6abb7cd64081102b19839a25904bb5` |
| Final pure guest, optimized/stripped, gzip `-9` | 4,529,699 | 4.320 | `7cef7063d211cfece8e843d68bd4fd9681416b7eecfa8062b97406220aac5508` |
| Final candidate `.crate` | 4,542,972 | 4.333 | `8505970f170919e9c5ab78aa1f218e7b105698dbc893c47a7f5bd6cd63bfa3b8` |

The `.crate` contained six files: `Cargo.lock`, normalized and original manifests, `README.md`,
`assets/guest.wasm`, and `src/lib.rs`. Cargo reported 11.3 MiB unpacked and 4.3 MiB compressed. It
contained no `build.rs` and did not generate or download the guest.

The final guest had 93 initial memory pages and exactly these imports:

```text
typst_env::wasm_minimal_protocol_send_result_to_host
typst_env::wasm_minimal_protocol_write_args_to_buffer
```

Its exports were `memory`, `abi_version`, `allocate_output`, `capabilities`, `fuel_probe`,
`render_svg`, `trap_probe`, `__data_end`, and `__heap_base`.

The candidate host's 19-package normal-plus-build closure was: `bitflags`, `itoa`, `libm`,
`memchr`, `proc-macro2`, `quote`, `roxmltree`, `rustdoc-wasm-host-candidate`, `serde_core`,
`serde_json`, `spin`, `syn`, `unicode-ident`, `wasmi`, `wasmi_collections`, `wasmi_core`,
`wasmi_ir`, `wasmparser`, and `zmij`.

#### Output parity

The native oracle called the same `merman-bindings-core::render_svg` function with the same four
capability features, sources, and serialized options as the guest. This is a render-backend oracle,
not the existing Typst guest. All 10 comparisons passed exact `cmp`, which is stronger than the
preregistered semantic/static-SVG requirement.

| Fixture/theme | Bytes | Shared native/WASM SHA-256 |
| --- | ---: | --- |
| Flowchart/light | 13,978 | `32216aaab4d4f50275f9e37b1bcf9ba9f60d1919fc8e92a1e0a36ffffb288484` |
| Flowchart/dark | 14,295 | `50bf771b1ce946ca1aa3d8680a44a2808553a42ff5225c76517d9bb1183f10e5` |
| Sequence/light | 21,785 | `9434db9cb1e3080933e2c30fee2ce730af7244722b0ee5f140352ecd1ddc57e2` |
| Sequence/dark | 21,949 | `b04632e68fc86ca69670f9e09504b511357a4621d0391ad76e9b45e6aba913d0` |
| Architecture/Cytoscape/light | 5,906 | `f0ff23bfa27c66dbaa00b7934461310eba0499caefd065e1bb7e4abd6724d789` |
| Architecture/Cytoscape/dark | 5,947 | `327de0dd6baaa1311de256cf0afb1edd83bee934e966a58b17a36a158bbc0eae` |
| ELK/light | 10,523 | `6a28fa20842252a6bbbde868af551c7fa398f3d5dc47ec7872d7a747f9b30567` |
| ELK/dark | 10,846 | `034f9d997074ef5202284477fecd2e752e69eebcf767b0c8a84233ecff1d7e09` |
| Math/light | 18,293 | `62df352f7aecd89c6289f5bfc5a5e50ce2248d48cd4dcabfedcc6214005f2efc` |
| Math/dark | 18,616 | `0908e6a07ab70bf9a989a2c533c3a8c2f79ea7ed6c07f312a05d43fcd6e79de1` |

#### Warm-render timing

Both release binaries were built before timing. Each lane performed three unrecorded warmups, then
16 measured renders in AB/BA process order: native, WASM, WASM, native. Thus each lane has 32
samples. The WASM lane reused one instantiated module/store and reset fuel per call; module compile
and instantiation were outside the internal samples. Both lanes rendered the same built-in
flowchart and options and produced 13,390-byte outputs.

| Lane | Samples | Median | Mean | Min | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native oracle | 32 | 1,271,979 ns | 1,329,792 ns | 1,173,459 ns | 1,799,459 ns |
| `wasmi` guest | 32 | 35,744,416 ns | 35,796,139 ns | 34,955,375 ns | 36,928,292 ns |

The raw native samples in nanoseconds were:

```text
1473417 1543916 1486500 1385166 1368459 1230667 1176958 1202500
1188958 1208666 1173459 1217542 1799459 1480875 1451666 1265291
1335583 1376792 1237208 1204833 1205583 1190584 1190958 1191375
1490500 1212958 1553791 1278667 1502500 1406458 1314584 1207459
```

The raw WASM samples in nanoseconds were:

```text
35620750 34955375 35793833 35735083 35387959 36190500 36370000 36028167
35443291 35559459 35581083 35743792 35153625 36151750 35799667 36705750
35745041 35899000 36272500 35812209 36362959 35338666 36155708 36129459
35544000 35500083 36928292 36033167 35522833 35291333 35623291 35097833
```

#### Workload wall time and RSS

For each lane and count, three independent `/usr/bin/time -l` processes performed two internal
warmups, then timed 1, 10, or 100 repeated renders in one persistent native process or WASM
instance. `elapsed_ns` is the render loop only; `real` includes process startup and, for WASM,
module compile/instantiation. Peak RSS is the maximum of the three macOS `maximum resident set
size` samples. Cargo was not running.

| Lane/count | Internal render ns, raw three samples | Median internal | Process `real`, raw seconds | Median `real` | Peak RSS |
| --- | --- | ---: | --- | ---: | ---: |
| Native/1 | 1,407,333; 1,265,875; 1,311,750 | 1,311,750 ns | 0.01; 0.01; 0.01 | 0.010 s | 21,725,184 B (20.719 MiB) |
| Native/10 | 12,612,083; 12,505,458; 12,937,750 | 12,612,083 ns | 0.02; 0.02; 0.02 | 0.020 s | 21,921,792 B (20.906 MiB) |
| Native/100 | 126,981,834; 126,822,958; 126,382,542 | 126,822,958 ns | 0.14; 0.13; 0.13 | 0.130 s | 21,954,560 B (20.938 MiB) |
| WASM/1 | 35,796,000; 35,962,958; 35,630,541 | 35,796,000 ns | 0.45; 0.45; 0.45 | 0.450 s | 47,038,464 B (44.859 MiB) |
| WASM/10 | 357,508,917; 355,699,833; 355,362,708 | 355,699,833 ns | 0.77; 0.77; 0.78 | 0.770 s | 47,104,000 B (44.922 MiB) |
| WASM/100 | 3,550,173,167; 3,582,095,583; 3,608,067,875 | 3,582,095,583 ns | 3.97; 4.00; 4.03 | 4.000 s | 46,661,632 B (44.500 MiB) |

RSS stayed flat across workload count in both lanes, but the WASM process peak was about 2.1 times
the native peak. No absolute RSS threshold was preregistered, so this observation neither creates a
new failure nor rescues the latency failure.

#### Fail-closed and package-shape probes

| Probe | Exact setting | Result |
| --- | --- | --- |
| Malformed source | `flowchart TD\n  A[unterminated` | Exit 1; `BindingError` / `ParseError`: unterminated node label. |
| ABI mismatch | expected `rustdoc-wasm-spike-abi-999`; actual `rustdoc-wasm-spike-abi-1` | Exit 1 before render. |
| Forbidden import | added `network::fetch` to the two allowed imports | Exit 1 before instantiation: expected 2 imports, found 3. |
| Guest trap | exported `trap_probe` executes `unreachable` | Rejected as a WASM trap. |
| Fuel exhaustion | 1,000 fuel units; unbounded `fuel_probe` | Rejected: all fuel consumed by WebAssembly. |
| Memory limit | 67,108,864-byte cap; 134,217,728-byte request | Rejected: growth operation limited. |
| Output limit | 65,536-byte cap; 1,048,576-byte request | Rejected with actual and maximum byte counts. |

The package-shaped proc macro embedded the final guest, verified the ABI and two-import surface,
executed the guest during Rustdoc, parsed the result as SVG, and generated
`<div class="merman-rustdoc"><svg id="u7-package-smoke" ...>`. The unpacked candidate and consumer
were both mode `dr-xr-xr-x`. In an isolated `PATH` containing only symlinks to the exact
`cargo`/`rustc`/`rustdoc` toolchain plus `/usr/bin:/bin`, `command -v merman` returned exit 1.
`CARGO_NET_OFFLINE=true cargo doc --locked --offline --no-deps` completed in 16.63 seconds and the
HTML contained the `Offline` and `Read-only` labels. This proves the candidate used neither network,
an external Merman CLI, writable source directories, nor build-time guest generation.

#### Exact command ledger

The commands below are the retained textual reproduction record. Their disposable `$WS` operands
were intentionally deleted after measurement under KTD13.

```bash
ROOT="$(git rev-parse --show-toplevel)"
SPIKE="$ROOT/target/bench/experiments/rustdoc-wasm-spike"
WS="$SPIKE/workspace"
export CARGO_BUILD_JOBS=1

date -u '+%Y-%m-%dT%H:%M:%SZ'
uname -a
sw_vers
system_profiler SPHardwareDataType
sysctl -n hw.logicalcpu hw.memsize
rustc -vV
cargo -V
rustup target list --installed
wasm-opt --version
wasm-tools --version
git rev-parse HEAD
git status --short --branch
git status --porcelain=v1 | shasum -a 256
shasum -a 256 "$ROOT/Cargo.lock"
shasum -a 256 "$WS/Cargo.lock"

cargo build --manifest-path "$WS/Cargo.toml" --locked --offline --release \
  --target wasm32-unknown-unknown -p rustdoc-wasm-guest
cp "$WS/target/wasm32-unknown-unknown/release/rustdoc_wasm_guest.wasm" \
  "$WS/results/guest-patched-build1.raw.wasm"
wasm-tools strip --all "$WS/results/guest-patched-build1.raw.wasm" \
  -o "$WS/results/guest-patched-build1.raw-stripped.wasm"
wasm-opt -Oz --enable-bulk-memory --enable-bulk-memory-opt --enable-multivalue \
  --enable-mutable-globals --enable-nontrapping-float-to-int --enable-reference-types \
  --enable-sign-ext "$WS/results/guest-patched-build1.raw.wasm" \
  -o "$WS/results/guest-patched-build1.optimized.wasm"
wasm-tools strip --all "$WS/results/guest-patched-build1.optimized.wasm" \
  -o "$WS/results/guest-patched-build1.optimized-stripped.wasm"
cp "$WS/results/guest-patched-build1.optimized-stripped.wasm" \
  "$WS/results/guest-patched-build1.optimized-stripped-for-gzip.wasm"
gzip -9 -f "$WS/results/guest-patched-build1.optimized-stripped-for-gzip.wasm"
wc -c "$WS/results"/guest-patched-build1.*
shasum -a 256 "$WS/results"/guest-patched-build1.*

cargo build --manifest-path "$WS/Cargo.toml" --locked --offline --release \
  -p rustdoc-wasm-host -p rustdoc-native-oracle
HOST="$WS/target/release/rustdoc-wasm-host"
NATIVE="$WS/target/release/rustdoc-native-oracle"
GUEST="$WS/results/guest-patched-build1.optimized-stripped.wasm"

"$HOST" surface "$GUEST"
"$HOST" abi "$GUEST" rustdoc-wasm-spike-abi-1
"$HOST" capabilities "$GUEST"
wasm-tools print "$GUEST" \
  | sed -n '/^[[:space:]]*(import/p;/^[[:space:]]*(export/p'

for fixture in flowchart sequence architecture elk math; do
  for theme in light dark; do
    "$NATIVE" render "$WS/fixtures/$fixture.mmd" "$WS/options/$theme.json" \
      "$WS/results/native-$fixture-$theme.svg"
    "$HOST" render "$GUEST" "$WS/fixtures/$fixture.mmd" "$WS/options/$theme.json" \
      "$WS/results/wasm-$fixture-$theme.svg"
    cmp "$WS/results/native-$fixture-$theme.svg" \
      "$WS/results/wasm-$fixture-$theme.svg"
  done
done

"$NATIVE" bench 16 >"$WS/results/warm-native-a.txt"
"$HOST" bench "$GUEST" 16 >"$WS/results/warm-wasm-a.txt"
"$HOST" bench "$GUEST" 16 >"$WS/results/warm-wasm-b.txt"
"$NATIVE" bench 16 >"$WS/results/warm-native-b.txt"

for lane in native wasm; do
  for count in 1 10 100; do
    for sample in 1 2 3; do
      if test "$lane" = native; then
        /usr/bin/time -l -o "$WS/results/workload-$lane-$count-$sample.time" \
          "$NATIVE" workload "$count" \
          >"$WS/results/workload-$lane-$count-$sample.out"
      else
        /usr/bin/time -l -o "$WS/results/workload-$lane-$count-$sample.time" \
          "$HOST" workload "$GUEST" "$count" \
          >"$WS/results/workload-$lane-$count-$sample.out"
      fi
    done
  done
done

"$HOST" render "$GUEST" "$WS/fixtures/malformed.mmd" "$WS/options/light.json" \
  "$WS/results/should-not-exist.svg"
"$HOST" abi "$GUEST" rustdoc-wasm-spike-abi-999
wasm-tools parse "$WS/fixtures/forbidden-import.wat" \
  -o "$WS/results/forbidden-import.wasm"
"$HOST" surface "$WS/results/forbidden-import.wasm"
"$HOST" trap "$GUEST"
"$HOST" fuel "$GUEST"
"$HOST" memory "$GUEST" 67108864 134217728
"$HOST" output "$GUEST" 65536 1048576

for edges in normal normal,build; do
  cargo tree --locked --offline --manifest-path "$WS/Cargo.toml" \
    -p rustdoc-wasm-host-candidate --edges "$edges" --prefix none --format '{p}' \
    | sed 's/ (\*)$//' | sort -u \
    | tee "$WS/results/host-${edges/,/-}-packages.txt" | wc -l
done

cp "$GUEST" "$WS/candidate/assets/guest.wasm"
cargo package --locked --offline --allow-dirty --no-verify \
  --manifest-path "$WS/candidate/Cargo.toml"
CRATE="$WS/target/package/rustdoc-wasm-host-candidate-0.0.0.crate"
wc -c "$CRATE"
shasum -a 256 "$CRATE"
tar -tzf "$CRATE"

# After unpacking the .crate and creating the consumer, the package smoke ran:
SMOKE="$WS/results/package-smoke"
mkdir -p "$SMOKE/source" "$SMOKE/bin"
tar -xzf "$CRATE" -C "$SMOKE/source"
for tool in cargo rustc rustdoc; do
  ln -s "$(rustup which "$tool")" "$SMOKE/bin/$tool"
done
chmod -R a-w "$SMOKE/source" "$SMOKE/consumer"
stat -f '%Sp %N' "$SMOKE/source" "$SMOKE/consumer"
! PATH="$SMOKE/bin:/usr/bin:/bin" command -v merman
PATH="$SMOKE/bin:/usr/bin:/bin" CARGO_NET_OFFLINE=true \
  CARGO_TARGET_DIR="$SMOKE/target" cargo doc --locked --offline --no-deps \
  --manifest-path "$SMOKE/consumer/Cargo.toml"
test ! -e "$SMOKE/source/rustdoc-wasm-host-candidate-0.0.0/build.rs"
rg 'u7-package-smoke|Offline|Read-only' "$SMOKE/target/doc"

# Reproducibility: record build 1, clean, repeat the exact guest build/optimization recipe,
# and compare build 2.
cargo clean --manifest-path "$WS/Cargo.toml"
cmp "$WS/results/guest-patched-build1.raw.wasm" \
  "$WS/results/guest-patched-build2.raw.wasm"
cmp "$WS/results/guest-patched-build1.optimized-stripped.wasm" \
  "$WS/results/guest-patched-build2.optimized-stripped.wasm"
shasum -a 256 "$WS/results"/guest-patched-build{1,2}.{raw,optimized-stripped}.wasm
```

The clean removed 3,561 disposable files (1.4 GiB). Both raw builds had SHA-256
`ae0ac2684bd39411d647b74f81bd2b74c2aaa5187b7e3599223665f1357fa2d0`; both optimized/stripped
builds had SHA-256 `b6f871067d6c3bd3062a893446be5cf3ec6abb7cd64081102b19839a25904bb5`.

The experiment has two unresolved productization gaps, neither hidden by a changed gate:

- `wasmi` warm rendering is `28.101420x` native on this host, so the current interpreter strategy
  is not viable for the plan.
- The full math guest requires an intentionally designed, reviewed, and upstreamable WASM policy
  for RaTeX system-font discovery. The disposable patch proved feasibility but is not shippable
  provenance.

Any future attempt must preregister and rerun the same gates, including the `<=2x` latency ceiling,
instead of treating this artifact as an admitted backend.

#### Cleanup proof

KTD13 cleanup removed the disposable workspace, guest source, RaTeX patch, host/native/candidate
harnesses, package smoke, measurements, SVGs, `.crate`, WASM files, tool installation, Cargo target,
and binaries. The first recursive removal correctly encountered the deliberately read-only package
smoke. Cleanup restored user write permission only on those two disposable read-only fixture trees,
then repeated the same bounded removal. Nothing was restored or deleted outside the ignored spike
directory.

```bash
find "$SPIKE" -mindepth 1 -maxdepth 1 ! -name experiment.yaml -print
find "$SPIKE" -mindepth 1 -maxdepth 1 ! -name experiment.yaml \
  -exec rm -rf -- {} +
# The read-only package smoke rejected removal, so only those disposable trees were unlocked.
chmod -R u+w \
  "$WS/results/package-smoke/source" \
  "$WS/results/package-smoke/consumer"
find "$SPIKE" -mindepth 1 -maxdepth 1 ! -name experiment.yaml \
  -exec rm -r -- {} +

find "$SPIKE" -mindepth 1 -print | sort
git check-ignore -v "$SPIKE/experiment.yaml"
rg -n 'rustdoc-wasm-host-candidate|rustdoc-wasm-guest|rustdoc-native-oracle|rustdoc-wasm-spike-abi|guest-patched-build|u7-package-smoke' \
  . --glob '!target/**' \
  --glob '!docs/research/rustdoc-mermaid-ecosystem-2026-08-14.md'
rg -n -i 'rustdoc[-_[:alnum:]]*wasm|wasm[-_[:alnum:]]*rustdoc' \
  Cargo.toml crates scripts .github \
  --glob 'Cargo.toml' --glob '*.rs' --glob '*.py' --glob '*.yml' --glob '*.yaml'
find crates scripts .github -type f \
  \( -name '*.wasm' -o -name '*rustdoc*wasm*' \) -print
git status --short --untracked-files=all -- \
  docs/research/rustdoc-mermaid-ecosystem-2026-08-14.md "$SPIKE"
git status --porcelain=v1 | shasum -a 256
git diff --check -- docs/research/rustdoc-mermaid-ecosystem-2026-08-14.md
```

The final `find` output contained only
`target/bench/experiments/rustdoc-wasm-spike/experiment.yaml`; `git check-ignore` attributed it to
the repository's `target/` rule. Both identifier searches and the product artifact search returned
no match. Scoped `git status` listed only this research report, and the final whole-worktree
porcelain-status SHA-256 was
`05aa91e5a842cea87c70a70d6459f6f7c1c0ae8a9c1489976d7af1dfcc2294d8`. `git diff --check`
returned no output.

## Chosen static architecture: CLI-generated checked fragments

### Shipped paths

The refactor resolves the distribution boundary with two explicit paths rather than an automatic
fallback chain:

```text
Cheap, explicit generation
  -> merman-cli rustdoc build/check
       -> checked static Rustdoc fragments and receipts
       -> ordinary cargo doc / docs.rs consumes committed output

One-step attribute expansion
  -> retained merman-rustdoc proc macro
       -> current native in-process renderer
       -> consumer deliberately accepts that Cargo closure
```

The earlier research considered a separate `merman-rustdoc-sidecar` facade. It remains an optional
future packaging choice, not the result selected by this refactor: the implemented cheap boundary
is the existing CLI plus checked generated files. If a facade is later added, it can be a normal,
`no_std`, zero-dependency library with declarative macros similar in shape to `simple-mermaid`, but
it must embed checked SVG rather than a script tag. It must remain separate from a procedural-macro
crate ([Rust Reference](https://doc.rust-lang.org/reference/procedural-macros.html)). For example:

```rust
/// System architecture.
#[doc = merman_rustdoc_sidecar::svg_pair!(
    "../docs/architecture.light.svg",
    "../docs/architecture.dark.svg"
)]
pub mod architecture {}
```

The exact macro API should support a fixed single SVG and a light/dark pair. It should not parse or
render Mermaid. Generation and validation belong to the external tool.

### Keep generation explicit and bounded

Do not make the tool scan arbitrary Rust syntax, infer macro call graphs, or reconstruct every
Rustdoc source form. Use a small structured manifest, for example:

```toml
[[diagram]]
source = "docs/architecture.mmd"
output = "docs/generated/architecture"
theme = "rustdoc"
pipeline = "readable"
features = ["layout-elk"]
```

`merman-cli rustdoc build` updates declared outputs. `merman-cli rustdoc check` renders to temporary
files and fails if source, receipt, or SVG hashes differ. The receipt should record at least:

- source SHA-256;
- Merman crate/tool version and Mermaid baseline (`11.16.1` at this snapshot);
- renderer capability leaves and pipeline;
- theme and sanitization policy; and
- output SHA-256 values.

This avoids maintaining an incomplete Rust parser in a release helper and makes packaging checks
straightforward: every referenced sidecar and receipt must be included by `cargo package`.

### Retain the native proc macro as an explicit capability

Keep the current `merman-rustdoc` implementation for users who want high-fidelity inline expansion
and knowingly accept its renderer closure. Do not add it as an optional dependency or fallback of
the CLI-generated path. Such a feature edge would recreate the original problem whenever a
consumer runs `--all-features`: Cargo would select the native proc macro and compile its complete
transitive closure even when `cfg(doc)` is false.

This makes the cheap contract explicit without deleting the one-step experience. Documentation
must state that consumers which add and feature-gate `merman-rustdoc` will pay its build cost in
their own `--all-features` jobs. Because the crate is still in an alpha release line and had 242
total registry downloads at the snapshot, this boundary is better clarified before stable. The
download count does not prove the number of affected users, so migration documentation remains
required.

### Reuse code only where ownership is real

The current renderer and include traits are enough to test the proc macro. Do not immediately add a
large `merman-rustdoc-core` abstraction. Extract only stable, shared units:

- the SVG safety validator and Rustdoc light/dark wrapper if both the generator and static macro use
  them;
- receipt types if more than one tool consumes them; and
- normalized rendering options shared with the existing Merman CLI.

The HTML postprocessor and Rust source attribute transformer process different representations and
should not be forced behind one generic rewriting framework.

## Resolved migration path

U7 settles the earlier migration fork. The measured path is:

1. Make `merman-cli rustdoc build/check` the explicit low-cost workflow, with deterministic checked
   fragments, receipts, package verification, and no renderer in ordinary consumer documentation
   builds.
2. Retain `merman-rustdoc` as the explicit native one-step workflow. Keep its high-fidelity
   behavior rather than hiding its closure behind misleading `cfg_attr(doc)` guidance.
3. Do not add a WASM backend, fallback feature, embedded artifact, build-time generator, or release
   ownership machinery in this refactor. U7 failed the `<=2x` warm-render gate at `28.101420x`.
4. Keep browser JavaScript and Rustdoc HTML post-processing optional and explicit; neither silently
   substitutes for the deterministic static contract.
5. Revisit WASM only as a new preregistered experiment after interpreter performance and the RaTeX
   WASM font-discovery policy materially change enough to justify a new run. Rerun all U7 gates
   without relaxing them.

## Acceptance criteria

The refactor is complete only when all of the following hold:

- `merman-cli rustdoc build/check` owns deterministic generation and stale-output detection, and
  package tests prove that every referenced generated fragment and receipt ships.
- The checked-fragment consumer path has no attributable Merman renderer closure during ordinary
  `cargo doc` or docs.rs execution.
- Generated light/dark static output works without JavaScript and without duplicate DOM ids, and
  passes the same strict SVG policy as the native path.
- `merman-rustdoc` remains a separate, explicit native dependency; documentation accurately
  explains Cargo feature selection, `cfg_attr(doc)`, and the cost of consumer `--all-features`.
- No WASM guest, host harness, embedded artifact, fallback feature, or build-time guest generation
  remains in the product after U7.
- If WASM is proposed again, the exact candidate must pass the frozen closure, `<8 MiB` package,
  parity, latency, resource, sandbox, reproducibility, and offline/read-only package gates before
  any default or distribution decision changes.

## Risks and controls

| Risk | Control |
| --- | --- |
| Checked SVG becomes stale | Mandatory receipt plus `merman-cli rustdoc check` in CI and release preflight |
| Generated files create noisy reviews | Stable serialization, deterministic ids, one source/output mapping per manifest entry |
| Generated fragments increase repository/package size | Measure light/dark pairs; permit fixed-theme single SVG where appropriate |
| Unsafe SVG is committed | Strict validation at generation and check time; fail closed by default |
| Tool and crate versions drift | Record exact tool/Merman/baseline in receipt and verify it |
| External tool is missing locally | Clear install command; checked files keep ordinary builds and docs.rs independent |
| Postprocessor breaks on Rustdoc HTML change | Narrow structured HTML parser, toolchain matrix, visible failure, checked-fragment path |
| A future embedded guest is slow, memory-heavy, or makes the `.crate` reach 8 MiB | Reuse the frozen U7 gates; do not ship or weaken the static contract on failure |
| Guest artifact drifts from source | Exact artifact recipe, ABI/hash manifest, reproducibility and package smoke |
| Native renderer is accidentally pulled into a lightweight package | No dependency edge between packages; `--all-features` closure checks |
| Default-feature change surprises alpha users | Prerelease migration guide and one release of targeted diagnostics |
| Per-diagram features become unmaintainable | Keep only evidence-backed layout/math capability leaves; do not mirror the family catalog in Cargo features |

## Rejected shortcuts

- **`cfg_attr(doc)` alone:** controls expansion, not dependency resolution.
- **A non-optional dependency with a documentation-only invocation:** always builds the proc macro.
- **`[target.'cfg(doc)'.dependencies]`:** Cargo target dependency selection is not Rustdoc
  source-level cfg evaluation.
- **Moving Merman into `build.rs`:** relocates, rather than removes, the host build closure.
- **Splitting only the proc macro and renderer library:** the renderer remains transitive and still
  compiles.
- **Making the native companion an optional dependency of either lightweight package:**
  `--all-features` selects it and recreates the expensive closure.
- **Downloading a renderer during build:** non-hermetic, blocked on docs.rs, and a supply-chain
  regression.
- **Making CDN Mermaid.js the silent fallback:** changes deterministic static documentation into
  remote executable content.
- **Adding a Cargo feature for every diagram:** multiplies configurations while preserving the
  in-process architecture.

## Final recommendation

Proceed with the dual explicit paths selected by the refactor. Use `merman-cli rustdoc build/check`
for deterministic checked static documentation without an attributable renderer closure in the
consumer's documentation build. Retain `merman-rustdoc` for users who explicitly prefer one-step
native expansion and accept its build cost. Keep both paths JavaScript-free, offline-capable, and
strictly validated.

Do not ship the U7 WASM design now. Its 19-package host, 4.333 MiB `.crate`, exact 10/10 output
parity, fail-closed sandbox, repeatable artifact hash, and offline/read-only package smoke are
promising future evidence, but the decisive measured result is the failed `<=2x` latency gate:
`28.101420x` native. The temporary RaTeX WASM font-discovery patch is also not product provenance.
No embedded guest, fallback feature, or build-time artifact generator should remain from the
experiment.

This keeps Merman's advantage over browser-only integrations without hiding costs: the cheap path
uses governed static output, the one-step path is explicitly native, and a future WASM proposal
must earn admission by passing the same frozen gates rather than by weakening them.
