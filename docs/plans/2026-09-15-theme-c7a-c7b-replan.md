# Presentation Theme C7a/C7b Replan

Date: 2026-09-15

## Decision

Keep the current presentation-theme architecture and treat the merged `origin/main` baseline as the integration point. Do not expand the proof framework or add new compatibility abstractions until a concrete contract gap is demonstrated.

C7a is the release-facing track. C7b is the long-tail migration track and does not block the C7a contract freeze unless it exposes a shared public-model defect.

## Verified starting point

The merge commit is `3e71378be` (remote main `4c2ac7817`); post-merge follow-ups
through `15e8ecbd5` are not full-platform release evidence. Before freezing, audit the
conflict resolutions against both parents, including binding generations, public docs,
SVG pipeline behavior, dependency changes, and generated license projections.

Block generic Text cutover and production provider retirement already landed before the
merge. KTD17 v89 records 506 routes and 814 route-profile witnesses; the executable Legacy
inventory is empty. The bridge, historical probes, and tombstones are restricted to unit
tests or internal acceptance cfg. The [coverage snapshot](../rendering/diagram-theme-coverage.md)
and [provider retirement verification](../knowledge/engineering/verification/2026-09-14-theme-provider-retirement.md)
distinguish this completed migration from remaining family breadth. Revalidate the
retirement after integration; do not implement it again.

Shared authoring errors, support discovery, and qualification-cell vectors already exist
under `crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/`. Installed consumer
records cover earlier source revisions. Check their current transport consumers and
fill demonstrated gaps rather than creating another golden format.

## C7a: release-facing closure

1. **Post-merge regression gate**
   - Run formatting, CI workflow contract tests, theme acceptance, support discovery, retirement tests, and the representative native export checks from the same commit.
   - Record the exact source SHA and artifact profile in the acceptance receipt.

2. **Public discovery and catalog promotion**
   - Complete first-party discovery for Rust, Web, Node, Typst, UniFFI, and Native C ABI.
   - Bind each qualified catalog cell to the artifact profile actually built and tested.
   - Keep unknown `profile_id` and `admission_status` values fail-closed.

3. **Cross-transport authoring contract**
   - Re-run existing shared invalid-input golden cases through Web, Node, Typst, UniFFI, and Native C ABI; add cases only for uncovered contract behavior.
   - Compare error envelope, path, details, and admission status; do not create a second validation framework.

4. **Budget decision**
   - Keep the current budget unless a measured artifact exceeds it on the merged source.
   - If a budget is exceeded, amend it with a recorded size breakdown, ownership of the increase, and a proof that the resulting package remains reasonable.
   - Do not embed Inter or another product font merely to satisfy visual fixtures. Use host or consumer-provided fonts unless a product requirement establishes a distributable font obligation.

5. **Freeze criteria**
   - Each declared C7a candidate scenario has qualification cells bound to its actual artifact. Unqualified presets and targets keep empty cells; host-dependent evidence never implies Portable admission.
   - Public support discovery agrees across transports.
   - Release preflight and package-content checks pass from a clean checkout.
   - No open P1 correctness or CI coverage gap remains.

## C7b: long-tail migration

1. Revalidate the completed Block route/provider retirement against the merged code, including the independent retirement authority and production cfg boundary.
2. Preserve historical probes and migration records in acceptance-only code; remove a helper only after confirming it has no remaining consumer or audit obligation.
3. Keep historical Class cutover failures separate from completed Class retirement, as the current coverage snapshot does.
4. Inventory remaining family mechanism breadth independently of the zero Legacy route count. Do not create new family bridges to improve route counts.

## Deferred unless measured

- XY Chart terminal-cache copies and receipt escaping.
- Block marker/edge XML receipt cost.
- SystemOnly font database cloning.
- Any broad proof-engine or family-trait rewrite.

These are optimization candidates. Each requires a workload, a before/after measurement, and a semantics-preserving change before entering the release critical path.

## Working sequence

1. Finish post-merge regression evidence.
2. Close C7a discovery, catalog, cross-transport golden, and profile matrix gaps.
3. Freeze the C7a contract and publish the evidence record.
4. Close the Block retirement audit and continue the remaining C7b family breadth work.
5. Revisit deferred performance work only when a benchmark shows material impact.

## Integration verification and next repair

On the working tree based on `15e8ecbd5`, the following checks ran after restoring
preflight qualification generation, replay, and catalog-companion upload:

- Workflow contracts: 87 tests passed (`test_ci_plan`, `test_release_workflow_security`,
  `test_ci_workflow_android_emulator`, `test_fuzz_config`); actionlint passed for
  `release-preflight.yml`. The added shell test uses recording verifiers to exercise
  x86_64 qualification, ARM64 smoke, and failure propagation; it does not build archives.
- Internal acceptance manifest digest: one test passed (121 excluded by the filter).
- Release retirement: 10 tests passed across all four integration targets, with no skips.

The Rust commands used `scripts/run_theme_acceptance.py` to enable the internal cfg.
Direct Cargo invocations without that cfg ran zero tests and supply no acceptance evidence.

The UniFFI mismatch found at `acffcfe1b` is repaired by `a1a8e407d` in the API 9 integration tranche:
ASCII requested/effective layout metadata and the theme-authoring error envelope now coexist,
and Apple/Python projections use the new probe. Old API 7/8 probes stay absent.
The restored consumers exercise both Auto layout branches and theme errors. Local checks:
60/60 Rust binding tests, 7/7 generator tests, 29/29 Python contracts, Swift and Python native
smokes, native ABI and shared binding contracts, and Rust formatting. Swift 6.3.2 on macOS
ARM64 compiled and linked the checked-in generated source against the `apple-uniffi-native`
host library. Python used a freshly generated package from that same library; this is not
an installed `python-uniffi-native` wheel or a full XCFramework/Swift 5.9 matrix claim.
The repository platform verifier also passed: Android target clippy, Dart ABI checks and
analysis, and rebuilt Flutter Native Assets authoring/support/resource smokes. The actual
UniFFI native library exports API 9 and rejects API 7/8 symbol lookup. These checks do not
claim Android device execution or all published platform packages.

Next, review post-merge lockfile churn and regenerate legal projections, then finish auditing
both merge parents and rebuild the final package profiles. Linux archive execution, the full
platform matrix, and C7a contract freeze remain open.
