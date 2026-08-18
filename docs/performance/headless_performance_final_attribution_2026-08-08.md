# Final headless-performance attribution — 2026-08-08

> The semantic-token entries in this revision-bound report were superseded on 2026-08-17 when the
> parser-backed token planner was removed in favor of Tree-sitter syntax highlighting. Other lanes
> in this historical attribution remain unchanged.

## Decision

U13 is complete with one new production candidate. The fixed-budget four-lane scan admitted the
smaller Browser-WASM host-disposition read as `accepted-structural`; it admitted no new native,
CLI, semantic-token, or diagnostics candidate.

The final decision is **completed-one-new-candidate**:

- native rendering retains stage-level triage and the existing performance queue;
- Browser-WASM reads the existing optional `handled` property directly before the unchanged full
  result decoder, removing one disposition-only Serde decode per handled callback;
- CLI target indexing and the semantic-token reducer remain accepted-structural only;
- compact CLI persistence and deeper diagnostics-only capture remain rejected-not-admitted; and
- the representative native interaction guard found no unexplained material regression in its
  comparable controls.

The default 50-microsecond comparator budget is a reporting and screening fallback, not a universal
admission or rejection rule. The Browser-WASM candidate lands under the registered structural
contract: its work reduction is exact, the change is small and owner-local, semantic and artifact
gates pass, and no A/B pair regressed. Browser A/A noise exceeded the timing-grade limit, so this
receipt deliberately makes no new latency or memory claim.

## Revision boundary

| Role | Commit | Meaning |
|---|---|---|
| `R` | `56227a541011a3929b808bb3555d67372d630aae` | Historical `v0.8.0-alpha.3` coverage context. |
| `S` | `8e9f38cf8d26d131fbb47acbe4f39a40681d34ff` | Performance-program branch start. |
| `E` | `5117c0ae12da2c0346b47061642286174cea3f5f` | Output-identity harness descendant with production source equivalent to `S`. |
| `F` | `8d626b6578ba25c53407f2e377c175ff9e3ff3e3` | Final measured production source, including the accepted Browser-WASM structural candidate. |

The committed branch-start receipt proves `production_source_changed: false` for `S..E` and lists
the three evidence-only commits in that range. The scenario-aware policy commit
`b60b4b1202fd2403d06b7c2f65d3953a1ba3e06f` precedes `F` and changes documentation only. The commit
containing this receipt follows `F` and also changes documentation only; performance observations
remain bound to `F` rather than to a mutable `HEAD` label.

The measurements ran on the registered Apple Silicon macOS host with Rust 1.95.0. Exact corpus,
runner, executable, feature, environment, order, and output-identity records remain in the ignored
raw reports listed below.

## Fixed-budget four-lane attribution

| Lane | Absolute observation and reachable upper bound | Owner | Claim class | Priority and disposition |
|---|---|---|---|---|
| Native rendering | `flowchart_medium` measured 2.0567 ms end to end: 159.11 us parse, 1.5872 ms layout, and 221.31 us render. `class_medium` measured 647.47 us: 77.910 us parse, 341.10 us layout, and 216.03 us render. Whole render stages are coarse ceilings; no semantics-matched owner-local upper bound was established. | Core parsing, selected layout backend, and `merman-render` SVG emission. | Triage-only; no accepted cross-runner latency claim. | Keep the existing Flowchart/Class/Requirement/Mindmap queue. Admit no native U13 candidate. |
| Browser-WASM host measurement | For `H` handled results, `F` replaces `H` disposition-struct deserializations with `H` direct `handled` reads while retaining the same `H` complete-result deserializations. Clean optimized artifacts changed from 12,255,769 to 12,255,381 bytes (-388). The current Edge support lane matched 468 callbacks, 33,567 returned bytes, 30,932 SVG bytes, and digest `5934a4f4` on both sides. | `merman-wasm` host text-measurement bridge. | Accepted-structural; browser latency not claimed. | Keep the direct disposition read. The historical custom visitor remains rejected and absent. See the [U3 follow-up receipt](wasm_host_measurement_disposition_2026-08-08.md). |
| CLI batch publication | The accepted plan owns one immutable target index. For `N = 1, 16, 64, 256`, every stage request performs exactly one expected hash lookup; 256 registered requests therefore perform 256 indexed lookups. The former owner bound was up to `N` sequential comparisons per request. | `TransactionPlan` normalization and staging transaction metadata. | Accepted-structural only. | Keep the index. The sealed-manifest/compact-frontier candidate remains rejected because its three-platform durability matrix does not exist. |
| Editor semantic tokens | Each active interval now uses one candidate scan, zero finalists-vector allocation, and no separate precedence or narrowness rescan. The complete planner still owns boundary sorting, active-set maintenance, source mapping, and result encoding, so no whole-operation timing bound is inferred. | `merman-editor-core` token planner. | Accepted-structural only. | Keep the reducer; queue any future LSP/WASM latency or allocation claim behind a new adjacent public-operation receipt. |
| Analysis diagnostics | `DiagnosticsOnly` avoids retaining rich indexes and generic syntax facts. The former diagnostic-bearing Flowchart projection was deleted with facts schema 2 because no editor/LSP operation consumed its public graph. | `merman-analysis` projection over shared parser evidence. | Accepted structural deletion; no latency claim. | Keep the narrow retained-object split and generic typed candidates; do not add a new parser capture policy solely for the removed graph. |

