# Comparative Release Reports

Use this guide when a maintainer requests a release comparison or when release notes make performance, size, or capability claims that need supporting evidence. Ordinary release preparation follows [Releasing](RELEASING.md); a changelog update does not require a new benchmark campaign.

## Establish the comparison

Choose an explicit base release and target commit. For incremental prerelease notes, use the preceding published prerelease, verify that it is an ancestor of the target, and compare the final trees. Keep any intermediate publication boundary visible. Consolidate additions and their pre-publication fixes into one user outcome; preserve separate migration notes for behavior users could already consume.

The read-only collector records source manifests, changed paths, first-parent commits, SVG admission records, tool versions, and explicitly supplied artifact sizes:

```console
python scripts/collect_release_facts.py --base v0.8.0-alpha.6 --target HEAD --manifest merman --manifest merman-cli --output target/release-facts.json
```

The collector reads committed revisions, not uncommitted edits. Manifest dependencies are not a resolved dependency closure, and an artifact path or label does not establish which revision built its bytes. Record that provenance separately.

## Use the owning evidence

| Claim | Authority |
| --- | --- |
| Parser identities and family behavior | `crates/merman-core/src/family.rs`, family tests, and the pinned upstream sources |
| SVG admission | `crates/xtask/src/cmd/admission.rs` and the corresponding verification receipts; distinguish record IDs from logical families |
| Cargo selections and release contents | `capabilities/feature-surface-v1.json`, `capabilities/artifact-profiles-v2.json`, manifests, and owner package checks |
| ASCII support | `docs/rendering/ASCII_SUPPORT_MATRIX.md` and runtime `ascii_capabilities` |
| Web and Node products | Platform descriptors, package manifests, transport admission, and exact package-group artifacts |
| Published availability | Direct registry or GitHub Release evidence for each package and channel |
| Performance and artifact size | Raw runs under the owner procedures in `docs/performance/RUNBOOK.md` and `docs/performance/BENCHMARKING.md` |

An artifact profile proves a capability recipe, not publication. Keep independent Typst, Tree-sitter, and extension versions separate from the workspace version. Report source declarations, measurements, and interpretation separately.

## Measure only the claims being made

Select workloads relevant to the requested comparison. Record full commits, target, host, toolchain, profile, features, lockfile and corpus digests, warmups, iterations, concurrency, and the raw commands/results. Reuse existing benchmark owners instead of adding a second harness.

Compare both product defaults and equivalent capability selections when a release changes defaults. Added ELK, math, or diagram support changes the product contract; it is not by itself evidence of an implementation regression. Compare raw, stripped, compressed, and packaged sizes only when the corresponding units and capabilities match.

For rendering comparisons, separate parse/semantic work, layout, SVG emission, and end-to-end rendering when the harness exposes them. Compare Merman, Mermaid.js, or another renderer only on equivalent inputs and output contracts with verified semantic/structural results. Browser text measurement and deterministic headless measurement remain different workloads. Keep Node N-API and Node WASM transport measurements distinct.

Label missing results as unavailable, blocked, or inconclusive. A budget is not a measurement, an old receipt is not a current run, and one host does not establish every platform. Preserve a failed locked build rather than silently changing its historical source or lockfile. Record a reproduced regression and its remaining constraints in `docs/performance/PERF_PLAN.md`.

## Present the result

Lead with the compared range and the user-visible result. Include only affected surfaces in a compact table with base state, target state, user consequence, and evidence. For package recommendations, state the exact feature or package selection and the operations it omits; derive choices from [Package Surfaces](PACKAGE_SURFACES.md) and [Features](../FEATURES.md).

Keep required migration actions prominent. State unmeasured targets, font-dependent behavior, parity failures, or unpublished channels alongside the claim they limit. Extract concise outcomes into the root changelog, give each outcome one detailed home, and project only relevant changes into package changelogs. Preserve verified external contributor credit and keep each Markdown paragraph or bullet on one physical line.
