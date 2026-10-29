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
