---
type: Audit Report
title: Alpha.7 presentation theme audit and C7a boundary
timestamp: 2026-09-20
git_branch: refactor/presentation-theme-model
source_commits: 0f5125e75, 0c1b1047a7453b5668e371da38019b20b9c66815, ff9b9991bb91327b1630f326bc389ccc3bdb89fa
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
tags: theme,audit,c7a,alpha7
---

# Decision

This record is a current-source audit, not a release approval. The completed evidence is enough
to retain the complete Cyberpunk recipe and its bounded native qualification profile, but C7a
remains open. No tag, publication, push or public catalog promotion was performed.

The strongest current result is a clean-source native qualification replay plus installed Python,
Node Darwin ARM64 and Node WASM recipe exchange. The evidence is execution-local and host-bound:
macOS ARM64, system fonts, selected SVG/PNG scenes and the installed consumer profiles described
below. It does not establish the release matrix or the U10 cost gate.

# Completed evidence

The [complete-scene record](2026-09-19-cyberpunk-controlled-scenes.md) replaces the obsolete
palette-only Cyberpunk qualifier with `native-cyberpunk-full-scenes-system-fonts-v1`. The unchanged
Flowchart, Sequence and XY Chart fixtures pass the sealed SVG checks and 110 native PNG/PDF pixel
contribution probes. The profile produces six execution-local HostDependent cells. The production
CLI matches all 18 scoped SVG/PNG observations, and a detached clean checkout records and replays
the same qualification object byte-for-byte. The record binds source `a4a04ef1b`, the lockfile,
qualification executable, CLI and scene record digests.

The [installed-consumer record](2026-09-19-installed-theme-recipe-consumers.md) rebuilds the Python
wheel, Node Darwin ARM64 package and Node WASM package from clean source `0c1b1047a`. The direct
versioned recipe is accepted after JSON save/load, complete Cyberpunk files move between separate
Python and Node processes, and all three transports render identical SVG bytes for Flowchart,
Sequence and XY Chart. Node contracts pass 109/109 under Node 24.21.0/npm 12.0.2; focused Python
owner tests pass 27/27. The same tranche repairs the standalone Node lock and regenerates legal
projections without changing the intended capability recipe.

The current literal Modern Mermaid matrix remains useful input coverage: 510 rows across SVG,
PNG and PDF, with 480 successful outputs and 30 input-specific parse failures. The failures are
classified as upstream-invalid or parser-specific before theme evaluation. The matrix measures
execution against literal inputs; it does not qualify every theme facet or every output family.

# Preset and capability boundaries

Brutalist and Spotless retain their earlier six native Flowchart/State/Sequence cells. Cyberpunk
now has six complete-scene Flowchart/Sequence/XY cells under the named native profile. The other
seven public preset IDs have no current qualified cells. All ten presets have execution evidence
on representative literal inputs, but that is a usability and identity observation rather than a
portfolio-wide qualification. Earlier inspections still flag dark arrowheads and Cyberpunk
Mindmap label contrast for follow-up.

The public catalog keeps qualification cells empty. Discovery responses remain a coarse capability
surface: the earlier inventory recorded 188 Conditional, 343 Unsupported, 99 Unverified and 4,023
NotApplicable SVG responses, with binary-output visual queries generally Unverified. These counts
must not be converted into a support percentage or a rendered-terminal claim.

# Artifact and legal observations

The final installed artifacts are macOS ARM64 observations: Python wheel 9,042,331 bytes, Node
root package 113,927 bytes, Node native package 10,541,620 bytes and Node WASM package 7,032,865
bytes. Their embedded native/WASM payloads are recorded with SHA-256 values in the installed
consumer record. These sizes are current artifact identities, not matched alpha.6 deltas or a
budget change.

The 13 Rust license reports pass generator `--check`; release legal projections pass for 382
files; third-party and package legal checks pass for 24 governed packages; representative
dependency-closure verification passes for 34 profiles. The generated Android legal copy now
matches its source report. These checks cover manifests and generated legal material, not builds
on every declared target.

