# Alpha.3 to Alpha.4 Refactoring Report

## Scope and verdict

This report compares `v0.8.0-alpha.3` (`56227a541011a3929b808bb3555d67372d630aae`)
with `a7026ea2da2ce2cb156e87b40d788fd91779c314`, the current alpha.4 candidate.
It was measured on an Apple M4 Pro with Rust 1.95.0, Cargo 1.95.0, Node 26.5.0, and
the same local source corpus on 2026-07-27.

The refactor succeeds at changing the cost model: users can now choose explicit capability
leaves and artifact profiles instead of inheriting the historical full stack. The clearest
result is lint/analysis CLI work, whose measured binary falls by 67.95% and whose normal
dependency closure falls by 63.06%.

It is not a universal size or performance reduction. Complete products now carry the Mermaid
11.16 contract and, depending on the surface, both layout engines and math, so their total
artifacts are larger. More importantly, the native minimal-SVG pipeline regressed on the
unchanged standard corpus. The median current/alpha.3 end-to-end ratio is 1.93x across 32 shared
fixtures, with a large fixed-cost SVG emit regression for Info and Packet. An isolated A/B traced
most of that cost to a shared deep clone of the effective Mermaid configuration when the default
`handDrawnSeed` is resolved. The target revision still contains the regression; the root cause
and safe repair boundary are recorded in
[`FEARLESS_REFACTORING.md`](../performance/FEARLESS_REFACTORING.md).

## At a glance

| Surface | Alpha.3 | Alpha.4 candidate | User consequence |
| --- | ---: | ---: | --- |
| Primary SVG admission records | 27 | 35 | Eight additional records across five logical family groups, with source-backed semantic, layout, SVG, and compare evidence. |
| CLI default binary | 32,194,272 bytes | 36,925,360 bytes | 14.70% larger. The product contract also changed, so this is not a same-capability implementation comparison and the size alone does not attribute a cause. |
| CLI default normal dependency closure | 387 package identities | 377 package identities | Ten fewer resolved normal dependencies despite the broader default capability contract. |
| CLI lint/analysis binary | 25,477,648 bytes | 8,166,352 bytes | 67.95% smaller for lint/CI workloads. |
| CLI lint normal dependency closure | 333 package identities | 123 package identities | 210 fewer resolved normal dependencies, or 63.06%. |
| Browser full WASM | 6,911,512 bytes | 12,339,868 bytes | Not like-for-like: current full adds Cytoscape, math, and the expanded Mermaid 11.16 contract. |
| Browser analysis WASM | 1,914,582 bytes | 3,373,026 bytes | 76.18% larger source rebuild; the analysis capability now covers the expanded semantic baseline. |
| Current complete browser renderer | not comparable | 11,665,436 bytes | `@mermanjs/web-render` removes analysis, editor, and ASCII APIs while retaining SVG, Cytoscape, ELK, and math. |
| Minimal native SVG end-to-end | baseline | median 1.93x alpha.3 | Regression remains in the candidate; a diagnostic no-clone A/B removed most of the Info/Packet fixed cost. |

The CLI sizes are unstripped macOS arm64 Cargo release executables. The WASM values are
`wasm-pack 0.15.0` optimized package artifacts rebuilt from source where historical output was
required, not registry tarballs. Gzip uses `gzip -n -9`; Brotli uses `brotli -q 11 -c`.

## What users gain

### Capability-oriented selection

Alpha.3 exposed broad implementation-era feature names such as `core-full`, `core-host`,
`render`, `raster`, `cytoscape-layout`, `elk-layout`, and `ratex-math`. Alpha.4 exposes
observable leaves instead:

| User-visible capability | Additive leaf | Cost that remains optional |
| --- | --- | --- |
| SVG rendering | `svg` | Analysis, editor, ASCII, exports, layouts, and math are not implied. |
| Mermaid-compatible layouts | `layout-cytoscape`, `layout-elk` | Each engine is independent; selecting one does not require the other. |
| Mathematics | `math` | Plain SVG does not pull the RaTeX and embedded-font closure. |
| Bitmap/PDF export | `png`, `jpeg`, `pdf` | Each format is independent instead of one aggregate pulling every exporter. |
| Diagnostics | `analysis` | Analysis does not imply renderer, layout, math, export, icon, or Markdown code. |
| Editor intelligence | `editor` | Editor facts imply analysis, but not SVG or export code. |
| CLI icons and Markdown | `icons`, `markdown`; opt into `network-icons`, `parallel-markdown` | Local icons and serial Markdown do not imply Reqwest/TLS or Rayon. |

