---
type: Verification
title: C7a native candidate and literal reference matrix
timestamp: 2026-09-16
git_commit: a5e3cd2d6e0d848feeb40d2f9407cc163f014ec0
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
---

# Scope

Verified clean source `a5e3cd2d6e0d848feeb40d2f9407cc163f014ec0` for candidate
`v0.8.0-alpha.7` on macOS ARM64. The detached checkout remained clean before and after
collection/replay, including untracked files. The maintainer worktree's existing knowledge
folders were untouched. Cargo builds ran serially and reused the existing target directory.
Tools: Rust/Cargo 1.95.0, cargo-dist 0.32.0, Python 3.14.6; macOS 26.6.2.

This closes the local CLI/LSP archive and qualification tranche at this source. C7a contract
freeze remains open: the remaining installed consumers, host/compiler floors and final preflight
do not inherit this result. No tag, publication, font bundle or budget change was made.

# Verified owner checks

| Check | Result and boundary |
| --- | --- |
| Archive, catalog, qualification and release workflow Python contracts | 135/135 |
| Optimized private theme acceptance | 146/146, zero skipped; library plus all eight integration targets, with PNG/JPEG/PDF and Cytoscape enabled |
| Version projection | Exact alpha.7 coupled dependency and package checks passed |
| Changelog projections | Preparation mode passed; entries remain Unreleased, dated immutable preflight remains open |
| Third-party license contract | Passed |
| cargo-dist local build | CLI and LSP macOS ARM64 archives built from the named clean source |
| CLI archive execution and qualification | Archive/runtime checks passed; 18 native Rust observations matched actual extracted CLI bytes |
| CLI qualification replay | Full recollection and existing-record comparison passed |
| LSP archive execution | Archive checks and native stdio initialize/shutdown passed |

Acceptance includes `c6_runtime`, `route_cutover_runtime`, `native_export_smoke`,
`preset_qualification`, `legacy_projection_retirement`, `block_title_legacy_projection`,
`class_edge_label_background_legacy_projection` and `flowchart_marker_legacy_projection`.
This is not a full workspace or browser test run.

# Artifact identity and qualified scope

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13499304 | `5cfe9ea14a9d87a04a832e7a16f86eb0343b962daf6f1826e3e091139c857ad8` |
| `merman-cli` | 51398256 | `af59a79ee32d31ddc8f2efb9fe92de7fca908a7e93e7fedfb1b64a6b2bd7239b` |
| `merman-lsp-aarch64-apple-darwin.tar.xz` | 4054316 | `83a058ec5d667a7cd7aa029d73d6c11adedc793463e8814340f8105a40dffb29` |
| `merman-lsp` | 17828544 | `18d1352c59b46040982ca7b420778f13a5f3ddd0d9ee4f316f39644e5d4b8244` |

The archive-bound catalog SHA-256 is
`3013d8fe0d0145005f148ab2a3f65ed342f43bfcfa46790bda7693c577256fc3`.
Brutalist, Spotless and Cyberpunk each have six cells: Flowchart, State and Sequence, each as
SVG and PNG, under `native-flowchart-state-sequence-system-fonts-v1`. All cells are
`host_dependent`. The other seven presets retain empty qualification cells. PDF execution in
the separate reference matrix does not grant PDF qualification. C6 observations are a separate
owner and must not be added to these 18 cells.

# Installed Python and Node consumers

The same clean source built a Python UniFFI wheel, a Node native package group for Darwin
ARM64 and a Node WASM package. Each was installed into a separate consumer environment.
The Python module resolved inside the wheel smoke virtual environment; both Node consumers
resolved the installed public package entrypoint. Node used 24.21.0 and npm 12.0.2.

| Installed artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `merman-0.8.0a7-py3-none-macosx_11_0_arm64.whl` | 9228164 | `0984c5540f9470ba9c0c64d15a31b3ddca6cbf842aa9dd5d7a334b6fdd8c0ad3` |
| `mermanjs-node-0.8.0-alpha.7.tgz` | 113593 | `db68b2ffb558051fdd8cd1795daed2f42704973753b25d541b3182af14d6e464` |
| `mermanjs-node-darwin-arm64-0.8.0-alpha.7.tgz` | 11068461 | `37749b16ba589b4c8a9d12a1210b3a4d2c739adabcf485d162f56724cfbb0bb8` |
| `mermanjs-node-wasm-0.8.0-alpha.7.tgz` | 7290115 | `d03a2e00acc9a6128f6dea10702412350a8e45956183a2b3e2eed590dc34f63b` |

The wheel owner passed generated-support and legal checks, shared definitions, three-family
isolation, rule overrides, cold complete specs, preset export/catalog, 22 support vectors and
all three authoring budget operations through one-shot and reusable consumers. An additional
installed-wheel render of the exact Japanese reference example produced 107776 SVG bytes.

