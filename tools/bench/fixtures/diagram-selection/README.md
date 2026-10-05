# Native diagram-selection probe

This independent consumer exercises the public SVG renderer with source files supplied at runtime.
`--families` reports the compiled semantic-parser families, excluding infrastructure models. It uses
no family-specific Rust types, so each build exposes the same called API to the linker.

The driver archives the committed library revision and seeds its isolated consumer from that
revision's lockfile. It reads the probe from a separate, explicitly committed `--fixture-revision`,
so a historical library revision does not need to contain the probe. Run builds sequentially;
each evidence directory has its own `cargo-target` and retains its executable. Every Rust command
uses the same explicit Rustup toolchain, including when `--output` is outside the repository.
`--toolchain` defaults to the driver's checkout pin, not the measured library's pin. It records
the compiler, exact release settings, dependency tree, parser set, corpus
hashes, SVG hashes, and Sequence control result. Existing output directories are never overwritten.
It measures executable bytes; it does not measure latency or predict another platform's savings.

After committing the candidate, replace `CANDIDATE` with its commit SHA in all three recipes.
Use the same fixture revision and toolchain in every lane. The example uses the original
measurement's Rust 1.95.0; choose another installed toolchain explicitly when collecting a new
comparison. Run from the repository root:

```console
python tools/bench/measure_diagram_selection.py --revision 72c024776 --fixture-revision CANDIDATE --toolchain 1.95.0 --features svg --label pre-change-full --output target/bench/diagram-selection/pre-change-full
python tools/bench/measure_diagram_selection.py --revision CANDIDATE --fixture-revision CANDIDATE --toolchain 1.95.0 --features all-diagrams,svg --label candidate-full --output target/bench/diagram-selection/candidate-full
python tools/bench/measure_diagram_selection.py --revision CANDIDATE --fixture-revision CANDIDATE --toolchain 1.95.0 --features diagram-flowchart,diagram-gantt,svg --label candidate-subset --output target/bench/diagram-selection/candidate-subset
```

Compare the retained `receipt.json` files. All lanes must share the fixture revision, fixture/input
hashes, toolchain, profile, resolved dependency versions and selected-corpus SVG hashes. The subset must report exactly
`flowchart` and `gantt`, reject Sequence with `merman.parse.unsupported_diagram`, and be strictly
smaller than candidate full. The two full builds must accept Sequence. Compare pre-change full with
candidate full separately to disclose architecture overhead. Keep this probe outside regular CI.
