# Theme rendering ownership refactor checkpoint — 2026-10-06

## Decision and scope

This checkpoint accepts two structural ownership changes and intentionally makes no latency,
allocation, RSS, or binary-size claim.

1. Parse operations retain a bounded mutation-journal checkpoint across detector execution rather
   than retaining a complete `MermaidConfig` JSON snapshot.
2. Resvg-compatible family output is sealed in one terminal operation after prepared-text and
   prepared-math evidence have been supplied. The final native projection owns the strict
   validation result, resource closure, finalization report, and resource fingerprint.

The change does not add a cache, relax validation, specialize by diagram family, change a public
output format, or change the dependency closure. Strict-validation traversal fusion and compiler
dependency movement are deferred experiments, not implied by this checkpoint.

## Configuration evidence

`MermaidConfig::clone()` is an `Arc` clone. The relevant cost occurs only when a later mutable
operation detaches the shared JSON value. Test-only scoped diagnostics at that detachment point
record COW event count and a structural estimate of copied JSON storage; they are not allocator
or wall-clock measurements.

The source-selected-theme characterization continues to observe two necessary pre-detection COW
events. They belong to legacy font-family mirroring and selected-theme materialization, not to
the removed detector boundary. A custom detector that mutates `flowchart.nodeSpacing` now records
one COW event: the operation-owned configuration detaches from the engine baseline once. Retaining
the former full configuration snapshot would force an additional complete JSON detachment during
that detector write.

The mutation journal preserves explicit ownership for same-value writes, parent-path writes and
deletions, which are all exercised by the existing overlay suite. The removed
`config/appearance.rs` file had no module registration or call sites; current appearance and
theme materialization use the live parse/theme pipeline.

## SVG evidence

The previous Resvg lifecycle constructed an artifact, then attached prepared-text and prepared-
math evidence through mutable transitions. The new `seal` operation consumes evidence before
publishing the artifact and computes the resource fingerprint from the selected native SVG only.
It retains the mandatory reserved-token scan for an empty prepared-text ledger, so a custom
postprocessor cannot inject an unowned internal token.

For non-empty prepared-text evidence, strict validation still runs before evidence partitioning to
preserve the established error boundary. The generated native projection is then validated again
because it is the projection bound to the final report and fingerprint. This is deliberate:
strict-pass fusion is a separate experiment and was not accepted without a proof that XML syntax,
resource, cancellation, and evidence error ordering remain identical.

Standalone exact artifacts now also validate and budget the native prepared-text projection while
retaining the public projection for callers. The targeted regression test supplies a public SVG
that is admissible and a native projection containing `foreignObject`; strict admission correctly
rejects the native projection.

## Validation

Commands run serially with `CARGO_BUILD_JOBS=1`:

```text
cargo nextest run --locked -p merman-core --lib
# 1,675 passed

cargo nextest run --locked -p merman-render --lib
# 2,530 passed, 2 skipped

cargo fmt --all -- --check
git diff --check
```

The complete `merman-core` nextest command remains blocked before test execution by the existing
`tests/usecase_contract.rs` feature mismatch: it references `RenderSemanticModel::Usecase` and
`EditorRenamePolicy::UsecaseIdentifier`, neither of which is built by the current crate feature
set. This checkpoint does not alter that integration-test configuration.

## Deferred experiments

- Fuse strict XML/resource validation only after a separate error-order and budget/cancellation
  experiment proves equivalence.
- Move the theme JSON compiler behind the `merman` facade only after equal-capability CLI and
  bindings tests cover error mapping. Do not infer a binary-size saving from a dependency edge.
- Measure raw, stripped, and compressed artifacts using matched feature closures before claiming
  any size change. The current evidence attributes renderer growth to theme/SVG capability paths
  more strongly than to a single added crate, but does not prove an eliminable byte count.
- Run calibrated adjacent-revision A/A and balanced AB/BA measurements before making a latency or
  allocation improvement claim.