The native stage spot-check compares Merman with `mermaid-rs-renderer` only for attribution. The
products have different layout algorithms, supported behavior, and SVG output, so the reported
ratios are not candidate causality or a quality-adjusted ranking. In particular, the Flowchart
layout ratio reflects different algorithms and does not justify tuning Merman toward another
renderer's output.

U13 preregistered at most two new candidates. Only the Browser-WASM owner-local structural change
met an applicable scenario contract. The remaining work stays in `PERF_PLAN.md` rather than
extending this program.

## Browser-WASM structural admission

The retained implementation is not the rejected 2026-08-04 custom visitor. The old candidate
combined disposition and payload parsing through a large field-aware visitor and was removed in
`2d9a0473f603063f5b5cc2c843513c262e03b666`. The new implementation performs one direct property
read and then uses the existing complete-result deserializer only when the callback is handled.

The support lane ran eight A/A pairs on each revision and eight alternating AB/BA pairs in Edge
151.0.4129.72. Its paired median moved -5.303% / -137.500 us and every pair favored `F`, but maximum
robust A/A noise was 4.415%, above the 3% timing-grade limit. The timing result is therefore
inconclusive for a latency claim. It supports non-regression only; the exact repeated-work removal,
full Web semantic smoke, getter-order contract, and 388-byte artifact reduction carry the
structural decision.

This is the practical consequence of the scenario-aware policy: an exact, small, semantics-neutral
owner-local improvement does not become valueless merely because a generic 50-microsecond budget is
inapplicable. Conversely, the same exception does not admit a cache, retained state, protocol
extension, compatibility layer, or broad abstraction without stronger measured evidence.

## Aggregate interaction guard

### Direct `S -> F`

The direct representative comparison failed closed before sampling. `S` predates the native
output-identity receipt contract, while `F` requires SVG digest, byte, and element identity. All six
selected rows were classified as `unverified_output`, and the report added one coverage contract
error because zero rows were comparable. This is a legacy evidence-contract boundary, not a
measured performance regression.

### Production-equivalent `E -> F`

`E` is the registered production-equivalent proxy for `S`. Cross-family discovery found 14 rows
with matching output identity and 22 output mismatches. The mismatched rows were excluded from
timing rather than normalized or relabeled. Applying the preregistered fixture order selected the
first two matching rows for the fixed two-pair diagnostic guard:

| Fixture | `E` median | `F` median | Diagnostic movement | Interpretation |
|---|---:|---:|---:|---|
| `sequence_medium` | 208.130 us | 177.890 us | -14.530%, -30.240 us | Faster aggregate point estimate with exact SVG identity. The two-pair lane is neither candidate-causal nor decision-grade confirmation, so it creates no latency claim. |
| `state_medium` | 412.255 us | 415.590 us | +0.810%, +3.335 us | The simultaneous upper bounds were +1.735% and +7.110 us, within the registered interaction-regression guard. |

Both controls matched SVG digest, bytes, elements, and postflight identity. Because this was a
two-pair diagnostic guard rather than an eight-pair candidate confirmation, it supports only the
integration conclusion: the representative comparable controls expose no unexplained material
regression. The `sequence_medium` interpretation does not depend on whether its absolute movement
is below or above 50 microseconds.

