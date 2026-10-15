# Theme final repair verification

The complete confirmed review of `f754a3c3..96eef309` is repaired. All 68
confirmed findings and 26 concrete testing gaps are verified. All 36 necessity
records are assessed: 35 are verified and N1 preserves required public
compatibility. Candidate #76 was independently rejected as unproven; integration
hypothesis U6-R6 was rejected as unreachable and is not counted as a fixed bug.

The exact item mapping is in [WORKSTREAM.json](WORKSTREAM.json); the immutable
review input is [issues.json](issues.json). Commands, source identities, log
hashes and independent review scope are retained in [verification.json](verification.json).

## Implementation

- Preserve source ownership, operation isolation, terminal evidence and actual
  font/glyph facts throughout core, renderer, native export and binding paths.
- Resolve all affected diagram families, including geometry, CSS precedence,
  native paint, text escaping and Markdown, cancellation and admission budgets.
- Close isolated feature recipes, remove obsolete private paths and duplicate
  immutable-plan/invariant work, and retain necessary published compatibility.
- Keep new unmerged theme contracts at version 1, regenerate all ten preset
  fingerprints from the compiler, and remove unpublished version migration
  alternatives. Unknown versions remain rejected. Published ABI namespaces and
  historical verification identities retain their separate histories.

## Verification

The integrated source passed 2,037 core/contract cases, 4,968 renderer cases,
135 explicitly selected public facade cases, 154 export cases and 20 Node
consumer cases. Five core crates passed all-target Clippy with warnings denied;
Rust formatting and Git whitespace checks passed.

Actual consumer checks also cover Typst, fresh Flutter FFI, the complete frozen
Swift loader boundary, CLI contracts and computed browser glyph/background paint.
Their exact feature sets, source/artifact identities, corrected test premises and
validation limits are preserved in the receipts rather than generalized to all
platforms.

## Rust 1.100 beta follow-up

The maintainer requested testing after the [Rust 1.100 breakage post](https://www.reddit.com/r/rust/comments/1x3yz9i/psa_incoming_breakage_in_1100_test_your_code_on/).
Official sources identify [never type and Infallible changes](https://github.com/rust-lang/rust/pull/155499)
and [intermediate build-directory changes](https://blog.rust-lang.org/2026/03/13/call-for-testing-build-dir-layout-v2/).

On macOS ARM64, `cargo +beta test --offline --locked --workspace --no-fail-fast
-j 2 -- --test-threads=2` passed with Rust/Cargo `1.100.0-beta.4`, including
workspace doctests. The project remains pinned to Rust 1.95. Optional native-font
checks passed 33/33; the independent Node N-API workspace passed 20/20. All five
Rustdoc integration targets passed 15/15 on both beta 1.100 and pinned 1.95.

The follow-up found and corrected three additional issues:

- R17: derive Rustdoc target and dependency paths from Cargo metadata and the
  same build's artifact messages. Remove assumptions about executable ancestors
  and adjacent `deps` directories. The old beta binary reproduced an actual E0463
  failure; the repaired macro reexport and external-file rebuild tests pass.
- R18: inspect XML-decoded Mindmap style text while preserving all four exact
  CSS rule assertions, DOM consumers and gradient definitions.
- R19: replace timestamp-based temporary directories with `tempfile::TempDir`.
  Two parallel tests had reproduced an `AlreadyExists` collision. Existing
  assertions, parallel execution and automatic cleanup remain intact.

These fixes are committed as `292d515a7`. No application API, MSRV or toolchain
pin changed in this follow-up.

## Clean preset qualification

The exact standalone release recipe passed from clean source
`c0e528f23bd1b3e833dccbfc0beb057e54c274e1` on beta `1.100.0-beta.4`.
Brutalist, Spotless and Cyberpunk each passed six Flowchart/State/Sequence SVG and
PNG cells: 18 cells total, all with zero theme, Mermaid, bridge and source
residuals. The artifact SHA-256 is
`f1aafbea8d6c0d5cd0da65ab1f14ac1ef47fddcf21f2bd48866fcf850345c300`; the lockfile SHA-256 is
`88b0d0baa92a1b62b915e5d9c7c542219a84dcb1f043a42078e994ef35f2b068`.

A separate clean release run on Rust 1.95 passed at `aa2c6c81c` before the beta
follow-up. Both runs report host-dependent font admission and retain empty
qualified cells in the public catalog. Raw records remain under the cited local
target paths; their concise source, recipe, compiler and artifact identities are
preserved in the durable verification ledger.

## Claim boundaries

This is completion of the authorized repair and local verification. It does not
assert an installed-package or release-profile matrix for every operating system.
Native qualification remains host-dependent; no public catalog cells are promoted.
Performance evidence is structural and semantics/budget-backed; no numerical
latency, allocation-count or artifact-size gain is claimed.
