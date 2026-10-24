---
type: "Work Progress"
title: "Corrected hosts and migration documentation audit"
description: "Corrected installed hosts and the actual Windows CLI archive pass; five migration-document defects are corrected while registered timing begins."
timestamp: 2026-10-22T19:33:04Z
record_id: "c3ccb0eec6634cf3a9829ce74d218f6f"
producer_id: "codex-root"
run_id: "semantic-capability-boundaries-u9"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "f0365e6a4948c4baff2f16adb735e0f143ee38c6"
supersedes: "f31d024a42c44bda8915cba1e6e13a"
---

# Summary

The complete U1-U9 objective remains active. No push, PR, merge, publication or release.
Corrected final-source integration session 50912 / PID 64288 ended with exit zero after all
27 serial gates. This is separate from the failed historical strict command.

# Details

Final evidence root:
`F:/codex-builds/merman-semantic-capability-boundaries/evidence/final-f0365e6a4-20261023/`.

- Semantic and same-capability SVG size controls pass at the corrected product revision,
  with equivalent outputs: 25.2251% and 0.3374% smaller respectively.
- Node N-API and Node-WASM installed smoke, Typst package preflight, Windows GNU/Dart ABI,
  capability/service/theme smoke, all five Web packages, DOM safety and mount 10/10 pass.
- Web artifacts are copied outside target: 447 package and source-input files. The receipt
  is `web-host-verification.json`, SHA-256
  `3ee7c89efddfbef27ef141cd56da5f77c0c96157c5f01f147e7c983cdfad924e`.
- cargo-dist 0.32.0 built actual local Windows CLI/LSP ZIPs. The existing CLI archive verifier
  executed the CLI ZIP; the exact retained binary advertises math and renders a required
  formula. `cli-archive-math-receipt.json` SHA-256 is
  `227d36412a7ed7fc610084285e4b0bde1e8fe2c7171eb9fc984e3526a1db9648`.
  `final-integration-receipt.json` freezes all gate logs, SHA-256
  `25a9d0a04e9a399db5276c6c21cf1f4cd945e509529f3e73096afb08a61aca3f`.
- Independent selected parser report/corpus/request/decoder/runtime were copied from the
  external target, with matching harness and compact-corpus identities. No new oracle run:
  the retained execution remains 881b2be08, 728 cases and zero mismatches.

Read-only reviewer `01a1650d-4181-7143-89e5-9cc3e3f0e99e` found five remaining documentation
defects: examples pointed to old published APIs; Gantt and calibration recipes omitted
compat-json; binding math defaults were unconditional; two accepted ADRs lacked retirement
scope. All are corrected against actual manifests and runtime defaults. Bounded document
assertions and diff whitespace checks pass; no product source or measurement recipe changed.
The reviewer is closed and ran no Cargo.

Whole-bundle memory validation still fails on pre-existing missing `type` frontmatter in
older records, with additional legacy/rollup warnings. Its output is retained as
`memory-validation.log`; those unrelated historical records were not edited. The two new
progress shards are validated separately before committing.

`docs/reviews/semantic-capability-boundaries-verification.md` joins U1-U9, R1-R16, AE1-AE12,
each named verification gate and its scope. Platform aggregate remains deferred to existing
CI owners because installed Flutter bundles Dart 3.9.2; standalone Dart 3.11.6/native evidence
does not prove Flutter UI or Android/Apple runtime checks.

# Next Action

Timing waiter session 48174 / PowerShell PID 59744 observed successful integration and the
exact archive math check, then started the unchanged `run_timing.py`. At this checkpoint it
is building U3 discovery. All six registered timing checkouts are clean and frozen runner
hashes match. Do not restart a slow/live schedule. Shared timing target is
`F:/codex-builds/merman-semantic-capability-boundaries/timing-target`; no other Merman Cargo
queue may use it concurrently. Other-project Cargo is present and left untouched.

Retain all 72 registered rows: U3/U4/integrated, four groups each, six fixtures. generic_v2,
eight A/A pairs, max32 fresh ABBA, 30 Criterion samples, two-second warmup, three-second
measurement, 10% AND 50,000 ns, 95%, seed0 and bootstrap10000 remain unchanged. No timing
admission is established yet. Keep historical inconclusive results and all new outcomes.
Product HEAD no longer needs to stay fixed for integration; documentation can now be committed
with precise owned paths while timing uses its clean independent checkouts.

# Citations

- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md`
- `docs/reviews/semantic-capability-boundaries-verification.md`
- `docs/performance/semantic_parser_embedding_2026-10-22.md`
- `docs/performance/semantic_boundary_adjacent_work_2026-10-22.md`
- Final evidence `completion-audit-hosts-and-docs.json`, `acceptance-example-audit-hosts.json`,
  `protected-inputs-source-bridge.json`, `documentation-corrections-verification.json`,
  `scoped-retention-addendum.json`, `parser-differential-retention.json` and `timing-readiness.json`.