Each Node consumer passed two shared definition vectors, two catalog checks, three family
isolation checks, one rule override, one cold-spec roundtrip, three preset exports, 44 support
queries, six diagnostic checks, two resource-limit checks, 23 JSON operations and 23 SVG
renders. Native and WASM results were parsed from actual nonempty smoke JSON, retained as
`node-native-smoke.json` and `node-wasm-smoke.json`. These witnesses do not promote additional
preset qualification cells or prove browser execution.

A validation defect was found during collection: Node canonicalizes `/var` to `/private/var`
on this host, but several script entry guards compared it to the uncanonicalized invocation
path. The first native smoke invocation therefore exited successfully without executing its
body. That empty run is excluded. Both frozen-candidate smoke scripts were then invoked by
canonical absolute paths and their result counters checked. The separate tooling repair
`39b6acee5` compares real paths and tests directory aliases for build, assembly, verification,
installed smoke and benchmark commands. Its Node contracts passed 109/109 and Web script
contracts passed 143/143; these are validator checks, not a claim that the candidate artifacts
were rebuilt at that later commit. The repaired alias invocation also executed the native
installed consumer successfully.

# Literal reference inputs, all ten presets

The same CLI binary rendered all 34 unmodified fences in Modern Mermaid's
`MERMAID_EXAMPLES.md` at `a021cbce37fc0b07a9f4791c28e983101ea06f2d`, using all ten catalog
presets and SVG/PNG/PDF: **1,020 invocations, 990 successful outputs, 30 parse failures**.
Each format had 330 successes. Every failure was example 22, the unquoted Chinese GitGraph
branch names already rejected by both tested Mermaid versions. The repaired Japanese
Flowchart passed all 30 combinations. The CLI executable digest in this matrix exactly
matches the binary inside the verified archive.

The existing diagnostic runner was invoked with its `PRESETS` tuple expanded to the ten
catalog IDs. Execution/signature success uses permissive admission. It proves neither full
facet application nor visual usability; the reference themes' CSS was not supplied here.

A headless Chromium follow-up inspected the first referenced arrow in examples 1 and 5 for
each preset. In Flowchart example 1, edge strokes changed with all ten palettes, while that
arrow retained default paint. Editor Dark had line `rgb(148,163,184)`, marker `rgb(11,11,11)`
and canvas `rgb(15,23,42)`; Cyberpunk had line `rgb(34,211,238)`, marker `rgb(51,51,51)` and
canvas `rgb(2,6,23)`. The inspected Sequence arrows followed the line colors. These are
computed-style observations, not a contrast certification or an exhaustive arrow audit.

This is an open preset-usability gap. Explicit Flowchart Marker fill/stroke is currently
Unsupported, as exercised by `flowchart_marker_theme.rs`; adding a Marker rule to the recipe
alone would not solve it. Review final edge-paint inheritance at the marker writer before
choosing a bounded repair. Qualification above remains limited to its declared recipe facets.

# Reproduction and remaining work

Logs, exact commands, environment, hashes, qualification record/catalog and output files are
under `target/bench/experiments/c7a-a5e3cd2d6/`. `execution.json` identifies the detached
checkout and shared target. The native owner sequence in that clean checkout was:

```console
dist build --tag=v0.8.0-alpha.7 --artifacts=local --target=aarch64-apple-darwin --output-format=json
python3 scripts/verify_cli_release_archive.py <cli.tar.xz> --target aarch64-apple-darwin --version 0.8.0-alpha.7 --repo-root . --execute --preset-qualification-output <record.json>
python3 scripts/verify_cli_release_archive.py <cli.tar.xz> --target aarch64-apple-darwin --version 0.8.0-alpha.7 --repo-root . --execute --preset-qualification-check <record.json>
python3 scripts/verify_lsp_release_archive.py <lsp.tar.xz> --target aarch64-apple-darwin --version 0.8.0-alpha.7 --repo-root . --execute
python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-theme-acceptance --no-default-features --features png,jpeg,pdf,layout-cytoscape --lib --tests --test-threads 2 --no-fail-fast
```

Same-candidate Web, Typst and the remaining native binding witnesses and their shared transport
vectors remain to be refreshed. Python UniFFI and Node native/WASM installation evidence is
recorded above. Linux, Windows, the Swift 5.9 floor and other host-specific
claims remain unverified here. Preserve older records at their original source identities.
The preset marker gap, wider visual/semantic coverage, matched alpha.6 size/performance
comparisons, theme compilation/discovery and large-diagram memory remain audit work.
Current file sizes alone do not establish regression or justify a budget change.
