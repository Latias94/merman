# Three-family semantic parser consumer

This runtime-fed consumer inspects Flowchart node/edge ids and labels, Sequence actor/message
order and text, and State node/edge ids and descriptions. It calls the typed parser directly
without SVG, layout or compatibility JSON. U1 freezes the existing full typed parser; the later
semantic boundary migrates its API while preserving these consumed observations.

Use the existing isolated artifact driver after committing the harness:

```console
python tools/bench/measure_diagram_selection.py --recipe semantic-parser --revision BASE --fixture-revision HARNESS --features diagram-flowchart,diagram-sequence,diagram-state --label semantic-base --output target/bench/semantic-boundary/size-base
```

The driver archives committed sources, seeds a consumer lock from that revision, fixes the release
profile and Rustup toolchain, builds serially, and retains normal dependency/features and corpus
identities. It retains raw and stripped executables and checks their consumed outputs match.
Stripping uses the selected toolchain's `llvm-objcopy` (`rustup component add llvm-tools` if absent).
Raw means the unstripped release executable, with debug info disabled; this is not a debug build.

For final comparison use the same corpus bytes, target, toolchain/profile and dependency versions.
If a breaking API requires a migrated probe, retain both committed probe revisions and compare
the actual consumed output identities. Do not pull compatibility JSON into the lean probe to
hash it. Final stripped bytes must be smaller; package counts alone do not satisfy the goal.
This recipe is local artifact evidence, not a timing benchmark or a regular CI gate.
