# Alignment Documentation Authority

[`STATUS.md`](STATUS.md) is the current human-readable dashboard for Mermaid alignment. Exact
admission, fixture, baseline, and comparison facts live in structured code or manifests; when a
prose document disagrees with one of those owners, the structured owner wins and the prose should
be corrected.

## Machine Authorities

| Concern | Owner |
| --- | --- |
| Selected Mermaid and companion sources | [`../../tools/upstreams/REPOS.lock.json`](../../tools/upstreams/REPOS.lock.json) |
| Built-in family and parser capabilities | [`../../crates/merman-core/src/family.rs`](../../crates/merman-core/src/family.rs) |
| Primary SVG matrix and compare commands | [`../../crates/xtask/src/cmd/compare/diagrams.rs`](../../crates/xtask/src/cmd/compare/diagrams.rs) |
| Admission consistency checks | [`../../crates/xtask/src/cmd/admission.rs`](../../crates/xtask/src/cmd/admission.rs) |
| Family fixture admission | Per-family `_baseline-manifest.json` files under [`../../fixtures/upstream-svgs/`](../../fixtures/upstream-svgs/) |
| Theme override behavior oracle | [`../../fixtures/_verification/theme_variables_oracle_12_0_0.json`](../../fixtures/_verification/theme_variables_oracle_12_0_0.json) |
| Deterministic root contracts | [`../../fixtures/_verification/deterministic-root-contracts.json`](../../fixtures/_verification/deterministic-root-contracts.json) |
| Semantic label residuals | [`../../fixtures/_verification/label-geometry-residuals.json`](../../fixtures/_verification/label-geometry-residuals.json) |

`cargo run -p xtask -- check-alignment` verifies canonical family capabilities, compare ownership,
fixtures, semantic/layout goldens, and upstream baseline manifests. It does not parse ordinary
alignment prose or require paired Markdown documents.

## Prose Roles

| Documents | Lifecycle role |
| --- | --- |
| [`STATUS.md`](STATUS.md) | Current human dashboard and verification entry point. |
| [`ADMISSION_INVENTORY.md`](ADMISSION_INVENTORY.md) | Human explanation of the structured admission model; not the model itself. |
| `*_MINIMUM.md` | Family scope and behavior notes, often retaining the source version at first admission. Read their status and source date before treating a claim as current. |
| `*_UPSTREAM_TEST_COVERAGE.md` | Source and fixture evidence reports. Counts and commands may be dated; manifests and executable compare facts remain authoritative. |
| [`GAP_BACKLOG.md`](GAP_BACKLOG.md) and [`MILESTONES.md`](MILESTONES.md) | Active work and maintained exit criteria; neither records a current successful gate run. |
| [`FIXTURE_EXPANSION_TODO.md`](FIXTURE_EXPANSION_TODO.md), [`PARITY_HARDENING_PLAN.md`](PARITY_HARDENING_PLAN.md), and completed admission plans | Historical import or decision evidence; revisit individual claims against the selected source before new work. |

The paired family documents remain useful for source provenance and review, but their filenames,
pair count, wording, and embedded paths are not standing release evidence. Rewording or
consolidating them must not change machine admission unless a structured owner changes too.
