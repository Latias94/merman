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

Cargo metadata owns that graph. Print the current topological order with:

```bash
python3 tools/publish.py --list-crates-io-packages
```

The release workflow and local publish helper consume the same derived batches. Independent crates
are ordered lexically only within a batch; this document does not restate the list.

## Dry runs

- `cargo publish -p <crate> --locked --dry-run --registry crates-io`
- If your working tree is not clean, add:
  - `--allow-dirty`

Important: dry runs for crates that depend on unpublished workspace crates will fail until those
dependencies exist on crates.io. Use the derived batches for local verification. The credentialed
release workflow additionally records each prepared `.crate` checksum and reconciles the complete
batch against crates.io before any dependent batch starts; local dry runs are not publication
evidence.