## Historical `R -> F` context

The final cross-family discovery again found zero comparison-eligible alpha.3 rows:

| Classification | Rows | Meaning |
|---|---:|---|
| `unverified_output` | 22 | Both revisions execute the row, but alpha.3 cannot provide the required output-identity receipt. |
| `execution_failure` | 2 | Alpha.3 skips `mindmap_medium` and `architecture_medium`. |
| `current_only` | 12 | The final corpus contains families or records absent from alpha.3. |
| Comparable | 0 | No timing or causal conclusion is permitted. |

The 12 current-only records remain coverage expansion, not synthetic alpha.3 comparisons: Venn,
Swimlane, Event Modeling, TreeView, Ishikawa, four Railroad dialect records, Wardley, Cynefin, and
Error. The report records 36 coverage-only rows plus one zero-comparable coverage error; its
expected `contract_failure` outcome is historical context only.

## Public break inventory

The complete branch already has a user-facing migration inventory in the alpha.3-to-alpha.4
upgrade guide and release projection. U13 reconciled the performance-program outcomes with that
surface:

| Surface | Final break state | Documentation boundary |
|---|---|---|
| Rust layout and work accounting | Surviving breaks include the canonical transactional Dugong pipeline, non-exhaustive `LayoutError` / `WorkError`, ELK `model_order`, ordered `GraphExecution`, JavaScript array-index ordering, and completed-operation `layout_work_units()` evidence. | Upgrade guide and changelog describe the migration. |
| Binding resource errors | `details.resource.cause` remains the stable discriminator. JavaScript `actual` and `max` counts are `number` when safely representable and canonical decimal `string` for wider `u64` values; consumers must preserve both forms. | Upgrade guide, TypeScript declarations, and binding contract tests define the migration. |
| Browser-WASM host measurement | No U3 protocol or callback-shape break survives. The direct `handled` read is internal and preserves protocol version, result fields, fallback behavior, and public APIs. | This receipt and the U3 follow-up receipt. |
| CLI transaction persistence | U10 changes private in-memory lookup ownership only. No journal version, on-disk transaction format, recovery authority, or synchronization break survives. | This receipt and the U10 decision receipt. |
| Semantic tokens | U11 preserves `PlannedToken`, packed five-field output, legend order, error ordering, LSP behavior, and WASM ABI. | This receipt and the U11 decision receipt. |
| Diagnostics capture | U12 adds no parser capture policy or public analysis mode. Existing diagnostics, recovery, cancellation, and rich-facts behavior remain the boundary. | This receipt and the U12 rejection receipt. |

No additional performance-program-specific browser protocol, CLI transaction-format,
semantic-token ABI, or diagnostics-capture break needs migration text. This conclusion does not
erase the broader alpha.4 Rust, binding, CLI, Web, or LSP breaks already listed in the upgrade
guide and changelog.

## Cleanup and documentation reconciliation

- The rejected custom WASM visitor and its candidate-only field table remain absent; the smaller
  direct disposition read is the only U13 production addition.
- No pinned-registry cache, global text cache, sealed recovery manifest, compact frontier,
  semantic-token finalists vector, deeper parser capture policy, or dormant experiment switch is
  retained by these decisions.
- `BENCHMARKING.md`, `RUNBOOK.md`, the implementation plan, and `PERF_PLAN.md` now treat 50
  microseconds as a default reporting fallback rather than a universal admission threshold.
- `PERF_PLAN.md` links this closure, records U13's final state, retains the remaining queue, and
  binds U10-U13 evidence to the Asia/Shanghai receipt dates.

## Raw evidence

