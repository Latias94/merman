# Theme path retirement artifact budget reassessment — 2026-10-10

## Decision and status

This historical reassessment replaced limits inherited from two different
products after measuring all six canonical artifact recipes at one committed
source and passing their package checks. All 24 replacement limits passed locally;
remote CI confirmed the catalog and recipes at `3a2a2de46` in run `38023696805`.
The measurements below apply to that source and toolchain, not current `main`.

Each raw, stripped, gzip and Brotli limit is the corresponding measured value
plus 3%, rounded upward to 1,000 bytes. Integer arithmetic implements it as
`((bytes * 103 + 99999) / 100000) * 1000`. All 24 metrics use this rule; smaller
calculated limits replace larger historical limits too. No extra host margin
is added, and this acceptance does not authorize automatic future rebaselines.

The margin is a regression allowance above today's measured product, not a
percentage estimate of the theme feature's implementation cost. That cost is
already in the new baseline. Observed local/CI Typst stripped-size variation is
about 0.1%, substantially below 3%; there is no measured reason for a larger
host allowance. Keeping the established margin provides room for small changes
while still flagging growth requiring attribution. The four metrics remain
independent: compressed transfer size cannot waive an uncompressed-size limit,
and passing any byte limit does not establish latency or memory acceptance.

## Fixed source and recipes

- Source: `dd79ca584de4bb8b0045f572302d2ba355ce1958`, in a clean independent clone.
- Toolchain: Rust 1.95.0, wasm-pack 0.15.0, Binaryen 131, wasm-tools 1.253.0.
- Typst package smoke: the CI-pinned Typst 0.15.0 (`3ae52774`).
- All five Web packages use their descriptor-owned build, TypeScript assembly,
  freshness, package and runtime-smoke commands.
- Typst uses the canonical publish feature set, artifact import validation,
  package assembly and example/positive/compile-fail fixture smoke.
- All six exact dependency closures were checked; final artifact hashes are listed below.
- The official owner measures final assembled Web WASM and canonical Typst
  post-link WASM. Rust gzip and Brotli compression remain unchanged.
- Typst retains Binaryen `-Oz` and its existing seven feature flags. Convergence
  and alternative optimization-level experiments did not change the recipe.
- Cargo and CPU measurement ran serially, using artifacts from the fixed source.

## Why the former Typst ceiling failed

The adjacent baseline already fits the stripped budget: 11,903,873 bytes against
11,950,000. The initial local refactor reproduction is 11,984,660 stripped bytes;
remote CI reports 11,972,759. These are a real product-size increase and a smaller
host/build difference, respectively; neither is a reproducible-build claim.
The refactor moves visual resolution into operation-owned prepared artifacts
while retaining diagnostics, winner provenance, resource policy and supported
diagram families. The same feature list does not mean the implementation has
the same responsibilities or code size as a historical release.

No package version changed and no new dependency explained this growth. Before
accepting it, the follow-up shared Class indexing, Sequence preparation, CSS
emission, Flowchart parsing and XML validation bodies. Its preliminary Typst
measurement reduced raw, stripped and gzip bytes by 62,067, 50,828 and 12,677,
respectively, relative to the initial reproduction, but still exceeded the old
gzip limit by 16,500 bytes. The final packaged measurements appear below.

Native latency acceptance remains separate. The long native confirmation at
the fixed source confirmed non-regression for eight of nine rows under the joint
10% and 50-microsecond gate. Label reuse remained A/A-unstable, leaving the suite
inconclusive (exit 3). No confirmed material regression or speedup was observed;
accepting size growth did not close that timing uncertainty.

## Accepted measurements and ceilings

All values below are bytes. Each ceiling is calculated from its own measured
metric; no profile inherits another profile's growth.

| Artifact | Raw | Stripped/post-link | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3,822,962 | 3,822,697 | 1,509,440 | 1,157,178 |
| web-ascii | 5,355,906 | 5,355,641 | 2,015,048 | 1,529,034 |
| web-editor | 3,935,758 | 3,935,493 | 1,554,938 | 1,187,784 |
| web-full | 16,365,767 | 16,365,502 | 6,124,133 | 4,528,848 |
| web-render | 14,462,153 | 14,461,888 | 5,471,666 | 4,050,124 |
| typst-wasm | 19,661,234 | 11,934,211 | 4,599,014 | 3,413,838 |

