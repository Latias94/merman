# Theme retirement and declaration deduplication — 2026-10-08

This is the historical October 8 implementation record. The retained authorization ledger and
Release Preflight chain described below were retired on October 9; they are not current gate
instructions. See the [retirement record](../knowledge/engineering/verification/2026-10-09-theme-retirement-gate-removal.md)
and [current verification owners](../rendering/diagram-theme-coverage.md#current-verification-owners).

## Outcome and scope

This implements the evidence-backed candidates from
`docs/plans/2026-10-08-001-fearless-theme-de-duplication-and-retirement-plan.md`.
The starting revision is `b78ea726e`; the runtime retirement commits are
`09d731ff2`, `a126543e4`, and `e4755d1b7`. The evidence-storage commit is `2d7973fd9`.
No theme appearance, output admission, public wire contract, or native validation policy is
intentionally changed. No latency or artifact-size improvement is claimed.

| Area | Disposition | Result |
| --- | --- | --- |
| Executable test bridge | Removed | Cache, overlay/provider builder, parse harness and tests of the empty bridge stub are gone; 4,220 lines net removed across bridge and unused retirement helpers |
| Retirement observation | Retained and concentrated | Seven bridge tests cover real family-program compilation, all historical routes, route reintroduction, and canonical inventory digests |
| Historical retirement manifests | Retained | Release Preflight still compares 80 independent historical rows and 160 value probes; deriving them from the current matrix would weaken the oracle |
| Public identifier catalogs | Deduplicated | Known targets/facets/base properties reuse existing enum parsers; independently authored claims and full matrix reconciliation remain; 78 lines net removed |
| Mechanism matrix | Separated | Runtime classification is 2,707 lines, private audit is 574 lines, and exhaustive tests are 6,033 lines; moving tests is not a code-size optimization |
| Raw performance evidence | Losslessly archived | Original 1,099,801 bytes retained in an 88,241-byte deterministic gzip; original path is a 187-line summary; raw receipt restores exactly |
| Cross-layer tests | Preserved | Semantic classification, SVG terminal evidence, native admission and public consumers protect distinct contracts; only obsolete empty-bridge tests were retired |

## Why the entire retirement ledger remains

At the recorded revision, the executable legacy route inventory was empty, but that alone did
not prove the historical migration contract. The consumer chain at that revision was:

```text
Release Preflight
 -> scripts/run_theme_acceptance.py
 -> ktd23_authorizes_the_independent_full_retirement_inventory
 -> authorize_legacy_projection_retirements
 -> renderer-owned inventory and current program probes
 -> independent historical projection rows and fixed digests
```

The final observations can be removed when that release contract is deliberately retired.
They do not install a production overlay provider. A future provider must add executable
observation; the current empty dispatch snapshot cannot discover an unrelated new provider.

## Verification ownership

- Known identifiers come from `ThemeTarget`, `ThemeRuleFacetV1`, and
  `ThemeSupportBaseTypographyPropertyV1`; malformed identifiers retain the existing result.
- Public support claims remain independent of the private matrix and are reconciled exhaustively.
- Audit traversal is excluded from normal builds via the existing test/private-acceptance cfg.
- Native filter/glyph containment, final SVG validation, cancellation and resource checks are
  unchanged; they are not substitutes for support discovery.
- Historical output observations are retained rather than regenerated from the current writer.

## Validation

- Normal full-feature renderer library: 2,874 tests passed, two existing manual tests skipped.
- Focused matrix and support suite: 98 passed before final bridge test retirement.
- Final bridge, projection retirement and tombstone suite: 15 passed.
- Independent historical retirement integration: 4 passed, fixed digests unchanged.
- Full private acceptance library with all diagram features: 53 passed.
- Rust facade authoring, layering and operation isolation: 20 passed.
- Production CLI all-target/all-feature Clippy with warnings denied: passed.
- Published acceptance boundary: passed, including an independent external consumer and rejected
  imports of private acceptance types and retired public theme types.
- Raw evidence decompression equals the pre-refactor Git bytes; JSON equality and both archive
  and uncompressed SHA-256 checks pass.
- Formatting and diff whitespace: passed.

A standalone private acceptance invocation without diagram features initially failed because
Flowchart was unavailable. The corrected recipe enables `merman/all-diagrams,merman/layout-elk,
merman/math` and passes all 53 tests; no test was skipped to hide missing capabilities.
Browser behavior and native export algorithms are unchanged; their earlier owner results remain
historical evidence, not new claims for this checkout. Remote CI owns the full workspace,
platform, browser, and security matrix after the branch is pushed.
