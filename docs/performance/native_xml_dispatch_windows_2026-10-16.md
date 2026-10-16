# Reference-attribute dispatch checkpoint — Windows, 2026-10-16

## Scope and decision

The reference collector now dispatches on the exact attribute name before entering the larger
href and marker handlers. The complete preceding XML validation remains in the same position.
This is a semantics-preserving internal refactor, submitted at the maintainer's request; it has
no admitted latency improvement or non-regression result. It does not close the v0.9.0 regression.

The preceding [terminal phase attribution](native_svg_terminal_phases_windows_2026-10-16.md)
motivates examining attribute work, but does not isolate this helper's CPU cost. The new dispatch
changes constant factors only, without changing the complexity order or resource policy.

## Correctness and artifact controls

The independent existing legacy collector remains the differential oracle. One focused test
covers 384 attribute sequences, comparing intermediate state, final reference facts, errors and
every reachable marker checkpoint failure. Coverage includes foreign elements, case-sensitive
geometry names, raw/parsed IDs, href/xlink precedence, first-seen values and marker errors.

The final renderer suite passes 5,352 tests with six skipped tests under
`all-diagrams,layout-elk,layout-cytoscape,math`. Workspace formatting and renderer all-target
Clippy with warnings denied pass. Independent source review finds no blocking semantic issue.

The locked native benchmark uses Rust 1.95.0, default features plus SVG, and opt-level 3 on
Windows 11 / Intel Core i7-11700. Both executables emit 459 complete preflight receipts; all
152 render/end-to-end SVG identities match exactly. State semantic JSON object-member ordering
remains a separate pre-existing nondeterminism, recorded without changing the comparator.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Base, production-equivalent to `e56dde946` | 40,518,656 | `c9c9216d03f22ab086aba2f3fa882eeedbb3bcd70cd75a2566e1a2d1bf28aaa1` |
| Frozen one-file candidate on `db5a51f93` | 40,431,616 | `38fd2649900ce66f66f7947bb6cf5971d9e70257a94e2e999488d9304df3d355` |

The 87,040-byte reduction describes this executable recipe only. It is secondary artifact evidence,
not a package-size claim or a substitute for latency admission.

## Timing disposition

The preregistered diagnostic calls for three balanced pairs across seven fixtures, with 30
Criterion samples, two-second warmup and three-second measurement. Its first attempt observes
external Cargo/rustc processes at the Journey candidate's after-boundary and fails. Three earlier
completed measurements are retained only as invalidated raw output. The entire attempt is
discarded; no partial mean, speedup, regression or non-regression is reported.

No retry or confirmation has run at this checkpoint. A future timing decision needs a fresh
complete diagnostic and, if justified, clean-revision A/A calibration and power-sized confirmation
under the existing joint 10% and 50 us gate. The implementation's correctness and artifact evidence
must not be relabeled as latency recovery.

The ignored ledger, exact patch/source hashes, complete build receipt, test logs, identities,
failed diagnostic and compiler snapshots remain under
`target/bench/experiments/xml-reference-dispatch-20261016/`.