| Artifact | Raw ceiling: old -> new | Stripped: old -> new | Gzip: old -> new | Brotli: old -> new |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3,944,000 -> 3,938,000 | 3,944,000 -> 3,938,000 | 1,558,000 -> 1,555,000 | 1,195,000 -> 1,192,000 |
| web-ascii | 5,522,000 -> 5,517,000 | 5,522,000 -> 5,517,000 | 2,079,000 -> 2,076,000 | 1,579,000 -> 1,575,000 |
| web-editor | 4,060,000 -> 4,054,000 | 4,060,000 -> 4,054,000 | 1,604,000 -> 1,602,000 | 1,226,000 -> 1,224,000 |
| web-full | 16,804,000 -> 16,857,000 | 16,804,000 -> 16,857,000 | 6,282,000 -> 6,308,000 | 4,646,000 -> 4,665,000 |
| web-render | 14,843,000 -> 14,897,000 | 14,842,000 -> 14,896,000 | 5,606,000 -> 5,636,000 | 4,155,000 -> 4,172,000 |
| typst-wasm | 19,718,000 -> 20,252,000 | 11,950,000 -> 12,293,000 | 4,582,000 -> 4,737,000 | 3,425,000 -> 3,517,000 |

Twelve limits decrease and twelve increase. Actual headroom is 3.000% to
3.050%, including upward rounding. All five assembled Web packages pass input
freshness, TypeScript exports, package contracts, runtime and DOM safety smoke.
All six dependency closures pass. Typst minimal/complete Rust tests pass 14/34
cases; the publish package passes 23 compilation fixtures and nine expected
failures with Typst 0.15.0. The owner-generated stripped Typst SHA-256 equals
the package WASM SHA-256; matching sizes alone were not used as identity proof.
The official second measurement passes all 24 replacement limits with identical
metrics and input/stripped hashes. No recipe or production capability changed.

| Final package | WASM SHA-256 |
| --- | --- |
| web-analysis | `23b41ba38354aa5651fa65022874c98f819d2e793f74291d15f0d8c255deed20` |
| web-ascii | `0d22c9b55e9328c10500611efca272b0c140dfc902178f143b3d7020038652fc` |
| web-editor | `b44b40ea5dec1671754ef257895e060f1acd7c9cd8ea1500d35725157d055b98` |
| web-full | `5c2a86d586f4de4ea80325829717577f4dafa68ca23e4fb22efeaa8521a5df81` |
| web-render | `9c3f7bb11b2d48da27eaa061b131e89fccae2e368257f3c3b4d3c919b5a87cbf` |
| typst-wasm | `b4bbc5261f4ab0af38302b136d579c2c13c22e1c2f24b66e521be1b85c43c477` |

The host is macOS ARM64, Node 26.11.0 and npm 11.20.0 for Web assembly. The
measurement executable was built from the fixed source and privately copied
before use; SHA-256 is
`572695fc7a035f314c436cb94c6dd1cdaca86a8ada4cba838c620133828b6148`.
The source clone stayed clean throughout measurement. Different checkout paths
or hosts may produce slightly different bytes; this is not a reproducible-build
claim, and the registered margin remains unchanged.

## Evidence availability and current authority

The tables and hashes above preserve the dated measurement summary. Detailed
package receipts and the complete local latency report are no longer retained in
this repository; this page is not an independently replayable raw dataset.
The recorded functional checks included public-facade and native-export tests,
SVG structure/parity/parity-root comparison, Playground build/typecheck and
Chromium theme/viewport tests. The earlier whole-workspace and 45-build feature
matrix was not repeated at the shared implementation commit.

Current ceilings belong to [WASM_SIZE_BUDGETS.json](../release/WASM_SIZE_BUDGETS.json).
The [October 25 reassessment](semantic_wasm_budget_2026-10-25.md) records a later
product baseline under the same 3% rule. Neither size acceptance substitutes for
functional, latency, memory or platform evidence.
