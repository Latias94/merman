# Headless performance branch-start evidence — 2026-08-02

## Decision boundary

Status: **evidence-only**. This receipt establishes benchmark coverage, output-identity
enforcement, and a same-tree diagnostic baseline. It makes no alpha.3-to-current latency,
speedup, or regression claim.

The historical release does not emit the v1 native Criterion output receipt. Consequently,
every `R -> E` row is coverage-only even when the fixture bytes and benchmark capability match.
The same-tree `E -> E` observations use only two diagnostic AB/BA pairs and are not eligible for
confirmation language.

## Revisions and source boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| `R` | `56227a541011a3929b808bb3555d67372d630aae` | `9305c69842f0e5a9d755c7f57d4cddd70a492ba6` | `v0.8.0-alpha.3` historical release |
| `S` | `8e9f38cf8d26d131fbb47acbe4f39a40681d34ff` | `2b10a6dd5930372d73cd0e7ec05f277577d2f3a0` | performance-branch program start |
| `E` | `5117c0ae12da2c0346b47061642286174cea3f5f` | `4ebfe46d8f48508ac6489d0bfea09ed469d97746` | instrumentation descendant and source tip used for discovery and sampling |

`production_source_changed: false`

The complete committed `S..E` chain is:

1. `84008389e3a65d7cfb411ee67a63eaf4058fffb0` —
   `docs(perf): plan headless performance hardening`
2. `b00db56c8a9512f220951f8ce66095a948ddd893` —
   `test(perf): bind benchmark output identity`
3. `5117c0ae12da2c0346b47061642286174cea3f5f` —
   `test(perf): enforce pipeline receipt contracts`

This file is committed after `E` in a receipt-only commit. That containing commit is the immutable
U1 receipt boundary; `E` remains the source revision whose harness and outputs were verified. The
split preserves reviewable intermediate commits instead of claiming that `E` directly contains
this receipt.

`S -> E` changes only the committed plan, Criterion benches and fixtures, test-only admission
checks, performance scripts, and the receipt contract. No runtime production source under the
parser, model, layout, render, CLI, binding, editor, or analysis implementations changed. `E`
therefore measures `S`-equivalent production code through a fail-closed harness.

The operator checkout contained pre-existing untracked `rust_out` and `test-results/`. They were
excluded from staging and were not read, modified, removed, or used as evidence.

## Host and build recipe

- Timestamp window: `2026-08-02T13:40:08+08:00` through
  `2026-08-02T14:02:09+08:00`.
- Host: macOS `26.5.1`, `arm64`, Apple M4 Pro. The benchmark JSON records the OS version,
  architecture, and CPU model; build `25F80` is operator-recorded because that schema did not
  capture the macOS build identifier.
- Rust: `rustc 1.95.0 (59807616e 2026-04-14)`, target
  `aarch64-apple-darwin`, LLVM `22.1.2`.
- Cargo: `1.95.0 (f2d3ce0bd 2026-03-21)`.
- Python: `3.14.6`.
- Cargo profile: `bench`. The benchmark JSON records `CARGO_BUILD_JOBS=1`. The operator also set
  `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_BENCH_DEBUG=0`; those two fields are operator-recorded
  because the benchmark JSON schema did not capture them. The verification manifest below records
  the commands that explicitly set them where applicable.
- Each side was built serially into the ordinary workspace `target`, then copied to an immutable
  `target/perf-frozen/<context>/...` path before discovery or sampling.
- Alpha.3 features: `render,cytoscape-layout,elk-layout,ratex-math`, without default features.
- Current features: `svg,layout-cytoscape,layout-elk,math`, without default features.
- Alpha.3 lockfile: 129,717 bytes,
  SHA-256 `7db2df9bfbe2c2f409cea702ad4875cc9b37eeaf97d118eb29d6360bc20e485c`.
- Current lockfile: 135,106 bytes,
  SHA-256 `d8a360a8a3ae88d70d576dba0a359208ce9e79ecc99af4fc3111fb7b6336f5b4`.
- Alpha.3 corpus: 18,268 bytes,
  SHA-256 `4ca25f15124b90764f451e77ad0a2442fc1b69237f2f87ca954ed945b83d03c9`.
- Current corpus: 28,973 bytes,
  SHA-256 `fc23edbf587844d846d13c32a241e6172b52ade52dc13d3d9514586537009dda`.
- Current pipeline source: 23,801 bytes,
  SHA-256 `a38265f1c72b92a8e5f7952d6cc053434685dcdb237c03b0de84913046fdbc31`.
