# Presentation Theme C7a/C7b Replan

Date: 2026-09-15

## Decision

Keep the current presentation-theme architecture and treat the merged `origin/main` baseline as the integration point. Do not expand the proof framework or add new compatibility abstractions until a concrete contract gap is demonstrated.

C7a is the release-facing track. C7b is the long-tail migration track and does not block the C7a contract freeze unless it exposes a shared public-model defect.

## C7a: release-facing closure

1. **Post-merge regression gate**
   - Run formatting, CI workflow contract tests, theme acceptance, support discovery, retirement tests, and the representative native export checks from the same commit.
   - Record the exact source SHA and artifact profile in the acceptance receipt.

2. **Public discovery and catalog promotion**
   - Complete first-party discovery for Rust, Web, Node, Typst, UniFFI, and Native C ABI.
   - Bind each qualified catalog cell to the artifact profile actually built and tested.
   - Keep unknown `profile_id` and `admission_status` values fail-closed.

3. **Cross-transport authoring contract**
   - Add one shared invalid-input golden case covering Web, Node, Typst, UniFFI, and Native C ABI.
   - Compare error envelope, path, details, and admission status; do not create a second validation framework.

4. **Budget decision**
   - Keep the current budget unless a measured artifact exceeds it on the merged source.
   - If a budget is exceeded, amend it with a recorded size breakdown, ownership of the increase, and a proof that the resulting package remains reasonable.
   - Do not embed Inter or another product font merely to satisfy visual fixtures. Use host or consumer-provided fonts unless a product requirement establishes a distributable font obligation.

5. **Freeze criteria**
   - All C7a qualification cells are non-empty and tied to current artifacts.
   - Public support discovery agrees across transports.
   - Release preflight and package-content checks pass from a clean checkout.
   - No open P1 correctness or CI coverage gap remains.

## C7b: long-tail migration

1. Migrate the remaining Block legacy routes and verify the provider, historical probe, and migration ledger entries have no production consumers.
2. Retire only the runtime compatibility code. Preserve the independent historical retirement authority as audit evidence until the migration record is finalized.
3. Reconcile the coverage document so historical Class cutover failures are clearly separated from completed Class retirement.
4. Reassess remaining family breadth after Block. Do not create new family bridges to improve route counts.

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
4. Complete Block retirement as the first C7b tranche.
5. Revisit deferred performance work only when a benchmark shows material impact.
