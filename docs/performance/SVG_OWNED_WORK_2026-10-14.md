# SVG owned-work checkpoint, 2026-10-14

This checkpoint implements private theme ownership and SVG terminal-fact repairs on frozen theme PR #178 (`f47abded02b540d7fe563af7bc75f2fa1bb90c51`).
It is not a main-versus-release comparison.
All retained changes preserve the selected baseline's public interfaces, output policy, capability set and digest contracts.
Timing was collected from an uncommitted local candidate. These numbers remain diagnostic; later commits do not substitute for calibrated clean-revision confirmation.

## Structural changes

Legacy default/extended theme calculation resolves variables locally and publishes them only after all fallible calculation succeeds.
The implementation preserves both framework mutation revisions needed by detector replay, without publishing an intermediate variable map or cloning the whole configuration for a second transaction.
A config-owned exact predicate skips legacy provenance propagation when there are no explicit paths or compatibility binding.
Staged theme execution, value calculation, color errors and authored/detector ownership remain governed by their existing semantics.

The old production code with characterization tests passed before replacement.
Exact work tests show that an exclusively owned legacy configuration changes from one complete JSON copy-on-write to zero, while shared input still copies once.
Empty-provenance requests stop examining explicit ownership candidates; tracking, frozen and explicit owners retain derived ownership.
The real late-error cases use `neo-dark`/`redux-dark` with invalid `cScale2`, preserving both JSON and metadata on failure.

The renderer consumes terminal validation and moves its reference plan/resource collection into the finalization report instead of cloning discarded owned data.
For explicit best-effort parity/readable output, its final pipeline XML check retains bounded ordinary reference facts and passes them directly with the exact final string to terminal resource admission.
The complete XML traversal count falls from three to two in the public family-to-standalone completion test.
Omitted pipeline, strict portability and resvg-safe paths retain their own scopes.

Reference expansion still occurs at terminal admission after resource fingerprinting.
Overflow of the internal observation retention bound drops the observation and falls back to the existing scanner; it does not introduce a new early rejection.
Unknown host processors still invalidate prepared text/math/theme evidence under the existing rules, including when their returned bytes happen to match.
The patch does not provide a new whole-path peak-memory bound, public certificate, generic cache or bypass of resource checks.

## Complete Windows CLI experiment

A separate main-based experiment (`3a4b8c393939b69372a8f05a2b971f7cdf6ae529`) built the full `cli-release --build-host` recipe with only final-link delay-import settings changed.
Both local artifacts use dynamic CRT; this is not the static-CRT release recipe.
PE inspection confirmed delayed OLEAUT32, CRYPT32 and IPHLPAPI imports.
Ninety-one metadata/help/error/completion contract invocations had identical stdout, stderr and exits.
The six diagnostic operations also retained identical output identities.

Each operation interleaved 24 fresh processes per artifact after three warmups, without a compiler or profiler running.

| Complete operation | Base median | Delay-import median | Change |
| --- | ---: | ---: | ---: |
| Tiny native SVG stdout | 49.94 ms | 47.47 ms | -4.93% |
| Tiny mmdc-compatible SVG stdout | 45.16 ms | 43.18 ms | -4.38% |
| Version | 37.66 ms | 35.07 ms | -6.85% |
| Help | 37.28 ms | 35.40 ms | -5.03% |
| Medium sequence SVG stdout | 45.17 ms | 42.45 ms | -6.03% |
| Tiny Mermaid input to PNG file | 157.72 ms | 156.55 ms | -0.75% |

The primary SVG improvements did not meet the preregistered joint 10% and 1 ms screen.
This candidate is rejected for that CLI latency objective and no production loader setting is retained.
The screen does not prove zero possible benefit.
Actual DNS/TLS first-use and release static-CRT confirmation were not expanded after rejection, and no correctness/admission claim is made for those untested surfaces.
PNG remains a separate capability control; its font catalog is not part of ordinary SVG generation.

## Verification evidence

At this checkpoint, the following owned checks have completed:

- Baseline theme characterization: 2 tests passed on old production code.
- Core library: 460 tests passed, including failure atomicity, provenance and work counters.
- Render pipeline: 281 tests passed, including differential budgets, observation fallback and cancellation.
- Complete render library with all diagrams: 2,775 tests passed; 2 pre-existing manual workload benchmarks are ignored.
- Core all-diagram appearance/source-presentation integration: 10 tests passed.
- Public renderer/runtime determinism: 12 tests passed, including identical fresh-process SVG with system adapters compiled.
- Core/render all-targets all-features Clippy: passed with warnings denied.
- Changed-crate formatting and diff hygiene: passed.
- Exact full SVG structure owner command: passed across 37 family reports, selecting 3,739
  fixtures and rendering 3,712, with 27 owner-documented skips. Existing exact browser-layout
  and parser-diagnostic residuals remain explicitly recorded; the latter are not DOM parity.