The ergonomic Rust facade keeps `default = ["complete-svg"]`. Lower-level crates, bindings,
and transport layers intentionally have empty defaults, so embeddings can state their required
capabilities rather than inheriting a renderer by accident.

These are additive Cargo features. A lean source build must set `default-features = false` (or
use `--no-default-features`) and ensure another dependency does not re-enable a leaf through
feature unification. Only an exact artifact profile can make a release-level absence claim.

The public migration is mechanical where an old aggregate has a direct replacement:

| Alpha.3 feature | Alpha.4 selection |
| --- | --- |
| `render` | `svg` |
| `cytoscape-layout` | `layout-cytoscape` |
| `elk-layout` | `layout-elk` |
| `ratex-math` | `math` |
| `raster` | Select `png`, `jpeg`, and/or `pdf` independently. |
| `core-full` | No direct replacement: core language semantics are invariant; select only the required output capabilities. |
| `core-host` | Select `system-clock`, `system-timezone`, `system-random`, and/or `system-timing` for the host behavior actually required. |
| Historical `full`/`tiny` profile aliases | Use an exact artifact profile, or disable Cargo defaults and select observable leaves. |

The old Cargo names have been removed; there are no compatibility aliases. Browser consumers
must also replace historical `@mermanjs/web/<subpath>` or raw `pkg/**` imports with a standalone
public package. There is no subpath or raw-WASM fallback, and the current `@mermanjs/web-render`
is the complete SVG/layout/math product rather than a name-only replacement for an older basic
render profile.

At the manifest level, `merman` moved raster implementation dependencies behind
`merman-export`: its direct dependency count changed from 11 to 9 while its public capability
vocabulary changed from 9 features to 16. `merman-cli` still has 10 direct declarations, but
four formerly unconditional dependencies are now optional: analysis, Rayon, Reqwest, and shell
completion support.

### Diagram coverage and parity

The authoritative admission inventory, rather than the older human status table, changed from
27 to 35 primary SVG records:

| New primary admission or promotion | User-visible effect |
| --- | --- |
| `error` | Error diagrams moved from parse-only tracking to primary SVG admission. |
| `swimlane` | Swimlane diagrams have typed layout and SVG evidence. |
| `railroad`, `railroadEbnf`, `railroadAbnf`, `railroadPeg` | Railroad syntax and the three grammar dialects are independently admitted. |
| `wardley` | Wardley mapping joins the source-backed primary matrix. |
| `cynefin` | Cynefin joins the source-backed primary matrix. |

`zenuml` remains compatibility-only because its upstream renderer is an external browser plugin;
it must not be described as an upstream SVG parity claim.

## Size and dependency evidence

### CLI profiles

| Measured build | Alpha.3 | Alpha.4 candidate | Interpretation |
| --- | ---: | ---: | --- |
| `cargo build --release -p merman-cli` | 32,194,272 | 36,925,360 | Default product grows 4,731,088 bytes while its capability contract also changes; this measurement does not isolate which additions account for those bytes. |
| Unique normal `cargo tree` identities for default | 387 | 377 | The resolved package count falls by ten even though the linked capability set grows. |
| Lean lint build | 25,477,648 | 8,166,352 | Alpha.3 `--no-default-features` still carried historical renderer/export/tool dependencies; current `--no-default-features --features analysis` does not. |
| Unique normal `cargo tree` identities for lean lint | 333 | 123 | Measured from `cargo tree --locked --edges normal --prefix none --format '{p}' | sort -u`. |

The complete CLI is the right choice for `mmdc`-style conversion, export, icons, and Markdown
work. The analysis profile is the right choice for a CI lint gate, where the old implicit full
closure was pure cost.

### Browser WASM profiles

| Artifact | Raw bytes | Gzip bytes | Brotli bytes | Contract |
| --- | ---: | ---: | ---: | --- |
| Alpha.3 `browser-full` | 6,911,512 | 2,641,193 | 1,946,384 | Historical full package; lacks the current complete SVG contract. |
| Alpha.4 `@mermanjs/web` | 12,339,868 | 4,648,558 | 3,359,651 | Analysis, ASCII, editor, SVG, Cytoscape, ELK, and math. |
| Alpha.3 analysis | 1,914,582 | 718,044 | 546,783 | Historical semantic-analysis profile. |
| Alpha.4 `@mermanjs/web-analysis` | 3,373,026 | 1,270,087 | 970,361 | Browser diagnostics and semantic analysis only. |
| Alpha.4 `@mermanjs/web-render` | 11,665,436 | 4,385,258 | 3,180,859 | Complete public SVG renderer: SVG plus Cytoscape, ELK, and math. |
| Alpha.4 `@mermanjs/web-editor` | 3,519,126 | 1,327,314 | 1,010,768 | Parser-backed editor intelligence plus analysis. |
| Alpha.4 `@mermanjs/web-ascii` | 3,518,439 | 1,332,638 | 1,019,437 | ASCII output only. |