- Receipt contract: `native-criterion-preflight-v1`, 805 bytes,
  SHA-256 `3d1b1922f52d2926c6d741e69138f20e82590471102b3e9686c908433092dad6`.
- Harness: `compare-self-v2`, 158,774 bytes,
  SHA-256 `8b9f66702d5f2ed52142cf02e40c30158459c47604099e2d34736dbf5e547934`.

The minimal current recipe `--no-default-features --features svg` is intentionally incomplete:
discovery reaches Mindmap and fails because `layout-cytoscape` is absent. The complete explicit
headless recipe above is required. Alpha.3's legacy pipeline still skips `mindmap_medium` and
`architecture_medium` even with its complete layout feature mapping; the old harness records only
the skipped names, so those rows remain `execution_failure` rather than being reinterpreted.

## Historical coverage discovery

Command shape:

```text
compare_self.py \
  --base-dir <clean-alpha3> --head-dir <clean-E> \
  --base-features render,cytoscape-layout,elk-layout,ratex-math \
  --head-features svg,layout-cytoscape,layout-elk,math \
  --no-base-default-features --no-head-default-features \
  --base-group end_to_end --head-group end_to_end \
  --suite cross_family --discovery-only --evidence-mode diagnostic \
  --freeze-shared-target
```

The current corpus contains 36 admission records: 35 primary SVG records plus compatibility-only
ZenUML. It is checked directly against the executable admission inventory, and every fixture's
declared family is checked against actual detector metadata.

| Classification | Count | Interpretation |
|---|---:|---|
| `unverified_output` | 22 | Both revisions register and execute the byte-identical row, but alpha.3 has no output receipt. |
| `execution_failure` | 2 | Alpha.3's legacy pipeline skips `mindmap_medium` and `architecture_medium`; current executes both. |
| `current_only` | 12 | Historical corpus/bench registration is absent: Venn, Swimlane, Event Modeling, TreeView, Ishikawa, four Railroad dialect records, Wardley, Cynefin, and Error. |

The discovery report correctly exits with `contract_failure`: it has zero comparable rows under
the output-identity contract. This is an expected fail-closed result, not a benchmark regression.

## Same-tree diagnostic controls

Exact filter:

```text
end_to_end/(flowchart_medium|flowchart_large|flowchart_ports_heavy|class_medium|mindmap_medium|requirement_medium|architecture_medium)
```

Both sides use commit `E`, separate clean worktrees, separate serial builds, and separate frozen
executables. The base executable SHA-256 is
`39673b5b0aaaf4f75085cb90a32de21be4ef139737096c0c01e0784670511cbb`; the head executable
SHA-256 is `2ac1cd9f9b3e195b1afe82861baafddd8728f7c20880efab4229dbac859fb6a8`.
Every one of the 28 exact sampling processes emitted exactly one matching preflight receipt and
one postflight receipt. Both runners passed post-sampling Git, file, executable, corpus, fixture,
and receipt verification.

The table records observed point estimates only. It is a noise/context snapshot from two pairs,
not a performance decision.

| Fixture | Input bytes | Input SHA-256 | SVG bytes/elements | SVG SHA-256 | E-A | E-B | Observed delta |
|---|---:|---|---:|---|---:|---:|---:|
| `flowchart_medium` | 1,474 | `b65d43d4c67df71bc188829344583b977b3c2a805e5649d711d849a7a56d6572` | 75,702 / 683 | `4a2b2d89ef3ffbb67a1524051b95a8658fde9e336798aed3154dc3ec6dfa9f38` | 2,060.050 us | 2,078.250 us | +0.88% |
| `flowchart_large` | 6,419 | `edba73dc4c0a12417e052ec3c98ada82b5d773c4346b5c735df5dfa354f5b161` | 313,587 / 2,902 | `51c479e446aedfd4d13298efc55dca11deba2b60ca82982ed58a088acedec3ae` | 9,885.250 us | 10,003.500 us | +1.20% |
| `flowchart_ports_heavy` | 815 | `ab22e6ae5085a10fc01bb9e7f8d89c26b728b95c84786d7685dc2986f3932217` | 59,750 / 423 | `a457d565b2722b2cec2a4b1ae947037dcbba28ea11966076c6e75b8fe111721c` | 757.350 us | 754.270 us | -0.41% |
| `class_medium` | 995 | `e99fac382b50d6499b96c5793deeb833d0bb92283127bfe17fffc8bbc909b637` | 66,396 / 576 | `69194b8234eeda7edfc2bfe8b92bd0e62099335af843d8c1c49eb430392c99f3` | 661.990 us | 664.485 us | +0.38% |
| `mindmap_medium` | 182 | `a0fe4aa2a0ef2fb356ffbb627026d88c3bf86ea4c3ba17c01e6575bc8e22a8bc` | 36,468 / 179 | `d4ba01ce274b09c32e86f91cb4237eecbc15ac179f5f7ee638529140deea8351` | 166.850 us | 165.070 us | -1.07% |
| `requirement_medium` | 251 | `219c5d15060bc00139be5aab4737f1203badfb8f4ac02ceb582970096ed3f0ac` | 14,269 / 115 | `7fbe3c8a1e846498b8cf1df3093d6a3c159dc98d447ecc06ddf187047c424f08` | 184.330 us | 181.350 us | -1.62% |
| `architecture_medium` | 120 | `5b249b03683c12ec815e81e96120069bd8a5c37eac640d3c5592ce0fb1901830` | 5,019 / 52 | `8aa4741d183f7dc7b58576b3d07d5690a5cd7df22e9e04bc25d41df80824b7e8` | 40.647 us | 40.901 us | +0.62% |

