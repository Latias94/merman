# Native diagram-selection size evidence (2026-09-26)

This report records the controlled Windows native measurement for selectable diagram
families. The measurement answers one narrow question: does compiling only Flowchart and
Gantt reduce the final executable size while preserving the selected-family SVG output?

## Method

The harness is `tools/bench/measure_diagram_selection.py`. It archives each committed
revision, builds the same small consumer with the same public `Renderer::render` API and
runtime-fed source files, and records the executable, compiler, dependency lock, fixture,
and corpus SVG hashes. Every build used the fixed release profile (`opt-level = "s"`, thin
LTO, one codegen unit, panic abort, symbol stripping, no debug info, and no incremental
build), Cargo jobs 2, the `x86_64-pc-windows-msvc` target, and the same Windows host and
Rust toolchain.

The pre-change baseline is revision `72c024776`. The reviewed implementation is commit
`278ae4f52`. The baseline uses the historical `svg` feature, the candidate full build uses
`all-diagrams,svg`, and the candidate subset uses `diagram-flowchart,diagram-gantt,svg`.

## Reproduction

Use the current [probe recipe](../../tools/bench/fixtures/diagram-selection/README.md) for a new
comparison. Pin `--fixture-revision` to the same committed probe in all three lanes, independently
of the measured `--revision`; the historical baseline predates the probe. Pin `--toolchain` as well.
The driver now records both selections and uses that toolchain for every Rust command, including
when the evidence directory is outside the repository. The table below records the current controlled rerun for `278ae4f52`; do not combine receipts from
different probe revisions or toolchains.

## Results

| Build | Diagram parser set | Executable bytes | Delta vs baseline |
| --- | --- | ---: | ---: |
| Pre-change baseline | historical `svg` | 9,717,248 | — |
| Candidate full (`278ae4f52`) | all 32 families | 9,725,440 | +8,192 (+0.084%) |
| Candidate subset (`278ae4f52`) | Flowchart, Gantt | 5,411,328 | -4,305,920 (-44.310%) |

The subset is 4,314,112 bytes (44.359%) smaller than the candidate full build. The small
full-build increase is within the expected change from the selectable-family implementation
and is not presented as a size reduction. The baseline and candidate lockfiles differ because
the candidate revision adds the selectable-family package graph; each lane still uses its archived
revision's lockfile, the same Rust 1.95.0 toolchain, release profile, target, probe, and corpus.

## Semantic controls

All three builds rendered the same three corpus inputs (`flowchart_small`,
`flowchart_medium`, and `gantt_medium`). Their input hashes and SVG output hashes are
identical across the baseline, full candidate, and subset receipts. The full candidate
accepted the Sequence control input. The subset compiled exactly the Flowchart and Gantt
parser families and rejected that same Sequence input with exit code 2 and stderr
`merman.parse.unsupported_diagram`.

Receipts:

- `target/bench/diagram-selection/pre-change-full-278ae4f52-review/receipt.json`
- `target/bench/diagram-selection/candidate-full-278ae4f52-review/receipt.json`
- `target/bench/diagram-selection/candidate-subset-278ae4f52-review/receipt.json`

The ignored experiment ledger is
`target/bench/experiments/selectable-diagram-families/experiment.yaml`. The current rerun receipts
are retained under `target/bench/diagram-selection/` and are intentionally not committed.

## Scope and limits

This is evidence for one Windows native executable, one compiler target, one release
profile, and a small selected-family corpus. It does not establish latency, memory,
WebAssembly, mobile, VST, or other platform behavior. Those claims require separate
measurements with their own controls.