The current renderer saves 674,432 raw bytes (5.47%) and 178,792 Brotli bytes (5.32%) against
current full while retaining the complete SVG contract. This is a useful split for viewer-only
browser applications, but it is intentionally not presented as a dramatic size win.

The alpha.3-to-alpha.4 full and analysis rows are valuable cost evidence, not a like-for-like
artifact-size regression gate. Mermaid 11.16 coverage and the current capability contracts changed
what those packages promise.

## Performance evidence

### Current Merman, Mermaid.js, and mermaid-rs-renderer

The checked-in comparison harness ran the `standard` suite: 34 end-to-end fixtures across 24
families, 20 Criterion samples, one-second warm-up, and one-second measurement windows. It used
Merman at `a7026ea`, `mermaid-rs-renderer` at `7ff1196`, and Mermaid.js 11.16.0 in one warm
Headless Chromium 131 process.

| Runner | Requested | Measured | Missing | Result |
| --- | ---: | ---: | ---: | --- |
| Merman native | 34 | 34 | 0 | Complete suite coverage. |
| Mermaid.js browser | 34 | 34 | 0 | Complete suite coverage. |
| mermaid-rs-renderer native | 34 | 32 | 2 | Missing `flowchart_large` and `info_medium`. |

On this host, Merman's median `Merman / Mermaid.js` warm end-to-end ratio was 0.0388 across all
34 rows: approximately 25.7x lower latency. This is a native Rust pipeline compared with a warm
browser renderer, not an intrinsic language benchmark or a browser-WASM claim.

Against the 32 shared `mermaid-rs-renderer` rows, Merman was faster on 11 and slower on 21; the
median `Merman / mmdr` ratio was 1.85. The result is workload-dependent:

| Fixture | Merman | mermaid-rs-renderer | Mermaid.js |
| --- | ---: | ---: | ---: |
| `flowchart_medium` | 3.81 ms | 106.65 ms | 57.80 ms |
| `flowchart_ports_heavy` | 1.20 ms | 1.28 s | 33.10 ms |
| `class_medium` | 1.01 ms | 2.39 ms | 45.10 ms |
| `mindmap_medium` | 719.40 us | 77.07 us | 15.50 ms |
| `kanban_medium` | 199.10 us | 28.88 us | 6.30 ms |

Merman is particularly strong on the measured complex Flowchart cases, but it does not yet win
the broad native comparison. The harness confirms successful execution and aligned fixture
selection; it does not claim byte, DOM, or Mermaid-semantic equivalence for
`mermaid-rs-renderer`. These ratios measure latency for each implementation's output, not a
quality-adjusted winner. That distinction matters more than a single geometric mean.

### Alpha.3 to alpha.4 native pipeline

The 34 `standard` fixture files are byte-for-byte unchanged across the range. The comparison used
two lanes so product-default cost is not confused with an implementation-only delta:

| Revision A/B lane | Shared rows | Median current / alpha.3 | Geometric mean | Current faster / slower |
| --- | ---: | ---: | ---: | ---: |
| Revision-complete SVG product | 34 | 1.84x | 1.95x | 7 / 27 |
| Minimal same-capability SVG | 32 | 1.93x | 2.12x | 4 / 28 |

The complete lane enables each revision's documented full SVG/layout/math product and therefore
includes deliberate capability changes. The minimal lane disables defaults and selects only the
smallest equivalent SVG leaf. Mindmap and Architecture require optional layouts and are
unavailable in both minimal builds, leaving 32 shared rows. The minimal lane used:

```console
cargo bench --locked -p merman --no-default-features \
  --features <render-or-svg> --bench pipeline -- \
  --noplot --sample-size 20 --warm-up-time 1 --measurement-time 1
```

| Metric | Result |
| --- | --- |
| Shared minimal-SVG rows | 32 |
| Rows within 5% | 3 |

The regression is not uniform. State medium improved from 1.13 ms to 555.1 us end-to-end, while
large and medium Flowchart cases remain within about 3.5%. The highest-priority fixed-cost
regressions are:

