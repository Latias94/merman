# Publishing (crates.io)

This repository is a Cargo workspace containing multiple publishable crates. Publishing is gated by
ADR-0050 (`docs/adr/0050-release-quality-gates.md`) and requires publishing workspace crates in a
dependency-safe order.

## Release gates (must pass)

- Format:
  - `cargo fmt --check`
- Tests:
  - `cargo nextest run`
- SVG DOM gates (Mermaid parity contract):
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity --dom-decimals 3 --flowchart-text-measurer vendored`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --flowchart-text-measurer vendored`
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --flowchart-text-measurer vendored`

Notes:

- `--dom-mode strict` is intentionally not a release gate. It is treated as a parity KPI / debugging
  tool (see ADR-0050).
- A higher-precision viewport stress check exists but is non-blocking:
  - `cargo run --release -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --flowchart-text-measurer vendored`

## Publish order

When running `cargo publish`, Cargo resolves workspace `path` dependencies as registry dependencies,
so dependency crates must be published first.

Recommended order (Mermaid renderer stack):

1. `dugong-graphlib`
2. `dugong`
3. `manatee`
4. `merman-core`
5. `merman-render`
6. `merman`
7. `merman-cli`

## Dry runs

- `cargo publish -p <crate> --dry-run`
- If your working tree is not clean, add:
  - `--allow-dirty`

Important: dry runs for crates that depend on unpublished workspace crates will fail until those
dependencies exist on crates.io. Use the publish order above for end-to-end dry-run verification.

