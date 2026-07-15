# ADR-0062: Fixture-Derived Overrides

## Status

Accepted

## Updated

2026-07-15 for Mermaid `@11.16.0`

## Context

Merman is a headless reimplementation of a pinned Mermaid release. Official upstream SVG baselines
are produced in a browser pipeline and therefore contain observable values that a deterministic
Rust implementation cannot always reproduce algorithmically:

- browser `getBBox()` and `getComputedTextLength()` float lattices;
- platform font fallback, shaping, and hinting;
- browser serialization details; and
- rare upstream oddities such as non-finite coordinates.

DOM parity must still catch semantic and structural regressions. A residual mechanism is therefore
needed without turning fixtures into an unrestricted tuning surface.

## Decision

Fixture-derived overrides are accepted only as small, deterministic, version-pinned projections of
the authoritative upstream SVG corpus. They model bounded browser behavior that is currently
impractical to reproduce headlessly. They are not visual tuning knobs and do not override parser,
semantic, layout, or DOM-structure correctness.

### Allowed categories

1. **Root viewport overrides**
   - Surface: root `<svg>` `viewBox` and responsive `max-width` only.
   - Key: typed render family plus exact fixture `diagram_id`.
   - Source: root attributes extracted from the pinned upstream SVG baseline.
   - Owner: the Root Viewport module; family renderers never query generated tables directly.

2. **Text and bbox overrides**
   - Surface: a documented text measurement result for an exact profile/font key and label.
   - Source: pinned upstream measurement or SVG evidence when vendored metrics are insufficient.
   - Owner: cross-family measurement adapters and decorators belong to the named profile selected by
     `RenderEnvironment`. A family-specific calibration may remain with its source-backed layout or
     SVG algorithm, but it must stay visible in the override inventory and must not instantiate or
     select another production measurer.

3. **Upstream-oddity compatibility markers**
   - Surface: rare behavior that cannot be represented directly in normal JSON, such as an upstream
     non-finite SVG value.
   - Policy: preserve semantic intent in the typed model and materialize the oddity only at the
     explicitly documented compatibility surface.

### Root override resolution

Root override resolution is an explicit render-environment policy:

- `RootViewportOverridePolicy::ApplyGenerated` permits the Root Viewport module to query generated
  evidence.
- `RootViewportOverridePolicy::ComputedOnly` uses computed source-backed bounds and is the audit
  path for identifying stale or unnecessary entries.
- Generated entries are used only under `ApplyGenerated`; computed bounds are the complete
  alternative under `ComputedOnly`. There is no mutable request-local explicit root override path.
- `merman-render` does not read `MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES` or any other process-global
  switch. Developer tooling may translate CLI or environment input into the explicit policy before
  constructing the operation.

Generated root lookup is centralized and accepts `RenderFamilyKind`, the pinned Mermaid baseline,
and `diagram_id`. A key from another family or baseline cannot apply accidentally. Historical
generated filenames may retain suffixes such as `*_11_12_2.rs`; those filenames are storage history,
not runtime authority. The typed router and generated provenance bind the values to Mermaid 11.16.

### Governance

- Every entry must be traceable to an upstream fixture and reproducible from its baseline.
- Prefer a source-backed parser, layout, measurement, or root algorithm fix before adding an entry.
- Overrides must not compensate for incorrect semantics, model ordering, layout topology, or SVG
  structure.
- Comparator normalization remains narrow and non-semantic. An override is not permission to add a
  broad mask.
- The inventory must not grow without reviewed upstream evidence:

  ```sh
  cargo run -p xtask -- report-overrides --check-no-growth
  ```

- Root entries must remain live and exact for the pinned baseline:

  ```sh
  cargo run -p xtask -- audit-root-overrides --fail-on-stale
  ```

- Root parity is verified through the canonical typed headless operation. Compare adapters must not
  rebuild a JSON parse-layout-render path to apply or inspect overrides.

## Paydown Strategy

- Run computed-only audits to identify entries whose source-backed algorithm now matches upstream.
- Remove stale keys when fixtures move or disappear.
- Prefer replacing fixture entries with deterministic measurement, layout, or root algorithms that
  generalize across the corpus.
- Keep accepted browser-dependent residuals explicit when no robust algorithmic fix exists.

## Consequences

- Browser-only float behavior can remain reproducible without weakening semantic or structural
  parity.
- Override selection has one policy owner and one typed lookup boundary.
- Family renderers remain free of fixture tables and mutable root-string patches.
- The override footprint is measurable debt whose growth and staleness are release-gated.
- Historical generated filenames no longer imply that production code targets an old Mermaid
  baseline.

## Related Decisions

- ADR-0014: Upstream Parity Policy
- ADR-0050: Release Quality Gates
- ADR-0057: Headless SVG Text `getBBox()` Approximation
- ADR-0073: Family-Owned Diagram Architecture