| Stage | Info medium: alpha.3 -> current | Packet medium: alpha.3 -> current |
| --- | ---: | ---: |
| Parse | 0.5 us -> 0.7 us | 0.9 us -> 1.8 us |
| Layout | 0.1 us -> 0.4 us | 0.2 us -> 0.6 us |
| SVG emit | 1.7 us -> 43.7 us | 2.2 us -> 44.4 us |
| End-to-end | 2.4 us -> 52.1 us | 3.6 us -> 54.3 us |

Criterion's `iter_batched` excludes the `family::prepare` setup from the timed render routine.
The emit measurements therefore isolate SVG emission/finalization. Packet output is byte-identical
between revisions; Info differs only by the `v11.15.0` to `v11.16.0` text. The issue is an
implementation cost regression, not an output-contract expansion.

#### Confirmed Info and Packet root cause

The shared SVG dispatch calls `SvgExecution::effective_config` before selecting a diagram
family. Mermaid's generated default configuration sets `handDrawnSeed` to `0`, which means
"derive a seed for this operation." The current implementation resolves that sentinel by cloning
the full top-level `serde_json::Map`, inserting the derived seed, and rebuilding a
`MermaidConfig`. The measured default config is 15,409 bytes, with 49 top-level keys and 461
scalar values. Info and Packet do not consume that seed, but every default render still pays for
the recursive clone and its drop. Alpha.3 passed the effective configuration by reference.

The path was introduced in commit `84477e467` (`refactor(render): own operation render
environment`). A detached-worktree experiment changed only the zero-seed path to borrow the
configuration. That intentionally incomplete diagnostic produced:

| Benchmark | Candidate | Diagnostic no-clone branch | Candidate / diagnostic |
| --- | ---: | ---: | ---: |
| `render/info_medium` | 43.7 us | 2.44 us | 17.9x |
| `render/packet_medium` | 44.4 us | 3.08 us | 14.4x |
| `end_to_end/info_medium` | 52.1 us | 4.35 us | 12.0x |
| `end_to_end/packet_medium` | 54.3 us | 6.29 us | 8.6x |

This A/B confirms causality but is not a valid production patch: treating zero as an ordinary
borrowed value would discard the operation-derived seed required by deterministic hand-drawn
rendering. The durable fix is to keep the resolved seed in `SvgExecution` and expose it directly
to the renderers that use randomness, or provide a borrowed configuration overlay. Either design
must preserve explicit non-zero seeds and the runtime determinism tests without materializing the
entire configuration for diagrams that never inspect the seed.

## Node candidate evidence

`@mermanjs/node` remains a private, inconclusive candidate. It is not a release recommendation
yet. The current official locked candidate build fails because
`crates/merman-node/Cargo.lock` still records local alpha.3 package versions. The following
runtime evidence was generated only in an isolated temporary worktree after regenerating that
lockfile offline; it demonstrates behavior but does not repair reproducibility.

| Candidate, macOS arm64 | Node-WASM | N-API |
| --- | ---: | ---: |
| Runtime artifact | 16,867,323 bytes | 21,289,616 bytes |
| Packed install | 6,145,608 bytes | 8,987,260 bytes |
| Warm SVG p50, 4,001-case corpus | 0.4085 ms | 0.3569 ms |
| Warm SVG p95 | 1.8691 ms | 1.5703 ms |
| Cold process p50 | 89.67 ms | 50.54 ms |
| Peak RSS | 602.21 MB | 236.04 MB |
| Concurrent batch p50 | 3.0083 ms | 2.0194 ms |

N-API is faster and uses less peak RSS on this host; Node-WASM is the smaller artifact and
install. The official SVG harness found matching success/error outcomes for all 4,001 corpus
cases, but 426 cross-transport geometry digests differ. A single macOS arm64 result also cannot
satisfy the all-target admission rule.

A separate `semantic-json` probe gives a cleaner parse-only comparison:

| Parse-only measurement | Node-WASM | N-API |
| --- | ---: | ---: |
| Shared accepted cases | 3,902 | 3,902 |
| Shared rejected cases | 99 | 99 |
| Canonical semantic JSON matches | 3,902 / 3,902 | 3,902 / 3,902 |
| Warm p50 | 0.2058 ms | 0.1924 ms |
| Warm p95 | 0.4770 ms | 0.4515 ms |
| Throughput | 5,260 cases/s | 5,430 cases/s |

Raw semantic JSON has nondeterministic object-key order across transports. After recursively
sorting object keys, every shared successful result matches; raw byte comparison must not be used
as semantic parity evidence.

## Scenario guide

