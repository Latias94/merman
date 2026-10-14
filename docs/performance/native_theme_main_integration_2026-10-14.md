# Native theme repair: Mermaid 12.2 main integration, 2026-10-14

The maintainer selected current main after the native repair checkpoint. This integration merges
`768370e0842dbd8df24d477fdbbf3449eb61de3d` into the repaired branch at
`95d0e452021edb3b6a7e1be25e5d5eddca382831`. Main supplies the admitted Mermaid 12.2 graph and CLI
distribution changes. The original [repair report](native_theme_repair_2026-10-14.md) remains a
record of its frozen Mermaid 12.1 candidate; its timings and platform results do not certify this
integration.

## Reference and source ownership

The selected Mermaid source is `1fb7869866dd619168d840291c9c54b4f129758b`. Main's selection
decision, package locks and baseline manifests remain authoritative. Locked reference and
Playground dependency trees were installed and checked. The existing generator produced runtime
theme schema 3 with 11 prepared constructors and 67 oracle cases, and audit schema 2. Materialized
source, main-base reference, generated theme and Rustdoc checks pass.
The fourteen-owner `verify-generated` gate and `check-alignment` also pass on the merged source.

The merge retains typed themes, prepared consumer facts, bounded output, cancellation, terminal
evidence, and one XML serialization boundary. Main's new behavior is integrated at those owners:

- Mindmap keeps depth colors, HTML/SVG label selection and font-dependent sizing; layout and
  emission share prepared font ownership and source normalization.
- Circle geometry and stronger RoughJS work estimation live in the shared path owner. Each ring
  charges once, reserves output before generation, and preserves random seed and checkpoint order.
  C4 does not acquire a Flowchart feature dependency.
- Agentflow keeps three source-backed self-loop helper segments. A key carries the semantic edge
  owner plus its segment; label, style, cache and document-ID consumers use the appropriate
  identity. Incomplete, duplicate, mixed and misbound occurrences fail closed.
- Usecase prepares class-derived animation before inline styles, preserving the new Neo marker
  rule without rebuilding the retired compiled-style owner during emission.
- Swimlane keeps source order, terminal detours, title rails, clip behavior and actual layout
  engine selection. New tests locate semantic data IDs within canonical document namespaces.

Four independent Codex review participants found no retained source defect. Separate reviews cover
the new private Swimlane test adapter, Circle selectors and generated layout snapshots. Review
receipts describe inspected bytes; execution results are recorded separately.

## Derived output and exact residuals

`OUT` means `target/bench/experiments/native-theme-repair-20261014/`. Original receipts are retained.
The first merged workspace test run reached 13,182 passes, eight failures and 17 existing skips.
The eight failures identified four new Circle tests using retired raw IDs, one layout-snapshot
test covering two new fixtures, and three parser-diagnostic tests with stale local receipts.

Circle tests now use scoped semantic selectors and preserve geometry, ring order, random seed,
stroke and work assertions. The existing layout generator changed exactly three Swimlane title
width fields: `140.563` to `140.56`, `85.125` to `85.12`, and `89.047` to `89.04`. All other geometry
is unchanged. The prepared title owner retains the measured value; the retired Markdown route
quantized to 1/64 pixel before snapshot formatting. No production geometry or tolerance was tuned.

Main's exact full structure/parity/parity-root SVG comparison passes on its unmodified source.
The initial merged run retained 77 exact local signatures already reviewed on the repaired branch.
Ten parser receipts were updated from actual output bytes after proving the only change was the
Mermaid version footer, including an exact byte check of the corresponding upstream SVGs. Main's
12.2 input hashes, upstream hashes, residual membership, comparison modes and precision remain
unchanged.

Accepted parser diagnostics remain non-parity evidence.

Independent actual-output comparison explains the remaining eleven composite signatures. Nine
Flowchart fixtures have identical element topology, namespaces, order, visible text, comments,
entity sequences and every geometry attribute, including path data. Their differences are canonical
IDs, corresponding resource references and node semantic data attributes. All 128 local references
continue to target the same element, and merged IDs are unique. State fixtures 039 and 040 add 25
and 28 empty style attributes through the existing prepared-style writer. Removing only those
identified additions reproduces every byte of the independently rendered main output. Neither
case changes geometry, text, existing styles or marker references. These are receipt updates for
reviewed composition; the existing comparator remains the signature authority. The fresh merged
release gate passes all three modes with root reports (`main-merge-final-release-svg-a3.json`).

## Verification boundary

Representative independent C4, Block and Agentflow renderer feature builds pass. Reference runtime
tests pass 8/8, and the new KaTeX dependency-resolution contract passes. Workflow and publisher
contracts pass 138 tests; actionlint 1.7.12 and zizmor 1.30.1 pass for the changed workflow owners.
Workspace/all-targets/all-features Clippy with `-D warnings` and format checks pass. The focused
retest passes 18/18, including all eight earlier failures and root-contract controls. The original
workspace aggregate remains recorded with its eight failures; it was not relabeled as a new full
green run.

The current private theme acceptance owner passes 3,105/3,105 tests with two existing skips. This
uses the workspace-only acceptance configuration and the exact four-library recipe, rather than
reusing the earlier 12.1 acceptance count.

Legal source reports pass for all thirteen reports, release projections pass for 424 files, and the
third-party contract and eighteen legal-generator tests pass. The
[integration evidence index](evidence/native_theme_main_2026-10-14.json) records actual command,
log and review identities.

The two new engineering-memory records pass the scoped validator. Repository-wide memory
validation still reports 76 legacy missing-type errors; all affected files are byte-identical to
the pre-merge branch. Those historical records and shared rollups are preserved.

The independent main render reused the Cargo target directory. The subsequent merged release build
incorrectly reused main's core artifact and rejected the branch's typed API. Its failed log remains
recorded. Three parent-produced 16-byte Cargo freshness records were identified through source
dep-info, backed up and invalidated. A subsequent missing transitive ASCII crate identified stale
downstream metadata, so 37 freshness records for nine downstream workspace packages were also
backed up and invalidated. Source, compiled libraries, executables and frozen benchmark captures
were preserved. The passing final release gate uses the current branch rebuilt at those owners.
Debug tests and Clippy were built from the current branch separately.

This integration does not rerun the frozen performance experiment or admit a new speedup. Its
Mermaid 12.1 result remains 14/14 corrected-control non-regressions, no admitted performance wins,
and ten confirmed regressions among 13 comparable old-main cases. The residual old-main cost is
not fully attributed. The original Binaryen 133 assertion failure and the separately identified
temporary upstream-fixed tool result remain explicit in the repair report. Fresh Linux parity,
cross-platform and Web/Playground owner jobs belong to the updated PR's CI; earlier 12.1 platform
results are not relabeled as 12.2 validation.
