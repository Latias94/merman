# Mermaid Upgrade Playbook

Use this playbook when moving the pinned Mermaid reference or importing a new upstream fixture
batch. The goal is semantic and structural convergence with Mermaid, not pixel-perfect output from
fragile local tweaks.

## Before The Upgrade

Record the upstream source version and the expected impact:

- parser grammar changes;
- config/theme changes;
- sanitizer or `securityLevel` changes;
- layout engine changes;
- DOM structure changes;
- new diagram families or renamed generated ids.

Check whether the change touches browser-dependent behavior. Text measurement, font fallback,
`getBBox()` floats, `foreignObject`, and RoughJS output often need bounded residuals rather than
broad comparator normalization.

## Import And Classify

For each changed family:

- import source-backed fixtures before changing render code;
- keep family-local evidence before adding cases to the main matrix;
- update DOM-id assertions from current baselines instead of preserving historical selectors;
- document accepted residuals in the family alignment notes.

Do not add broad normalizers to hide semantic drift. Comparator normalization should stay narrow,
structural, and reversible.

## Recheck Feature And Package Boundaries

Mermaid upgrades can change dependency pressure. After the upgrade, re-run the package surface
checks:

```bash
python scripts/verify-release-surfaces.py
cargo run -p xtask -- wasm-size-matrix --surface browser
cargo run -p xtask -- wasm-size-matrix --surface typst
```

If a feature now pulls new dependencies, update `docs/FEATURES.md`, `docs/release/PACKAGE_SURFACES.md`,
and the relevant package README before release.

## Verification Gates

Use the smallest gate that proves the touched family first, then widen:

```bash
cargo nextest run -p merman-core <family-or-parser-filter>
cargo nextest run -p merman-render <family-or-svg-filter>
cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3
```

For security-sensitive sanitizer or URL behavior, also run the focused sanitizer tests and update
the rendering security docs if host obligations change.
