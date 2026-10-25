# Semantic boundary WASM budget reassessment

## Decision

The maintainer authorized a budget adjustment when the implementation and its size growth are
reasonable. Retain the semantic/presentation boundary and reassess all six canonical artifacts
with the existing rule: measured bytes plus 3%, rounded upward to 1,000 bytes. Apply the rule
independently to raw, stripped, gzip, and Brotli bytes:

```text
((bytes * 103 + 99999) // 100000) * 1000
```

This raises 16 ceilings and lowers eight. The allowance remains 3%; there is no additional
platform margin, cross-profile offset, compression change, or automatic future rebaseline.
The [previous reassessment](theme_artifact_budget_reassessment_2026-10-10.md) defines the same
policy. This decision supersedes the requirement to fit the historical ceilings in the
[rejected local size trial](semantic_wasm_ci_2026-10-24.md); that trial remains rejected and
its source is not restored.

This is artifact-size acceptance, not latency or memory acceptance. It does not resume R14,
pool historical timing cohorts, or complete U1-U9.

## Fixed source and CI evidence

- PR head: `1a9e8b3777ce155761e444bbd4496460b64a0917`.
- Adjacent main: `83e3ea74bf4da30089a8012ec2d3418d40638e52`.
- Head CI run: `40113172363`; adjacent-main CI run: `39747740429`.
- Cargo.lock SHA-256: `458ddf1be6973de583d2c0f694538f7642d39debe973130930de4c74a7b58370`.
- Linux Rust 1.95.0 / LLVM 22.1.2, Node 24.21.0, wasm-pack 0.15.0, wasm-tools 1.261.0;
  Typst uses Binaryen 133 with the existing `-Oz` recipe.
- Head Web artifact: `pages-web-package-1`, artifact ID `12567787759`.
- Main Web artifact: `pages-web-package-1`, artifact ID `12413188400`.

The follow-up changes only workflows, their Python contract, budgets, and documentation; the
measured Rust implementation, Cargo.lock, and artifact feature descriptor remain unchanged.

Both complete Web artifact archives were downloaded. Every head package's archived file length
and SHA-256 matches its committed-format package provenance; the final assembled WASM, not a
raw Cargo intermediate or an old local package, supplies the size measurements. The existing
`xtask wasm-size-matrix` owner measures those bytes again with its own strip and compression
implementation. The three unchanged-profile main controls use the same owner.

The CI head's SVG structure/parity/parity-root owner, Web build/package/runtime smoke,
Windows/macOS build-test lanes, Node-WASM/N-API, and native binding owners pass. The Linux
build-test job reaches a later theme-consumer recipe failure: `render_operation` requires
`compat-json`, omitted by that recipe. The focused repair adds it explicitly in CI and release
preflight without adding it to default features or changing the selected tests.

The original head run remains failed. A replacement budget passing locally does not make that
historical run green, and the updated source needs its own CI results.

## Why the growth is accepted

Analysis, ASCII, and editor retain their existing Web public operation contract. In particular,
`parseJson` still exposes `semantic-json`; its compatibility projection deliberately uses
`MermaidEngine` and requires `compat-json`/presentation. The same products now also execute the
independent `SemanticParser` path for default analysis or terminal requests. Supporting both
contracts in one Web artifact can increase code size even though a semantic request avoids
presentation work.

The owning sources are `crates/merman-wasm/src/lib.rs`,
`crates/merman-bindings-core/src/engine.rs`, `crates/merman-analysis/src/analyzer.rs`,
`crates/merman/src/render.rs`, and the family terminal views under `crates/merman-ascii/src`.
The distinction is specified in [ADR-0098](../adr/0098-semantic-presentation-capabilities.md).
Canonical construction, requested recovery facts, terminal normalization, resource policy, and
full-processing errors remain explicit responsibilities; none is removed to make the size gate fit.

There is no new heavy external dependency behind the three smaller Web profiles. Their checked-in
license-report inventories change from 103 to 102 for analysis/editor and 100 to 98 for ASCII,
removing `euclid` and, for ASCII, `num-traits`. Their artifact recipe digests do not change.
The exact source-to-binary contribution of each internal refactor is not isolated by this review.

