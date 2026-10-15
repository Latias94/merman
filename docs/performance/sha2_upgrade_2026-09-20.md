# SHA-256 dependency upgrade — 2026-09-20

## Decision

Keep the sha2 0.11.0 upgrade. On this macOS ARM64 host, Class medium public end-to-end
latency falls from 1,461.74 to 1,248.73 microseconds: 213.01 microseconds, or 14.58%. The
simultaneous bounds show a 14.11–15.17% latency reduction and a 206.64–221.66 microsecond
absolute reduction. The registered primary clears both existing thresholds. All five rows
confirm non-regression, and Sequence medium also clears the improvement gate.

This is an adjacent native optimization, not recovery against alpha.6 or C7a closure.

## Scope and compatibility

The candidate upgrades workspace-owned SHA-256 from sha2 0.10.9 to 0.11.0. The earlier
[ARM64 attribution](sha2_arm64_attribution_2026-09-20.md) identified software compression in the
old native profile; the new crate selects ARM64 SHA instructions at runtime and retains a
software fallback. This change preserves the algorithm, digest framing, domain separation,
resource policy, public receipt fields and both required SVG hashes.

The new digest return type no longer implements LowerHex. Existing consumers now use
`data_encoding::HEXLOWER`: `encode` for complete strings and `encode_display` inside prefixed
identities. The latter avoids an extra temporary string. The migration includes fixture/tool
identities, math occurrence keys, FFI catalog identity, and benchmark preflight helpers.

## Controlled comparison

The original discovery attempt rejected the dirty candidate and unequal benchmark source
before collecting timing samples. The confirmation harness was not weakened. Two clean,
local measurement commits use byte-identical pipeline benchmark source and corpus:

- Base `15a0e543a`, derived from `f0e86d32d`, keeps old production code and sha2 0.10.9. Its
  four-file adjustment changes only preflight hex encoding and the benchmark's dev dependency.
- Candidate `6af1f879c` snapshots the 58 validated upgrade files. Its parent `557036b4d` differs
  from the original baseline only by an isolated Dart formatter correction.

The integrated source commit `62e5afcea` has the exact same tracked Git tree as measured
candidate `6af1f879c`; the integration changes only commit metadata.

Both use Rust 1.95.0, release, no default features, and `svg,layout-elk,layout-cytoscape` on an
Apple M4 Pro running macOS 26.6.2 ARM64. Builds and measurements run serially. The registered
primary is Class medium public end-to-end latency: improvement must exceed both 10% and
50 microseconds with simultaneous 95% confidence. Four cross-family/control rows must have no
confirmed regression under the same gate. Each executable receives eight balanced A/A pairs;
calibration selected eight AB/BA confirmation pairs, below the registered cap of sixteen. Each run
uses 30 samples, two seconds of warmup, three seconds of measurement, and 10,000 paired bootstrap
resamples. No sample-count or threshold change is permitted after observing confirmation.

## Latency and size results

Values below are the comparison tool's paired estimates. Relative change is candidate/base
elapsed time minus one; negative values mean lower latency. The mirrored improvement gate in
the raw report also contains reciprocal ratios, which must not be read as latency reductions.

| Fixture | Base µs | Candidate µs | Delta µs | Relative change | Joint improvement gate |
| --- | ---: | ---: | ---: | ---: | --- |
| class_tiny | 204.73 | 162.01 | -42.72 | -20.87% | Below 50 µs absolute threshold |
| class_medium | 1,461.74 | 1,248.73 | -213.01 | -14.58% | Confirmed |
| xychart_medium | 258.37 | 219.70 | -38.67 | -14.97% | Below 50 µs absolute threshold |
| flowchart_medium | 2,991.00 | 2,762.80 | -228.20 | -7.63% | Below 10% relative threshold |
| sequence_medium | 495.30 | 402.54 | -92.76 | -18.72% | Confirmed |

All five rows pass the non-regression gate. The three rows below one improvement threshold
retain their `inconclusive` improvement classification; no threshold was relaxed. Sequence
medium's latency-reduction bounds are 18.32–19.04%, or 89.31–95.38 microseconds.

There are 160 A/A calibration invocations and 80 fresh confirmation invocations, each with
30 Criterion samples: 7,200 samples in total. The inference unit is the eight paired runs per
fixture, not 7,200 independent experiments. The comparison applies Bonferroni simultaneous
95% coverage across ten relative/absolute components, with 99.5% component bounds and 10,000
bootstrap resamples.

With identical benchmark source, executable bytes decrease from 29,172,336 to 29,103,968:
68,368 bytes, or 0.23%. This is the native benchmark executable, not a shipping-package budget.
The initial unequal-preflight binary comparison is superseded by this controlled pair.