| User workflow | Recommended surface | Selection | Why |
| --- | --- | --- | --- |
| Static Rust documentation or a blog generator | `merman` | Default `complete-svg`, or `default-features = false` with `svg,layout-cytoscape,layout-elk,math` | Complete deterministic SVG without CLI/tooling dependencies. |
| CLI conversion and broad export | `merman-cli` | Release/default artifact profile | SVG, ASCII, PNG, JPEG, PDF, icons, Markdown, and math are available. |
| CLI lint or CI | `merman-cli` | `--no-default-features --features analysis` | Measured lean executable with no renderer, exporter, icon, network, or Markdown closure. |
| Embedded lint library | `merman-analysis` | Depend on the crate directly | Analysis APIs do not require a nonexistent `analysis` feature on this crate. |
| Browser diagram viewer | `@mermanjs/web-render` | Standalone render package | Complete SVG layout and math without analysis/editor/ASCII APIs. |
| Browser diagnostics or lint | `@mermanjs/web-analysis` | Standalone analysis package | Detection, validation, facts, and diagnostics without rendering or editor APIs. |
| Browser editor | `@mermanjs/web-editor`, or `@mermanjs/web` when one realm also renders | The editor package contains analysis plus parser-backed editor intelligence | Use `web-editor` for a dedicated Worker; do not describe it as editor-only or combine full with duplicate slim packages. |
| Native editor integration | `merman` | `default-features = false, features = ["analysis", "editor"]` | Parser-backed diagnostics and editor facts without linking SVG/export code. |
| LSP process | `merman-lsp` | Published binary after release, or `--no-default-features --features stdio` | The LSP executable requires the `stdio` leaf; use artifact profile `lsp-stdio-release` for the exact release contract. |
| Markdown or MDX conversion | `merman-cli` | `--no-default-features --features svg,markdown`; add `parallel-markdown` only after throughput measurement | Avoid Rayon unless batch conversion needs it. |
| Terminal preview | `merman` or `merman-cli` | Disable defaults and select `ascii`; query `ascii_capabilities` | No SVG backend is required, but only 14 families are admitted and support is graded Full, Partial, or Summary. SVG admission does not imply ASCII support. |
| Node SSR or a Node static-site generator | Current CLI subprocess | Do not depend on private `@mermanjs/node` yet | There is no admitted in-process Node package; the candidate still lacks reproducible all-target admission. |
| Typst | `@preview/merman:0.2.0` | Published Typst package on its independent version track | The current package embeds Merman alpha.3 and its `typst-wasm` profile has no math; it is not an alpha.4 artifact. |
| Python or Flutter embedding after alpha.4 publishes | `merman` on PyPI or pub.dev | Planned full ABI 3 package; verify the installed version before use | The target contract has no slim prebuilt SKU; a declared channel is not proof that this candidate is already live. |
| Android or Apple embedding after alpha.4 publishes | GitHub Release AAR or XCFramework | Planned full ABI 3 artifact-only output | Maven Central and a remote SwiftPM binary are not published channels; verify the release asset version. |
| C ABI embedding after alpha.4 publishes | `merman-ffi` from crates.io | Build the source crate and verify its version | Its artifact profile proves reproducible reference libraries; it is not a downloadable prebuilt SDK. |

## Risks and next work

1. Remove the per-render effective-configuration clone while preserving operation-derived
   hand-drawn seeds before presenting alpha.4 as a native performance release. The confirmed
   cause, diagnostic A/B, and validation gate are in the fearless-refactoring backlog.
2. Regenerate and commit the Node nested lockfile, then run the official locked harness on every
   declared target. Investigate the 426 geometry digest differences before admission.
3. Treat browser full and analysis growth as an explicit release cost. A future size win requires
   a same-capability baseline, not comparison with the historical smaller contract.
4. Keep the three-runner benchmark honest: it compares native Merman and mmdr with browser
   Mermaid.js. Browser-WASM needs its own same-host, same-browser harness before making Web
   throughput claims.

## Changelog extraction

- Added eight primary SVG admission records across five logical groups: Error, Swimlane,
  Railroad dialects, Wardley, and Cynefin on the Mermaid 11.16 baseline.
- Replaced broad feature aggregates with explicit capability leaves for rendering, layouts, math,
  exports, diagnostics, editor support, icons, and Markdown.
- Added separate browser packages for complete rendering, analysis, editor intelligence, and
  ASCII output.
- Reduced the measured lint/analysis CLI binary by 67.95% and its normal dependency closure by
  63.06% compared with alpha.3's historical lean build.
- Added capability discovery and artifact-profile contracts for safer native and browser embedding.
- See this report for current native SVG performance and private Node candidate limitations.
