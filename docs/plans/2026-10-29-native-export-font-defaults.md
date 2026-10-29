# Native export font defaults

## Problem and evidence

The Spotless Sequence gallery note was laid out with font-agnostic deterministic measurement
and painted with native fonts. Its 181px rectangle could not contain a 214.9px label. Sharing
a native font context produced a 235px rectangle with about 10.1px horizontal padding. The
same CLI preparation policy exists on `main` and `release/0.9.x`; this is independent of the
new compiled theme system.

## Decision

- Mermaid input rendered to PNG, JPEG, or PDF selects a native font context before layout by
  default. The existing operation-local context also supplies native painting.
- CLI codec features expose their existing native/system font dependency through the CLI's
  own font features, so lean codec builds receive the same policy and explicit controls.
- Pure SVG keeps deterministic measurement by default. Explicit font files and directories
  select native measurement with a closed set unless system fallback is requested.
- Explicit `--font-measurement deterministic` retains the approximate compatibility path.
  Post-layout CSS remains rejected for native measurement with an actionable compatibility
  alternative. Missing native resources fail visibly rather than pretending to measure them.
- Raw SVG remains conversion-only. Rust, FFI, WASM, ASCII, and Rustdoc defaults do not acquire
  fonts implicitly. The Rust facade already normalizes caller-selected contexts across binary
  measurement and painting; no new public API is required.
- Capability discovery keeps SVG defaults separate from encoded-output defaults. Release and
  installation checks continue to validate the complete exact policy rather than weakening it.

## Implementation and maintenance backport

1. Change the common CLI font preparation path used by `render`, `mmdc`, and Markdown batches;
   include automatic native measurement in shaping-budget admission.
2. Add actual-output regressions comparing implicit and explicit native output for each codec,
   plus explicit deterministic, raw-SVG, CSS, batching, and no-system-font boundaries.
3. Add a fixed-font Sequence note containment regression using independent usvg text bounds.
4. Regenerate CLI help assets and update user documentation and unreleased changes.
5. Apply the equivalent policy to a maintenance feature branch after a verified fast-forward.
   Keep its published codec discovery features, Options schema, ABI, and request architecture.

## Validation

Run Cargo serially with two build jobs and the existing target cache. Use focused nextest
contracts first, CLI feature leaves and Clippy next, then the relevant release Python contracts,
generated CLI assets, dependency/legal projections, and representative feature checks. Verify
both branches with their own source and a real default encoded render. Renderer algorithms,
upstream fixtures, and parity normalization are outside this change.

System font acquisition and shaping can increase native export cost. This corrects geometry
for the selected host and does not establish browser parity, identical cross-host pixels,
complete glyph coverage, or a general performance improvement. Existing budgets and explicit
fixed-font inputs remain the controls for bounded and reproducible operations.

## Related native filter findings

The fixed-font containment test also detected an older maintenance-line Sequence defect:
Neo CSS referenced `#drop-shadow` while the writer emitted a diagram-prefixed definition.
Native SVG parsing discarded the unresolved-filter note frame. Main already scoped those
references; the maintenance patch reuses the same helper and preserves custom filters.
Four existing Sequence residual receipts change only their four CSS filter references. Using
the existing canonical-signature function, reversing those references reproduces every old
signature exactly; source, upstream bytes, geometry, text, modes, and precision are unchanged.

A targeted search found the same missing scoping on GitGraph tag backgrounds on both lines.
The patch maps that one CSS owner to its actual definition. Integration coverage checks four
Redux themes, two diagram IDs, and unchanged custom URL/function filters. Other audited shared
Neo, Mindmap, State, Flowchart, and Timeline paths already scope their built-in references.

## Verification record

- Main's complete CLI inventory ran 617 tests with one separate intentional skip. The first
  run passed 614 and exposed three old CSS/help expectations; all affected targets passed
  after correction. Fifteen explicit-font-only contracts and all five selected exact process
  profiles (SVG, PNG, JPEG, PDF, parallel PDF) passed.
- Both lines passed the three fixed-font containment tests, covering nine real Sequence note
  cases. Main's actual default Spotless PNG equals its explicit-native output byte-for-byte.
- Main passed 16 isolated feature consumers, 49 representative feature builds, all-target and
  all-feature CLI Clippy, generated assets, representative dependency closures, and legal
  reports/projections. The focused release/install Python contracts passed.
- Maintenance passed all 573 CLI tests with one separate intentional skip, all five selected
  exact process profiles, 166 Sequence contracts, its GitGraph contract, all-feature CLI
  Clippy, generated assets, 112 focused Python contracts, closures, and legal material checks.
  The complete structure/parity/parity-root SVG comparison passed after the reviewed
  Sequence receipt refresh. Main's 67 GitGraph integration tests and its final complete
  structure/parity/parity-root SVG comparison also passed.
- Maintenance commit `674a56cba` is proposed separately in
  [PR #255](https://github.com/Latias94/merman/pull/255), with real before/after native exports.
  Remote CI owns the remaining cross-platform matrix; pending jobs are not local passes.