The archived binaries provide a narrower attribution: analysis/editor gain about 95 KB of code
and 1.2 KB of data; ASCII gains about 196 KB of code and 4.7 KB of data. Export-section bytes are
unchanged for those three profiles. This supports an internal reachable-code explanation rather
than a large new data payload or expanded Web API. It does not assign exact bytes to particular
Rust functions.

Two optimization opportunities remain: unconditional core `editor-facts` in the WASM transport
can be reviewed separately, and Flowchart's metadata-generic constructors can produce duplicate
machine-code instances. Neither has a proved all-product repair here. The previous shared
Flowchart constructor trial still exceeded the old analysis ceiling and was rejected; its
rejection neither proves optimization impossible nor justifies restoring it without admission.
These are follow-up opportunities, not hidden improvements in this budget-only change.

Full/render deliberately stop including `math` by default. Their dependency counts fall from
171 to 126 and 169 to 124, respectively, with the RaTeX/font closure removed. Their smaller bytes
are current-product measurements, not an equal-capability optimization claim or a credit against
the other profiles' growth. Their new ceilings decrease too.

Typst already excludes math. Its 379-byte excess is the amount above an old ceiling, not a measured
379-byte branch regression. The historical baseline used Binaryen 131; the current owner uses
133. There is no matched adjacent-main Typst artifact in this comparison, so this report does not
claim a precise branch-only Typst delta.

## Adjacent-main comparison

Values are stripped bytes. Analysis/ASCII/editor retain identical artifact recipes. Full/render
change their math capability and must be interpreted separately.

| Profile | Adjacent main | Head | Difference |
| --- | ---: | ---: | ---: |
| web-analysis | 3,936,967 | 4,033,974 | +97,007 (+2.464%) |
| web-ascii | 5,488,152 | 5,689,883 | +201,731 (+3.676%) |
| web-editor | 4,049,958 | 4,146,707 | +96,749 (+2.389%) |
| web-full | 16,668,044 | 14,374,041 | -2,294,003 (-13.763%; math excluded) |
| web-render | 14,754,019 | 12,371,711 | -2,382,308 (-16.147%; math excluded) |

The adjacent main already has only 1,033 stripped bytes of headroom for analysis, 28,848 for
ASCII, and 4,042 for editor under the older theme baseline. A ceiling change is therefore not
the same percentage as this branch's growth. The new 3% allowance starts above the explicitly
reviewed current product rather than pretending the old baseline still has that allowance left.

## Accepted Linux measurements and ceilings

All values are bytes. These exact head values come from the six owner rows in run `40113172363`;
they are not the older candidate's measurements.

| Profile | Raw | Stripped | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 4,034,209 | 4,033,974 | 1,539,622 | 1,175,453 |
| web-ascii | 5,690,118 | 5,689,883 | 2,097,346 | 1,582,318 |
| web-editor | 4,146,942 | 4,146,707 | 1,583,704 | 1,205,034 |
| web-full | 14,374,276 | 14,374,041 | 5,289,704 | 3,884,051 |
| web-render | 12,371,946 | 12,371,711 | 4,597,272 | 3,383,446 |
| typst-wasm | 20,124,533 | 12,293,379 | 4,660,522 | 3,439,561 |

| Profile | Raw ceiling: old -> new | Stripped: old -> new | Gzip: old -> new | Brotli: old -> new |
| --- | ---: | ---: | ---: | ---: |
| web-analysis | 3,938,000 -> 4,156,000 | 3,938,000 -> 4,155,000 | 1,555,000 -> 1,586,000 | 1,192,000 -> 1,211,000 |
| web-ascii | 5,517,000 -> 5,861,000 | 5,517,000 -> 5,861,000 | 2,076,000 -> 2,161,000 | 1,575,000 -> 1,630,000 |
| web-editor | 4,054,000 -> 4,272,000 | 4,054,000 -> 4,272,000 | 1,602,000 -> 1,632,000 | 1,224,000 -> 1,242,000 |
| web-full | 16,857,000 -> 14,806,000 | 16,857,000 -> 14,806,000 | 6,308,000 -> 5,449,000 | 4,665,000 -> 4,001,000 |
| web-render | 14,897,000 -> 12,744,000 | 14,896,000 -> 12,743,000 | 5,636,000 -> 4,736,000 | 4,172,000 -> 3,485,000 |
| typst-wasm | 20,252,000 -> 20,729,000 | 12,293,000 -> 12,663,000 | 4,737,000 -> 4,801,000 | 3,517,000 -> 3,543,000 |

