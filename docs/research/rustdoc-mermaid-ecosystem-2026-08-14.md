# Rustdoc Mermaid Ecosystem and Merman Integration Architecture

**Researched:** 2026-08-14

**Merman snapshot:** `24b0be325c5b30483dc42ef20b37b7c2bf1f22bb`

**Registry snapshot:** 2026-08-14T04:32:39Z

**Decision:** refactor the Rustdoc integration and distribution boundary before a stable release;
first validate a thin proc-macro host backed by a release-built embedded Merman WASM guest. Keep
checked SVG sidecars as the zero-dependency fallback, and do not make browser-side Mermaid.js the
primary product.

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

The preferred long-term product is two explicit lanes:

1. **Embedded static lane (recommended default if the spike clears its gates):** keep the current
   attribute interface, but replace the native renderer dependency with a release-built pure-WASM
   Merman guest executed by a small `wasmi` host. The renderer is compiled once by Merman's release
   process rather than in every consuming Cargo graph.
2. **Thin, checked-sidecar lane (strict zero-dependency option):** an external Merman tool renders
   declared `.mmd` inputs to committed light/dark SVG sidecars and a receipt. A separately published
   normal library, called `merman-rustdoc-sidecar` in this report, provides zero-dependency
   declarative macros that embed those SVGs in Rustdoc. Ordinary builds and docs.rs do not compile
   or execute the renderer.

An optional `cargo merman-doc` HTML postprocessor can support inline fences for local and CI-hosted
documentation without putting Merman in the consuming crate's graph. It cannot be the sole docs.rs
solution because docs.rs metadata cannot run a Cargo subcommand.

This is a fearless refactor of the integration boundary, not a retreat to a weaker renderer. The
current crate has a defensible and uncommon advantage: it produces deterministic, sanitized,
build-time SVG without Node.js, a browser, runtime JavaScript, or a network fetch. The embedded
guest attempts to preserve that advantage and the current user interface; sidecars remain the
honest fallback if interpreter latency, memory, or package size does not clear a preregistered bar.

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
| Thin proc macro + embedded pure-WASM Merman guest | Estimated low host closure | Yes | Self-contained | Interpreter latency, guest size, release provenance | Preferred measured spike |
| Postprocess `target/doc` with external Merman | Tool installed once | Yes | Cannot run custom subcommand | Rustdoc DOM drift | Good local/CI lane |
| Checked SVG sidecars + zero-dependency macro | Tiny | Yes | Works with packaged files | Generated-file freshness and diffs | Zero-dependency fallback |
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