- Compiled native-memory protocol smoke: passed the registered 1x/100x operation/zero pairs;
  the existing driver verifier also passed its zero-work measurement boundary checks.
- Core/render compilation with no default features: passed. This minimal recipe emits unused
  code warnings with diagram leaves disabled; the all-features Clippy recipe above denies warnings.

The smaller all-diagrams-only Clippy recipe exposes unused math/layout code; the complete owner feature recipe passes.
`cargo fmt --all` exceeds Windows process command-length limits in this workspace, so formatting is checked using the affected crate invocations without formatting unrelated source.
The structure command ran locally on Windows; this is not a claim that a Linux runner completed
the check. It used frozen reference SVGs without regeneration or comparator changes. Its first
release build took 25 minutes 13 seconds and the subsequent full comparison took about 28 seconds.
The native-memory probe was separately prebuilt with its registered locked
`--no-default-features --features all-diagrams,svg` bench recipe and frozen before execution.
The driver used `--smoke --executable` against that SHA-verified artifact, retaining a separate
build receipt and leaving shared build artifacts intact. This proves completion under the probe's
resource policy; it is not an A/B memory improvement or a new whole-path memory bound.

## Native diagnostic observations

Two reversed-order rounds measured six same-input fixtures with strict parsing, a reused renderer, deterministic resources, 30 Criterion samples, 2 seconds warmup and 3 seconds measurement.
The identical temporary harness distinguishes `parse`, public `end_to_end` without a requested pipeline, and public `end_to_end_parity` with explicit parity.
All 18 base/candidate output identities matched before and after timing, including the SVG bytes for every complete render.
Values below are medians of the two Criterion round estimates, not medians of independent raw native samples.

| Fixture | Parse change | No-pipeline complete change | Explicit-parity complete change |
| --- | ---: | ---: | ---: |
| Tiny flowchart | +1.48% | +8.38% | +11.52% |
| Tiny class | -0.19% | -7.21% | -9.28% |
| Tiny sequence | +6.57% | -1.79% | -5.17% |
| Medium flowchart | -0.68% | +4.85% | -9.13% |
| Medium class | -0.62% | +0.14% | -13.81% |
| Medium mindmap | -2.37% | +1.60% | -9.36% |

The theme repair does not establish a broad ordinary-fixture latency improvement.
The explicit-parity path generally benefits, but the second tiny-flowchart candidate invocation had a wide estimate interval, 1.1801-1.6232 ms, compared with the base's 1.0763-1.1117 ms.
That observation is retained, not removed as an outlier.
It triggers a separate four-pair longer diagnostic for tiny flowchart plus class control, using 3 seconds warmup and 5 seconds measurement per invocation.
That follow-up does not retroactively convert this initial screen into clean-revision confirmation.

The four alternating AB/BA pairs completed with identical output identities and these medians of round estimates:

| Fixture and public operation | Base | Candidate | Change |
| --- | ---: | ---: | ---: |
| Tiny flowchart, omitted pipeline | 833.53 us | 842.89 us | +1.12% |
| Tiny flowchart, explicit parity | 1039.85 us | 955.83 us | -8.08% |
| Tiny class, omitted pipeline | 761.08 us | 784.75 us | +3.11% |
| Tiny class, explicit parity | 976.28 us | 895.88 us | -8.24% |

The longer follow-up did not reproduce a material systematic regression on the anomalous flowchart lane.
It supports the traversal repair as a diagnostic non-regression control; it does not provide calibrated statistical acceptance or guarantee an 8% portable speedup.
The production disposition is structural: less duplicate XML work with existing admission contracts, not an admitted wall-clock claim.

Evidence limitation: the follow-up inadvertently reused the first diagnostic's Criterion sample directory for the overlapping round-0/1 tiny-flowchart/class complete-operation cases.
Sixteen of the initial 72 `new/sample.json` paths were replaced by follow-up samples; all 104 invocation logs and their output/estimate receipts remain separate.
There are 88 distinct retained latest raw sample paths, rather than 104 independently retained native sample files.
The two diagnostic tables come from their separately retained logs/JSON; no pooled raw-sample confidence or admission claim is made.

This is a combined U1/U2 diagnostic; exact work counters identify the separate structural repairs, but wall-clock deltas must not be assigned to individual functions by subtraction.
No global renderer ranking or fresh-CLI improvement follows from these sub-millisecond native results.

## Review and reproduction

