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
`eb1d5aca1`. The baseline uses the historical `svg` feature, the candidate full build uses
`all-diagrams,svg`, and the candidate subset uses `diagram-flowchart,diagram-gantt,svg`.

## Results

| Build | Diagram parser set | Executable bytes | Delta vs baseline |
| --- | --- | ---: | ---: |
| Pre-change baseline | historical `svg` | 9,723,392 | — |
| Candidate full | all 32 families | 9,727,488 | +4,096 (+0.042%) |
| Candidate subset | Flowchart, Gantt | 5,410,304 | -4,313,088 (-44.358%) |

The subset is 4,317,184 bytes (44.381%) smaller than the candidate full build. The small
full-build increase is within the expected change from the selectable-family implementation
and is not presented as a size reduction.

## Semantic controls

All three builds rendered the same three corpus inputs (`flowchart_small`,
`flowchart_medium`, and `gantt_medium`). Their input hashes and SVG output hashes are
identical across the baseline, full candidate, and subset receipts. The full candidate
accepted the Sequence control input. The subset compiled exactly the Flowchart and Gantt
parser families and rejected that same Sequence input with exit code 2 and stderr
`merman.parse.unsupported_diagram`.

Receipts:

- `target/bench/diagram-selection/pre-change-full/receipt.json`
- `target/bench/diagram-selection/candidate-full-eb1d5aca1/receipt.json`
- `target/bench/diagram-selection/candidate-subset-eb1d5aca1/receipt.json`

The ignored experiment ledger is
`target/bench/experiments/selectable-diagram-families/experiment.yaml`.

## Scope and limits

This is evidence for one Windows native executable, one compiler target, one release
profile, and a small selected-family corpus. It does not establish latency, memory,
WebAssembly, mobile, VST, or other platform behavior. Those claims require separate
measurements with their own controls.
