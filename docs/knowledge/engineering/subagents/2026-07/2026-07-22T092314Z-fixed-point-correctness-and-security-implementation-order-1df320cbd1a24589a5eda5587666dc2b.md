---
type: "Subagent Finding"
title: "Fixed-point correctness and security implementation order"
description: "Verified implementation packets for the 15 open fixed-point findings, ordered by ownership and risk."
timestamp: 2026-07-22T09:23:14Z
record_id: "1df320cbd1a24589a5eda5587666dc2b"
producer_id: "codex-root"
run_id: "019f5be4-4389-72e0-9695-99fe14830072"
subagent_id: "implement_u2_remove_core_full_facades"
related_plan: "docs/plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md"
git_branch: "refactor/family-owned-architecture"
git_commit: "4e2a2d6c6"
---

# Finding

The fixed-point review was revalidated against the current tree. Three reported issues are already
closed; fifteen remain and are tracked by the related plan's review intake. Finish the exact
artifact-recipe boundary first, then implement the remaining findings in ownership and risk order.

# Evidence

- The C engine entry path dereferences an opaque pointer before it can acquire lifecycle ownership,
  leaving a use-after-free window against concurrent free.
- Native raster preflight scans `<image>` but not `<feImage>` before usvg decoding; the Web DOM
  policy validates data-URL shape without decoded byte, dimension, pixel, or frame budgets.
- The detector global fast path runs before the selected registry and therefore violates empty,
  custom, and first-match registry semantics.
- ASCII bindings do not apply the same operation resource policy as SVG/analysis; raw Typst
  `analysis` selects a resource profile that the no-render build rejects.
- SVG DOM text normalization, Wardley roles, Venn union/title behavior, runtime capability
  reporting, CI feature/freshness gates, and current-facing docs retain confirmed drift.

# Recommendation

1. Replace C FFI raw-engine ownership with a process-wide opaque-handle registry whose entries own
   `Arc<FfiEngineState>` leases. Free atomically removes the handle; already leased calls finish.
2. Delete the detector global fast path and always honor the selected registry's first-match order.
3. Introduce one native embedded-raster scanner for `<image>` and `<feImage>` that enforces byte,
   format, dimension, pixel, frame, and aggregate budgets before decoder entry.
4. Add equivalent synchronous pre-decode budgets to the canonical browser DOM insertion policy and
   regenerate downstream copies.
5. Move operation resource policy to a backend-independent owner and apply it to SVG, ASCII, and
   analysis bindings; make raw Typst analysis valid without implicitly compiling render.
6. Preserve visible text while normalizing browser `tspan` wrapping; then align Wardley/Venn to the
   pinned source and finish capability, CI, freshness, and documentation corrections.

# Disposition

Accepted for serial implementation after the artifact-recipe unit. Each behavior
change starts from a focused regression or deterministic characterization; crate/platform tests
run per batch before workspace nextest, clippy with warnings denied, generated freshness, and the
strict owning gates.

# Citations

- `docs/plans/2026-07-22-001-refactor-capability-driven-feature-and-distribution-architecture-plan.md`
- `crates/merman-ffi/src/lib.rs`
- `crates/merman-core/src/detect/mod.rs`
- `crates/merman/src/render/raster.rs`
- `platforms/web/src/svg-safety-policy.ts`
- `crates/merman-bindings-core/src/ascii.rs`
