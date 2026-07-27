# Alpha.3 to Alpha.4 Refactoring Report

## Scope and verdict

This report compares `v0.8.0-alpha.3` (`56227a541011a3929b808bb3555d67372d630aae`)
with `d2698d0a365b905bb65a58a7690c74075878a4f9`, the current alpha.4 candidate.
It was measured on an Apple M4 Pro with Rust 1.95.0, Cargo 1.95.0, Node 26.5.0, and
the same local source corpus on 2026-07-27.

The refactor succeeds at changing the cost model: users can now choose explicit capability
leaves and artifact profiles instead of inheriting the historical full stack. The clearest
result is lint/analysis CLI work, whose measured binary falls by 67.95% and whose normal
dependency closure falls by 63.06%.

It is not a universal size or performance reduction. Complete products now carry the Mermaid
11.16 contract and, depending on the surface, both layout engines and math, so their total
artifacts are larger. A pre-release benchmark pass also exposed a recursive clone of the full
effective Mermaid configuration on every default/zero-seed SVG render. Commit `d2698d0a3`
removed that fixed cost without changing hand-drawn seed semantics. After the repair, the
minimal same-capability pipeline has a 1.12x median and 1.07x geometric-mean current/alpha.3
ratio across 32 fixtures: 10 are faster, 22 are slower, and seven are within 5%. That is a
substantial recovery, but not a universal native performance win. Remaining family-local
regressions are recorded in
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
| Minimal native SVG end-to-end | baseline | median 1.12x alpha.3 | The shared configuration-clone regression is fixed; 10 of 32 fixtures are faster and seven are within 5%, with family-local hotspots still visible. |

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
Merman at `d2698d0a3`, `mermaid-rs-renderer` at `7ff1196`, and Mermaid.js 11.16.0 in one warm
Headless Chromium 131 process.

| Runner | Requested | Measured | Missing | Result |
| --- | ---: | ---: | ---: | --- |
| Merman native | 34 | 34 | 0 | Complete suite coverage. |
| Mermaid.js browser | 34 | 34 | 0 | Complete suite coverage. |
| mermaid-rs-renderer native | 34 | 32 | 2 | Missing `flowchart_large` and `info_medium`. |

On this host, Merman's median `Merman / Mermaid.js` warm end-to-end ratio was 0.0237 across all
34 rows: Mermaid.js's median latency was approximately 42.2x Merman's. This is a native Rust
pipeline compared with a warm browser renderer, not an intrinsic language benchmark or a
browser-WASM claim.

Against the 32 shared `mermaid-rs-renderer` rows, Merman was faster on 19 and slower on 13; the
median `Merman / mmdr` ratio was 0.697. The result is workload-dependent:

| Fixture | Merman | mermaid-rs-renderer | Mermaid.js |
| --- | ---: | ---: | ---: |
| `flowchart_medium` | 3.68 ms | 105.62 ms | 57.65 ms |
| `flowchart_ports_heavy` | 1.13 ms | 1.28 s | 32.30 ms |
| `class_medium` | 949.54 us | 2.38 ms | 44.70 ms |
| `mindmap_medium` | 685.48 us | 76.79 us | 15.40 ms |
| `kanban_medium` | 153.33 us | 29.38 us | 6.20 ms |

Merman is particularly strong on the measured complex Flowchart cases and wins the median native
comparison, while `mermaid-rs-renderer` remains much faster on Mindmap and Kanban. The harness
confirms successful execution and aligned fixture selection; it does not claim byte, DOM, or
Mermaid-semantic equivalence for `mermaid-rs-renderer`. These ratios measure latency for each
implementation's output, not a quality-adjusted winner. That distinction matters more than a
single geometric mean.

### Alpha.3 to alpha.4 native pipeline

The 34 `standard` fixture files are byte-for-byte unchanged across the range. The comparison used
two lanes so product-default cost is not confused with an implementation-only delta:

| Revision A/B lane | Shared rows | Median current / alpha.3 | Geometric mean | Current faster / slower |
| --- | ---: | ---: | ---: | ---: |
| Revision-complete SVG product | 34 | 1.10x | 1.09x | 13 / 21 |
| Minimal same-capability SVG | 32 | 1.12x | 1.07x | 10 / 22 |

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
| Rows within 5% | 7 |

