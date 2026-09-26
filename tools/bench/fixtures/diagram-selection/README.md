# Native diagram-selection probe

This independent consumer exercises the public SVG renderer with source files supplied at runtime.
`--families` reports the compiled semantic-parser families, excluding infrastructure models. It uses
no family-specific Rust types, so each build exposes the same called API to the linker.

The driver archives one committed revision, seeds its isolated consumer from that revision's lockfile,
builds serially into the repository target directory, and preserves the final executable before any
subsequent build. It records the compiler, exact release settings, dependency tree, parser set, corpus
hashes, SVG hashes, and Sequence control result. Existing output directories are never overwritten.
It measures executable bytes; it does not measure latency or predict another platform's savings.

After committing the candidate, run the three recipes from the repository root:

```console
python tools/bench/measure_diagram_selection.py --revision 72c024776 --features svg --label pre-change-full --output target/bench/diagram-selection/pre-change-full
python tools/bench/measure_diagram_selection.py --revision CANDIDATE --features all-diagrams,svg --label candidate-full --output target/bench/diagram-selection/candidate-full
python tools/bench/measure_diagram_selection.py --revision CANDIDATE --features diagram-flowchart,diagram-gantt,svg --label candidate-subset --output target/bench/diagram-selection/candidate-subset
```

Compare the retained `receipt.json` files. All lanes must share the fixture/input hashes, toolchain,
profile, resolved dependency versions and selected-corpus SVG hashes. The subset must report exactly
`flowchart` and `gantt`, reject Sequence with `merman.parse.unsupported_diagram`, and be strictly
smaller than candidate full. The two full builds must accept Sequence. Compare pre-change full with
candidate full separately to disclose architecture overhead. Keep this probe outside regular CI.