## Preferred target architecture: embedded static guest

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
- raw/compressed guest and final `.crate` size, with publication rejected if the actual packaged
  archive exceeds crates.io's current 10 MB limit
  ([Cargo publishing](https://doc.rust-lang.org/cargo/reference/publishing.html#packaging-a-crate));
- normalized SVG DOM and representative screenshot parity across ordinary, Cytoscape, ELK, and
  math diagrams;
- 1, 10, and 100-diagram expansion latency, including repeated-source cache behavior;
- offline/read-only docs.rs-shaped execution; and
- ABI mismatch, corrupt guest, trap, fuel exhaustion, memory limit, oversized output, and unsafe
  SVG failures.

The numeric package-closure target is a proposal, not measured evidence. Package size is already a
material risk: the repository's current Typst release measurement records a 9,893,522-byte
optimized/stripped WASM and a 3,791,109-byte gzip form
([WASM size budgets](../release/WASM_SIZE_BUDGETS.json)). A full Rustdoc guest may differ, and gzip
size is not a substitute for measuring Cargo's final `.crate` archive. If the guest exceeds the
registry limit or misses the agreed latency, memory, or package-size bars after one bounded
optimization pass, stop and adopt checked sidecars instead of weakening the static contract.

## Fallback target architecture: checked sidecars

### Product lanes

Use these as working package names; the behavioral boundaries matter more than the final names.

```text
Declared .mmd files
  -> cargo-merman-doc render/check
       -> diagram.light.svg
       -> diagram.dark.svg
       -> diagram.merman.json (source/options/output hashes and versions)
  -> merman-rustdoc-sidecar normal library (zero-dependency macro_rules)
       -> #[doc = inline static SVG wrapper]
       -> cargo doc / docs.rs

Inline Mermaid fences requiring one-command local docs
  -> cargo merman-doc build
       -> cargo rustdoc
       -> narrow HTML postprocessor
       -> Merman static SVG

Inline Mermaid fences requiring docs.rs build-time rendering
  -> merman-rustdoc-native attribute proc macro
       -> current Merman in-process renderer
```

The sidecar package can be a normal, `no_std`, zero-dependency library with declarative macros
similar in shape to `simple-mermaid`, but it should embed checked SVG rather than a script tag. It
must be a different package from the embedded guest's procedural-macro host: a proc-macro crate
cannot also serve as this ordinary declarative-macro facade
([Rust Reference](https://doc.rust-lang.org/reference/procedural-macros.html)). For example, the
user-facing shape could be:

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

`cargo merman-doc render` updates declared outputs. `cargo merman-doc check` renders to temporary
files and fails if source, receipt, or SVG hashes differ. The receipt should record at least:

- source SHA-256;
- Merman crate/tool version and Mermaid baseline (`11.16.1` at this snapshot);
- renderer capability leaves and pipeline;
- theme and sanitization policy; and
- output SHA-256 values.

This avoids maintaining an incomplete Rust parser in a release helper and makes packaging checks
straightforward: every referenced sidecar and receipt must be included by `cargo package`.

### Preserve the native proc macro as a separate premium capability

Move the current implementation to a companion proc-macro crate that is not an optional dependency
of either lightweight package. A feature edge would recreate the original problem whenever a
consumer runs `--all-features`: Cargo would select the optional native crate and compile its
complete transitive closure even though `cfg(doc)` is false. The package boundary, not a
disabled-by-default feature, is what keeps the sidecar lane cheap under every sidecar-package
feature combination.

Suggested native companion policy:

```toml
[features]
default = ["svg"]
svg = [
    "dep:merman",
    "dep:proc-macro2",
    "dep:quote",
    "dep:roxmltree",
    "dep:serde_json",
    "dep:syn",
    "merman/svg",
]
layout-cytoscape = ["svg", "merman/layout-cytoscape"]
layout-elk = ["svg", "merman/layout-elk"]
math = ["svg", "merman/math"]
complete-svg = ["layout-cytoscape", "layout-elk", "math"]
```

The sidecar package should expose only checked-sidecar inclusion APIs. Its
`cargo tree -p merman-rustdoc-sidecar --all-features` output must therefore remain renderer-free.
The embedded `merman-rustdoc` host likewise must have no dependency edge to the native companion.
Users who deliberately choose the native companion can still gate that dependency in their own
crate, but
their own `--all-features` run will predictably pay the renderer cost. That trade is explicit and
cannot be hidden by Cargo features.

This makes the cheap contract the default without deleting the high-fidelity inline experience.
Because `merman-rustdoc` is still in an alpha release line and has 242 total registry downloads at
the snapshot, the compatibility change is better made now than after a stable contract. The
download count does not prove the number of affected users, so migration documentation and a
deprecation interval are still required.

### Reuse code only where ownership is real

The current renderer and include traits are enough to test the proc macro. Do not immediately add a
large `merman-rustdoc-core` abstraction. Extract only stable, shared units:

- the SVG safety validator and Rustdoc light/dark wrapper if both the generator and static macro use
  them;
- receipt types if more than one tool consumes them; and
- normalized rendering options shared with the existing Merman CLI.

The HTML postprocessor and Rust source attribute transformer process different representations and
should not be forced behind one generic rewriting framework.

## Preferred migration path

### Phase 0: measure before changing defaults

Create a tiny published-crate-shaped fixture and record clean, same-host runs for:

- no Mermaid integration;
- `simple-mermaid` 0.2.0;
- Aquamarine 0.6.0;
- current `merman-rustdoc` `svg`;
- current default complete SVG; and
- the proposed embedded guest and sidecar macro.

Measure wall time, peak RSS, target-directory growth, downloaded crate bytes, and unique
normal/build packages. Run sequentially with an isolated Cargo home/target per cold sample and a
separate warm-cache lane. Do not publish closure counts as compile-time measurements.

### Phase 1: bounded embedded-guest spike

1. Add a non-published dedicated Rustdoc guest with an exact full-capability artifact recipe.
2. Exercise it through `wasmi` in an xtask smoke before changing the public proc macro.
3. Measure the admission gates above against current `svg` and complete-SVG native baselines.
4. Add a troubleshooting section explaining `cfg_attr(doc)`, `--all-features`, and Cargo feature
   unification.
5. Add locked closure receipts using a script that strips Cargo's repeated `(*)` markers.

If a release must ship before the spike completes, changing the native default from `complete-svg`
to `svg` is an acceptable breaking-alpha mitigation, but it is not completion of the architecture
work and it reduces out-of-box diagram capability.

### Phase 2: switch the existing host behind its current interface

1. Replace `HeadlessMermaidRenderer` with an internal embedded-guest adapter.
2. Preserve current fences, includes, options, diagnostics, wrappers, and host SVG validation.
3. Reuse one initialized guest per proc-macro process and cache repeated requests in memory.
4. Delete the host's `merman` dependency and renderer capability features; do not keep a native
   fallback feature.
5. Add package extraction and offline `cargo doc` tests against the exact `.crate` candidate.

### Phase 3: align architecture and release ownership

1. Supersede ADR-0076 only where it requires Rustdoc to mirror the facade's `complete-svg`
   features.
2. Split the artifact profile into a release-built guest and a lightweight proc-macro host.
3. Extend legal, provenance, size, ABI, and dependency-closure gates to the embedded guest.
4. Test current options and all admitted diagram/layout/math behavior through the guest.
5. Remove any temporary native companion after its announced transition window.

### Phase 4: add checked sidecars if users need a strict zero-dependency lane

1. Add the zero-dependency fixed/pair SVG macros in the separate normal-library package
   `merman-rustdoc-sidecar`; do not add them as a feature of the proc-macro host.
2. Add manifest-driven `render` and `check` commands to a documentation-focused binary, reusing the
   existing Merman render API rather than duplicating it.
3. Emit deterministic receipts and validate SVG with the same strict policy as the proc macro.
4. Add `cargo package --list` and unpacked-package tests that prove every referenced artifact ships.
5. Dogfood the path in Merman's own public Rustdoc.

### Phase 5: optional HTML postprocessor

Add this only after the embedded path or its sidecar fallback is stable. The command should:

1. invoke `cargo rustdoc` for one selected package/target;
2. transform only well-formed Rustdoc Mermaid code blocks;
3. preserve the original block or fail according to an explicit policy;
4. write atomically; and
5. test selectors against the project's MSRV, current stable, and docs.rs nightly Rustdoc outputs.

Rustdoc's HTML DOM is not a versioned public API. The postprocessor must fail visibly when the
expected shape changes; it must not perform a broad regex replacement across generated HTML.

### Phase 6: settle recommendations before stable

Make the embedded guest the README default only if it cleared the preregistered gates. Otherwise,
make checked sidecars the default and explain why the guest was rejected. Keep browser JavaScript
as an optional interoperability mode, not a fallback that silently changes security or
determinism.

## Acceptance criteria

The refactor is complete only when all of the following hold:

- If the embedded lane ships, its proc-macro host graph contains no `merman`, `merman-core`, or
  `merman-render` package.
- If the embedded lane ships, the exact guest and host clear the agreed closure, time, RSS, disk,
  package-size, and output-equivalence gates, and the actual `.crate` candidate remains below
  crates.io's current 10 MB limit.
- A sidecar consumer, if shipped, has no Merman package in its normal Cargo graph.
- If the embedded lane ships, docs.rs can render its fixture with network blocked and source
  directories read-only.
- If sidecars ship, docs.rs can render their fixture without a custom Cargo subcommand,
  `cargo merman-doc check` detects stale inputs or outputs, and generated SVG passes the same strict
  policy as the proc macro.
- Light/dark switching works without JavaScript and without duplicate DOM ids.
- If the embedded lane ships, it preserves inline fences, `include_mmd!`, failure policies, source
  details, theme behavior, and the three SVG pipelines.
- Closure, cold-build, warm-build, peak-memory, and artifact-size reports compare the old and new
  lanes without sharing target directories between cold samples.
- `cargo package` integration tests prove that sidecars and receipts used by Rustdoc are published.
- The sidecar package's `--all-features` graph remains renderer-free, and neither lightweight
  package depends on `merman-rustdoc-native`; documentation separately states that a
  consumer which adds and feature-gates `merman-rustdoc-native` will compile it in that consumer's
  own `--all-features` jobs.

## Risks and controls

| Risk | Control |
| --- | --- |
| Checked SVG becomes stale | Mandatory receipt plus `cargo merman-doc check` in CI and release preflight |
| Generated files create noisy reviews | Stable serialization, deterministic ids, one source/output mapping per manifest entry |
| Sidecars increase repository/package size | Measure light/dark pairs; permit fixed-theme single SVG where appropriate |
| Unsafe SVG is committed | Strict validation at generation and check time; fail closed by default |
| Tool and crate versions drift | Record exact tool/Merman/baseline in receipt and verify it |
| External tool is missing locally | Clear install command; checked files keep ordinary builds and docs.rs independent |
| Postprocessor breaks on Rustdoc HTML change | Narrow structured HTML parser, toolchain matrix, visible failure, sidecar fallback |
| Embedded guest is slow, memory-heavy, or makes the `.crate` exceed 10 MB | Preregistered benchmark and package gates, then a bounded sidecar fallback |
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

Proceed with the refactor before stable release. The current `merman-rustdoc` implementation is a
good high-fidelity integration built on the wrong default distribution boundary. Keep its renderer,
sanitization, deterministic themes, failure policy, and source-backed Mermaid behavior. Stop making
that renderer an implicit cost of the default Rustdoc helper.

The first strategic change should be a bounded embedded-guest spike, because it is the strongest
candidate for preserving the current one-command attribute experience, docs.rs compatibility,
and Merman's deterministic static contract while removing the native renderer from consumer Cargo
graphs. If the spike clears its measured gates, replace the current backend and delete the host
renderer features. If it does not, adopt checked SVG sidecars plus the separate zero-dependency
`merman-rustdoc-sidecar` package backed by an external `render/check` tool. Do not retain a heavy
feature edge or fall back silently to browser JavaScript.

This gives Merman a stronger position than Aquamarine rather than merely a heavier imitation of it:
the normal path remains deterministic, offline, JavaScript-free Rustdoc. If the embedded guest is
admitted, the expensive renderer is compiled once as a governed release artifact instead of once
per consuming workspace. If it is rejected, sidecar consumers receive governed static artifacts
while rendering remains in an explicit external tool.