The remaining delta is not uniform. State medium improved from 1.12 ms to 501.4 us end-to-end,
while large and medium Flowchart cases remain within about 1.7%. The largest minimal-lane
regressions are Kanban (4.08x), Requirement (2.25x), Info (1.99x), Packet (1.93x), and Radar
(1.31x). The complete-product lane additionally exposes a 4.78x Mindmap regression.

An additional focused run used 30 samples, a one-second warm-up, and two-second measurement
windows to verify the repaired small-diagram path:

| Stage | Info medium: alpha.3 -> current | Packet medium: alpha.3 -> current |
| --- | ---: | ---: |
| SVG emit | 1.68 us -> 2.37 us (1.41x) | 2.28 us -> 3.17 us (1.39x) |
| End-to-end | 2.45 us -> 4.84 us (1.97x) | 3.60 us -> 6.87 us (1.91x) |

Criterion's `iter_batched` excludes the `family::prepare` setup from the timed render routine.
The emit measurements therefore isolate SVG emission/finalization. Packet output is
byte-identical between revisions; Info differs only by the `v11.15.0` to `v11.16.0` text.

#### Fixed Info and Packet root cause

The pre-fix shared SVG dispatch called `SvgExecution::effective_config` before selecting a
diagram family. Mermaid's generated default configuration sets `handDrawnSeed` to `0`, which
means "derive a seed for this operation." Resolving that sentinel cloned the full top-level
`serde_json::Map`, inserted the derived seed, and rebuilt a `MermaidConfig`. The measured default
config is 15,409 bytes, with 49 top-level keys and 461 scalar values. Info and Packet do not
consume the seed, but every default render paid for the recursive clone and its drop.

The path was introduced in commit `84477e467` (`refactor(render): own operation render
environment`). Before committing the repair, a same-worktree A/B based on the unfixed parent
`1bd6f9b90` and the eventual `d2698d0a3` patch used 30 samples, a one-second warm-up, and
two-second measurement windows:

```console
cargo bench --locked -p merman --no-default-features --features svg --bench pipeline -- \
  'render/(info_medium|packet_medium)|end_to_end/(info_medium|packet_medium)' \
  --noplot --sample-size 30 --warm-up-time 1 --measurement-time 2
```

| Benchmark | Unfixed candidate | Formal fix | Improvement |
| --- | ---: | ---: | ---: |
| `render/info_medium` | 45.28 us | 2.36 us | 19.2x lower latency |
| `render/packet_medium` | 45.48 us | 3.01 us | 15.1x lower latency |
| `end_to_end/info_medium` | 47.38 us | 4.26 us | 11.1x lower latency |
| `end_to_end/packet_medium` | 49.60 us | 6.22 us | 8.0x lower latency |

Commit `d2698d0a3` makes shared dispatch borrow the effective configuration directly and resolves
only the `0`/`-0` sentinel inside `SvgExecution::rough_randomness`. Explicit non-zero negative,
fractional, and large-number JavaScript seed semantics remain unchanged, as does the
domain-separated operation stream. Every production RoughJS/hand-drawn randomness consumer uses
that central method, so the repair needs no diagram-family allow-list.

Focused validation after the change covered:

- `cargo fmt --all -- --check`.
- `merman-render` library tests: 796 passed.
- Hand-drawn seed, State, Ishikawa, and Venn integration tests: 32 passed.
- Architecture integration tests with Cytoscape: 12 passed and one skipped.
- Runtime determinism tests: two passed.

The full workspace `nextest`, SVG DOM parity, and complete golden gates were not rerun for this
focused repair. The final clean alpha.3 comparison above shows that the catastrophic fixed cost
is gone; the remaining roughly 1.9x end-to-end delta on these ultra-small fixtures is a separate,
much smaller profiling target.

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

1. Profile the remaining family-local regressions: Mindmap, Kanban, and Requirement are the
   clearest suite outliers. Treat the residual Info/Packet parse, layout, and session fixed costs
   as a separate small-diagram target rather than reopening the repaired seed path.
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
- Removed the default/zero-seed full-configuration clone while preserving deterministic
  hand-drawn seeds; see this report for the final native SVG comparison and private Node
  candidate limitations.