The pinned cargo-dist `0.32.0` macOS ARM64 binary was downloaded to `/tmp` and verified against
its release checksum (`aa343b2ff78ec2981f17a65140250c5ad6062c74072163f68c5c2686d94763a7`). Its
plan passed the repository's release artifact-bundle verifier for the `macos-15` native
`aarch64-apple-darwin` route. A serial local build from source `0f5125e75` produced and verified
both native archives with the real CLI and LSP smoke contracts:

| Archive | Packed bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13,745,380 | `a5a132d375f8d04a785898bd75c6adcc960dd9e85afd3cbfdd079f0e00eb15bd` |
| `merman-lsp-aarch64-apple-darwin.tar.xz` | 4,079,896 | `201313d1b706752bd62b2ec90197f2d2bb07d6a94e2efb97ae2d41328ef106ea` |

The archive checks used `scripts/verify_cli_release_archive.py` and
`scripts/verify_lsp_release_archive.py` with `--execute`, including the generated sidecar
checksums. These are current-source macOS ARM64 results; they do not imply Linux, Windows or
Intel archive execution.

# Performance and footprint status

The published alpha.6 versus old local candidate table in the impact audit remains a non-matched
toolchain comparison. It shows package growth, including the CLI, Python and Node artifacts, but
does not assign that growth to the theme work or justify a budget increase. The current-source
Criterion cold-parse run is retained at
`target/bench/experiments/theme-perf-current-ff9b9991b/cold-parse-summary.json`:

| Fixture | Criterion interval | Output bytes |
| --- | --- | ---: |
| architecture_medium | 8.1591–8.2323 µs | 23,877 |
| mindmap_medium | 16.950–17.094 µs | 29,734 |
| class_medium | 78.824–79.940 µs | 11,083 |
| flowchart_medium | 183.57–185.09 µs | 24,373 |

The comparison field is explicitly `unverified` because `repo-ref/mermaid-rs-renderer` is absent.
The native large-diagram memory run was not executed because its owner would clean the shared
target and launch 60 subprocesses. Matched alpha.6 latency, cold start, memory, themed/native
workloads, PNG/PDF throughput, compile/discovery cost and same-source archive size comparisons
therefore remain unverified. The declared U10 rule still treats unavailable metrics as unverified;
no default budget or limit was changed.

# Open C7a gates

The following evidence is still required before C7a can be marked eligible:

- Same-source CLI/LSP archive assembly and replay for every declared host. The macOS ARM64
  archive route now passes locally; Linux, Windows and Intel archive execution remain open.
- Installed Web, Typst, Native C/UniFFI and relevant mobile/Apple/Flutter consumer journeys,
  including the six public customization/resource journeys and explicit resource failures.
- Linux/Windows native and compiler-floor results; the current package evidence is macOS ARM64
  only.
- Portable-font, missing-font and controlled-font coverage beyond the named host-dependent cells;
  browser, PDF and HTML labels retain their documented host/resource boundaries.
- Matched U10 performance and footprint evidence, including the large-memory workload and binary
  export throughput. Existing alpha.6 comparison numbers remain historical context.
- Broader preset/application review and non-Cyberpunk consumer cases before any catalog-cell
  promotion. A successful render on a literal input does not imply a complete reference design.

# Reproduction anchors

Use the two linked verification records for the exact qualification and installed-consumer
commands. Current source-only cold parsing is reproduced with the four `cargo bench --locked`
commands recorded in the experiment logs. Run the legal checks with:

```console
python3 scripts/generate-rust-license-report.py --check
python3 scripts/sync-release-legal-materials.py --check
python3 scripts/verify-third-party-licenses.py
python3 scripts/verify_crate_package_legal_materials.py
python3 scripts/verify_artifact_dependency_closures.py --representative-targets
```

The ignored experiment directories contain package receipts, smoke JSON, exchanged recipe files,
SVG hashes and logs. They are diagnostic evidence and are not release assets. This audit does not
change the public schema, qualification thresholds, package budgets or catalog qualification.