All U13 raw files are intentionally ignored under
`target/bench/experiments/u13-final-attribution/`:

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `experiment.yaml` | 5,016 | `4b07b4bdff7eeca2352756e2af64dfb5a9aefc6691294410f4ec136ea6cb9397` |
| `stage-spotcheck.md` | 1,012 | `03a213ad96d8186d233165aba888f779547110403e5d2a90abb13ea860feb0ef` |
| `s-to-f.json` | 237,615 | `599b87ffad981019acc509f31a24825f67888bf27d212cadbd930078d07ffa83` |
| `s-to-f.md` | 2,190 | `a5e9de6b664ced71309ac9c6a2f721aa650c67aaef436f12fd66f6bd6727cdce` |
| `s-equivalent-to-f-discovery.json` | 526,064 | `c4d053430b041b83d680bebde2c64793bd050badca9a244561ae90f40ed1a98a` |
| `s-equivalent-to-f-discovery.md` | 5,321 | `b0ccac19c2f5e9d34029029aa69b32454fb07bea028a2709f141863bb285dbbb` |
| `s-equivalent-to-f.json` | 387,969 | `86efebd20abbeb6c4035bf8b32389a534ac5128ccc2c4c386d0969996ef469e6` |
| `s-equivalent-to-f.md` | 1,857 | `be8dd19fd3261d69214dee4a9c10c8e5533f0068365454bace5150603ac4ef70` |
| `r-to-f-discovery.json` | 373,103 | `fc90ae1ceabb7f092d592194f78bdde5b54699b922a69ab9ab0213df13eabf18` |
| `r-to-f-discovery.md` | 5,369 | `940f29f72f7b8ecf42c65938b92fd5d5be70392fa51885013681bef664504911` |

The Browser-WASM worktree, artifact, Edge A/A, and AB/BA evidence is separately bound by digest in
the [U3 follow-up receipt](wasm_host_measurement_disposition_2026-08-08.md).

## Verification

The closure worktree was verified serially with `CARGO_BUILD_JOBS=1` for every Cargo command.

| Gate | Outcome |
|---|---|
| `python3 tools/bench/test_perf_contracts.py` | 102 contract tests passed. |
| `cargo nextest run --locked -p merman-core --test-threads 1` | 1,450 tests passed. |
| `cargo nextest run --locked -p merman-render --test-threads 1` | 1,488 tests passed and 3 were skipped. The comparison-text semantic fix required one reviewed Flowchart layout-golden width change from 148.0 to 162.88. |
| `cargo nextest run --locked -p merman-layout-elk -p manatee -p dugong -p merman-cli -p merman-editor-core -p merman-analysis --test-threads 1` | 1,425 tests passed and 1 was skipped. |
| `cargo nextest run --locked -p merman-wasm --all-features --test-threads 1` | 33 tests passed. |
| `cargo nextest run --locked -p merman-bindings-core -p merman-ffi --test-threads 1` | 160 tests passed. |
| `cargo nextest run --locked --manifest-path crates/merman-node/Cargo.toml --features svg,layout-cytoscape,layout-elk,math,transport-napi --test-threads 1` | 12 tests passed. |
| `npm --prefix platforms/node test` | 82 tests passed. |
| `npm --prefix platforms/web test` | 127 tests passed, including the absolute Cargo target-directory regression. |
| `npm --prefix platforms/web run build` and Web smoke | The full, analysis, render, editor, and ASCII WASM packages built and the browser smoke passed. |
| Full pinned-source root-residual regeneration and review | 1,566 hash-bound observations passed the final `parity-root` gate. Three Flowchart observations were added and 12 existing local root signatures changed; all 15 retained complete descendant `parity` and the source-backed `browser-root-bbox` classification. |
| `cargo run -p xtask -- verify-mermaid-reference --materialized` and `verify-dompurify-defaults` | Passed after advancing the governed DOMPurify baseline to 3.4.13. A full upstream-family regeneration was inspected before retaining the canonical SVG bytes: 21 Block and 18 State changes were generated IDs, 122 Gantt changes were the wall-clock-dependent `today` marker, and one Flowchart change was external-image loading. The 35 manifests therefore advance only the reference package digests. |
| `npm --prefix playground run verify:dependencies` and `verify:security` | Passed; the production audit reports zero vulnerabilities. |
| Complete strict matrix | The last complete Rust closure matrix passed 7,128 tests with 8 skips, the 116-case feature matrix, formatting, checks, Clippy, doctests, structure, SVG parity, and root parity. After the final Web target-directory and dependency-provenance fixes, the focused gates above passed. Per maintainer direction, the exact final whole-repository rerun is delegated to PR CI instead of being duplicated locally. |
| `cargo fmt --all -- --check` and `git diff --check` | Passed on the final intentional diff. |

The direct `S -> F` and historical `R -> F` comparison commands intentionally returned evidence
contract failure because the older revisions cannot supply the required output-identity receipts.
The production-equivalent `E -> F` sampled guard returned diagnostic advisory success with exact
output identity and no material control regression, as recorded above.