Independent correctness and adversarial reviewers found no source-level contract defect in the retained theme/SVG changes.
A maintainability finding removed an unnecessary private forwarding function.
An independent evidence audit verified all retained timing tables, artifact/source hashes, output
identities, CLI contracts, test counts and the raw-sample limitation above.
Runtime test results remain separate from those static review conclusions.
Facts-to-bytes association is private and adjacent; a future transform inserted between those stages must invalidate the observation.

The executable experiment ledger, logs, exact source patch and frozen artifact identities are retained under ignored `target/bench/experiments/svg-owned-work-2026-10-13`.
The CLI experiment uses its separate `svg-owned-cli-2026-10-13` directory in the main-based worktree.
Cargo builds are serialized with `CARGO_BUILD_JOBS=1`; timing invokes frozen executables directly
with per-side/fixture sample directories and the explicitly disclosed follow-up phase reuse.
The benchmark-only explicit-parity lane was temporary and identical on both revisions; both
checked-in harness files were restored from byte-verified backups after sampling.

The blocking structure command is reproduced without additional feature or comparator settings:

```text
cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
```

The compiled memory build, smoke and verifier commands, frozen SHA-256, raw responses and all
37 structure reports are included in the evidence bundle. The native timing lane retains two
separate executables and the exact temporary harness rather than rebuilding during sampling.

The verified bundle is stored outside the repository at
`E:/Rust/merman-cli-perf-20261013/svg-owned-work-evidence.zip`, with a separate archive receipt.
It contains a per-file SHA-256 manifest and excludes build targets and frozen executables;
executable paths, sizes and digests remain in their experiment receipts.

The bounded implementation follows `docs/plans/2026-10-13-2320-refactor-svg-owned-work-plan.md`.
Finite theme caches, persistent CLI protocols and file-sync stage changes remain follow-up work.
The experiment introduces no release publication or ordinary CI timing gate.

## Authorized shipping and main integration

After the local checkpoint, the maintainer authorized committing the changes, merging remote main
and opening a PR. The three implementation/evidence commits are `1977d4cfd`, `481087d5c` and
`7149629bb`. Integration selects the existing admitted Mermaid 12.2 main at
`768370e0842dbd8df24d477fdbbf3449eb61de3d`; it is separate from the frozen 12.1 timing evidence
above. The PR targets #178's `refactor/theme-ownership-performance` branch, as selected by the
maintainer, because that typed-theme prerequisite is still open.

The original evidence archive remains immutable. Current integration validation is retained under
ignored `target/bench/experiments/svg-owned-integration-2026-10-14`; main merge conflicts and
generated-output refresh are verified before publication. No timing result above is reassigned
to the merged 12.2 revision.


Merged-source local checks completed:

- Core all-diagram library: 1,899 passed.
- Render all-diagram library: 2,802 passed; 2 pre-existing manual workload benchmarks skipped.
- Agentflow/Circle/Mindmap/Swimlane/Railroad/Usecase/State ELK regressions: 94 passed.
- Public prepared-render/runtime determinism: 12 passed.
- All-targets/all-features core/render Clippy with `-D warnings`: passed.
- Theme runtime/oracle generation: passed; values and prepared constructors unchanged,
  only selected 12.2 provenance changed.
- Full CLI Rustdoc build/check: passed; generated fragment bytes unchanged.
- Full `structure,parity,parity-root` SVG comparison: passed across 37 family reports,
  with 3,756 selected / 3,729 rendered / 27 owner-documented skips.
- Workflow contracts: 97 run, 3 Linux-shell execution cases deferred on Windows.
  CI-pinned actionlint 1.7.12 and zizmor 1.30.1 passed for resolved workflows.
- Release legal projections (394 files) and third-party license contract: passed.
  Rust dependency license generation freshness also passed: 13 reports. The initial local
  cargo-about 0.9.1 could not run the owner; an isolated official 0.9.2 prebuilt was SHA-verified
  and used for the successful check, leaving the system installation unchanged.

The residual catalog retains main's exact 132 browser and 10 parser identities, modes and
source/upstream digests. Eleven browser signatures changed only after independent comparison
with real merged SVGs and pinned Circle/ELK behavior. Ten parser outputs were proven byte-identical
to the accepted pre-merge outputs after one exact 12.1-to-12.2 version-footer substitution.
No new residual scope or comparator normalization was added.

The complete materialized-reference check was attempted but lacks the local Playground and
selected source checkouts. The standing committed reference graph verifier passed. Browser
painted-content checks and Linux workflow execution remain their existing CI owners.

The merged-source compiled native resource probe passed both 1x/100x operation/zero pairs
and its zero-work boundary verifier. This is protocol/resource completion evidence, not an A/B
memory improvement. Affected-crate formatting, staged diff hygiene and indexed path-length checks also passed.
No native timing claim is transferred to the integrated 12.2 source.