The downloaded head Web raw SHA-256 values are:

| Profile | SHA-256 |
| --- | --- |
| web-analysis | `2e5fb182f7f1a4b4176b5ad7bc34bad1b49c40dcc79a8d164e3bd091b80a421c` |
| web-ascii | `b7bef8ba8f9fde9e25708c0542b78ad4d035ec7202f9cc3cc06eb091caa7d8a4` |
| web-editor | `42b6fc4a27a7eb645dd5f140a129d68eb2a151a2658e3896807b8b9f06daddd6` |
| web-full | `d73a435e378bd62f31e5c0b4ca2adad5fca9cb0c27bf104265b96d871b7f4cd1` |
| web-render | `6a7239bca664b1a7b0ea1a0bade4df5eccd1607da1c9028f5e203d1e6027d21e` |

The independent Windows Typst rebuild reports 20,113,329 raw, 12,301,301 stripped,
4,661,579 gzip, and 3,442,443 Brotli bytes. Its raw SHA-256 is
`33fe7b5d60f9041914bd7c817289e28f5e78d3b52e43e4da1bf3e74a9dbb8c26`.
The stripped difference from Linux is 7,922 bytes (0.0644%); all four metrics fit the
Linux-derived ceilings without additional host allowance. The local artifact is not claimed
to be byte-identical to Linux or used to replace its baseline values.

## Verification and remaining gate

The measurement registration, original failed logs, downloaded packages, provenance/hash checks,
section comparisons, and local owner replays are retained under
`target/bench/experiments/pr241-wasm-budget-review/`. Local checks and the subsequent CI run must
be recorded separately from the original Linux measurements.

The workflow planner, release-security, and theme-runner contracts run 103 tests: 100 pass and
three Bash execution cases skip because no native Bash is installed. The first attempt found
Windows' WSL launcher named `bash.exe`, causing launcher/decoding errors rather than workflow
assertion evidence; it is retained as an environment failure. The replay excludes those launcher
directories only from that subprocess's PATH, allowing the existing Bash availability skips to
apply. No test or source guard is weakened. Linux CI remains the Bash execution owner.

The corrected CI theme-consumer command passes **84/84 tests**, including the real PNG/PDF
consumers. Size-owner contracts pass **20/20 tests**. All six descriptor-owned WASM runtime
dependency-closure checks pass; these runtime-only inventories have a different scope from
the license-report inventories used for the historical comparison. Actionlint 1.7.12 passes
both changed workflows with ShellCheck unavailable; ShellCheck and zizmor remain CI-owned.

The existing owner passes all **24 new byte ceilings**: 20 on the downloaded Linux Web artifacts,
and four on the independent Windows Typst rebuild. The canonical Typst package build also passes
its shared theme vectors: 22 support, two materialization, and three error cases. These checks
retain the original recipes; the final Linux owner still belongs to the new CI run.
The resulting publish-profile package passes its real **Typst 0.15.1** smoke: 23 compilation
fixtures and nine expected failures. The local pinned CLI archive matches the official release
asset SHA-256. No registry publication or release is performed.

VS Code's packaging-tool audit still reports one MSAL advisory through two high dependency-path
nodes. At this review, released `@azure/identity@4.14.0` still requires MSAL `^6.0.0`, while the
patch requires 7.1.0. Per maintainer direction, no incompatible override is forced. The supported
graph and visible audit result are retained until a supported patched parent chain is available.
The production audit remains clean; this is not an audit waiver. No dependency or audit gate is
changed by this budget decision.