The largest absolute same-tree point-estimate spread is 1.62%. This is diagnostic context only;
later candidate decisions must still run the plan's preregistered A/A calibration and at least
eight fresh balanced confirmation pairs.

## Raw artifacts

Generated artifacts remain under the ignored workspace `target` tree:

- `target/perf-evidence/u1/alpha3-to-evidence-cross-family-discovery.json` —
  SHA-256 `447d269442d0b4c4384f8ae1303860f9d89fe52a0bf671d299ff3a67cdb83907`.
- `target/perf-evidence/u1/alpha3-to-evidence-cross-family-discovery.md` —
  SHA-256 `21b0cbe7fed1e58d03027d2a7001ba55302e93572e10f724d1853bc227f798c7`.
- `target/perf-evidence/u1/evidence-aa-controls.json` —
  SHA-256 `ea20f6185eb20a0169e1b5d3e52dece6f53a357afa6de3cc04c2c21e921dbbeb`.
- `target/perf-evidence/u1/evidence-aa-controls.md` —
  SHA-256 `290e0d4ed427be0959137e9e8da00a283d9db84a578a0e095dc06ad534e1ac04`.
- `target/perf-evidence/u1/alpha3-render-only-to-evidence-cross-family-discovery.json` —
  SHA-256 `3849f9deaa52513d77ef796ce0df4972b5376c6458e5343425a7324b9028c14f`.
- `target/perf-evidence/u1/alpha3-render-only-to-evidence-cross-family-discovery.md` —
  SHA-256 `104274f899f4c3a398ebb3b1414802e79008b4c2ba0120e4c75d06cd6cfaf5c5`.

The committed verification manifest is
`docs/performance/evidence/headless_performance_u1_verification_2026-08-02.json` — 4,594 bytes,
SHA-256 `6d3e63d54961b66911852700316a00cd5d1d8c1130c69ff457cf79505c76a2fc`.
Its raw logs remain under `target/perf-evidence/u1/verification/`; the manifest fixes each command,
source revision, timestamp, exit code, byte length, output hash, and result summary.

## Verification completed

The following checks were repeated at `E` from `2026-08-02T14:49:24+08:00` through
`2026-08-02T14:51:17+08:00` and are bound by the committed verification manifest rather than being
operator-attested prose alone:

- `python3 tools/bench/test_perf_contracts.py`: 100 tests passed.
- `cargo fmt --all -- --check`: passed.
- `git diff --check`: passed.
- `CARGO_BUILD_JOBS=1 cargo check --locked -p merman --features complete-svg --bench pipeline`: passed.
- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p merman --features complete-svg --test pipeline_bench_fixtures`: 1 test passed.
- `CARGO_BUILD_JOBS=1 cargo nextest run --locked -p xtask native_cross_family_corpus_matches_admission_inventory`: 1 test passed.
- `CARGO_BUILD_JOBS=1 python3 tools/bench/verify_pipeline_bench_list.py --target-dir target --features svg,layout-cytoscape,layout-elk,math`: 357 benchmarks, 357 receipts, seven lane groups.
- Direct Criterion exact smoke on `end_to_end/error_basic`: one preflight receipt, one point estimate, and one matching postflight receipt.

## Next use

This receipt freezes U1 only. Candidate latency claims must use adjacent clean `A -> B` commits;
structural and memory candidates must use their owner-local counters or retained-state probes.
`R -> F` remains historical context, and `S -> F` remains the aggregate interaction guard.