## Correctness and dependency checks

The original source baseline and candidate produce byte-identical SVGs and full public admission
receipts for Class tiny/medium, XY Chart medium, Flowchart medium and Sequence medium. All ten
preset recipe/font-catalog fingerprints match. Fifteen paired allocation observations preserve
allocation count, cumulative bytes and tracked heap peak growth exactly, with zero retained
heap growth after each operation.
The controls were repeated against the libraries behind the final clean executables and also
match the original source baseline. Post-sampling verification confirms unchanged executable,
source, corpus, manifest, lockfile and Git identities.

Validation completed on the candidate source:

- 4,629 core/render/facade/binding library tests passed; two skipped.
- 111 FFI and fixture catalog tests passed; one skipped.
- Workspace all-targets, minimal facade, Node native, Web WASM and Typst WASM compilation passed.
- Representative dependency closures, all eleven feature combinations, cargo-deny and scoped
  Clippy passed. The complete SVG structure gate passed with the existing browser-text policy.
- Native ABI and binding contracts passed. Android target Clippy, generated Dart binding identity,
  Flutter analysis, ABI contracts, Native Assets smoke and theme-authoring smoke passed. The first
  platform run stopped on existing Dart format drift, corrected separately in `557036b4d`.
- Thirteen generated legal source reports, 382 release projections, the third-party contract and
  eighteen generator/projection Python tests passed.

The root lock removes sha2 0.10.9 without adding a package; 0.11.0 was already present through
another dependency. Node replaces its older hash dependency chain and adds data-encoding. Fuzz
removes the old hash chain and adds data-encoding. Representative runtime-profile closure checks
pass; a smaller workspace lock is not proof of a smaller shipped package.

## Evidence and limits

Raw inputs, logs, hashes, temporary revision identities, allocation controls and comparison
reports are retained under `target/bench/experiments/sha2-upgrade-20260920/`. The experiment ledger
preserves the rejected preparation attempt. The native benchmark executable is a size control,
not an installed-package measurement. Cross-host performance, packaged size, cold start and
browser execution remain Unverified for this dependency change. Android compilation is not
Android device execution. The historical alpha.6 Class regression and C7a/U10 gates remain open.

Earlier installed-package and archive receipts remain bound to their recorded revisions. They
must be regenerated for the final release candidate; source/compile checks cannot relabel those
binaries as this upgrade.

The clean base benchmark executable SHA-256 is
`c3348a5c7cc4f35f5ff0012097a2fc1090fb0c09f0083cd9a6ba2a004c5cf8c0`;
the candidate is
`7136963df7b13fd27f193a1f60c5c1a45a9b6f2bb88a9d2a560de00205543d33`.
The final `confirmation.json` SHA-256 is
`6fabea7af31553dba28f80a9bceaf708505b315c98a18d53a3164298a94eca54`.
`clean-allocation-identity-controls.json` records the matching public artifacts and heap controls.

Reproduce with two clean checkouts and independent target directories. If reconstructing the
baseline from `f0e86d32d`, copy the candidate `crates/merman/benches/pipeline.rs`, add
`data-encoding = "2.11.1"` to workspace dependencies and `data-encoding.workspace = true` to
merman's dev dependencies, then refresh only the corresponding root lock entry. Keep sha2 at
0.10.9 and all production Rust source unchanged. Commit this baseline preparation locally.
The recorded temporary measurement revisions already contain the exact preparation.

```console
CARGO_BUILD_JOBS=1 python3 tools/bench/compare_self.py \
  --base-dir /private/tmp/merman-sha2-confirm-base \
  --head-dir /private/tmp/merman-sha2-confirm-head \
  --base-label sha2-0.10-common-preflight --head-label sha2-0.11-candidate \
  --base-package merman --head-package merman --base-bench pipeline --head-bench pipeline \
  --base-features svg,layout-elk,layout-cytoscape \
  --head-features svg,layout-elk,layout-cytoscape \
  --no-base-default-features --no-head-default-features \
  --base-toolchain 1.95.0 --head-toolchain 1.95.0 \
  --base-target-dir /private/tmp/merman-sha2-base-f0e86d32d/target \
  --head-target-dir target \
  --filter 'end_to_end/(class_tiny|class_medium|xychart_medium|flowchart_medium|sequence_medium)' \
  --preset long --sample-size 30 --warm-up 2 --measurement 3 \
  --evidence-mode confirmation --calibration-pairs 8 --max-pairs 16 \
  --bootstrap-resamples 10000 --relative-threshold-percent 10 --absolute-threshold-ns 50000
```
