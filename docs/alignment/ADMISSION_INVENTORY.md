# Diagram Admission Inventory

Status: Active
Baseline: Mermaid `@11.15.0`
Last updated: 2026-06-06

The structured admission inventory lives in `crates/xtask/src/cmd/admission.rs`.

It records, per diagram family:

- admission state: primary SVG matrix, compatibility-only, parse-only, not admitted, or not in the
  pinned baseline;
- fixture corpus state: normalized fixtures, normalized plus deferred fixtures, or no admitted
  fixtures;
- semantic, layout, SVG baseline, and root viewport coverage;
- compare command ownership;
- owning alignment document;
- explicit defer reason for non-primary or root-deferred families.

Parser and typed-render capability evidence is projected from `merman-core` diagram family facts.
The inventory still owns fixture corpus state, coverage status, compare-command ownership, owner
docs, and defer reasons because those are release/admission policy rather than parser registry
facts.

Current consumers:

- `xtask compare-all-svgs` reads the primary SVG matrix projection and the root-viewport-deferred
  projection from the inventory.
- `xtask check-alignment` verifies inventory paths, owner docs, semantic/layout fixture evidence,
  upstream SVG directories, compare-command presence for primary diagrams, and defer reasons for
  non-admitted families.
- `xtask check-alignment` also checks that semantic/layout/SVG-covered records are backed by the
  corresponding `merman-core` family capability facts, and that families marked outside the pinned
  baseline are absent from those facts.

This inventory does not move fixtures or admit unsupported families by itself. Promotion still
requires the gates in `docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md`.
