# Typst 0.5.0 preparation

`@preview/merman:0.5.0` is the selected Typst candidate, bundling Merman 0.9.0. It is prepared alongside the workspace release and remains an independent manual registry submission. This document does not claim that Typst Universe accepts or serves 0.5.0.

Published `@preview/merman:0.4.0` remains the 2026-10-09 package bundling Merman 0.8.0, plugin ABI 2, and minimum Typst 0.15.0. Its source/version mapping and immutable artifact history remain unchanged; see the [publication receipt](PUBLISH_ORDER.md#typst-040-publication).

## User contract

The candidate retains the existing rendering functions, figures, profiles, and raw-block handlers, options schema 2, plugin ABI 2, and minimum Typst 0.15.0 declared by its package. Use the [package README](../../distribution/typst/merman/README.md) for imports and rendering options. The prepared documentation uses `@preview/merman:0.5.0`; local staging belongs in development instructions and is not evidence of registry availability.

Relative to the 0.4.0 bundle, Merman 0.9.0 corrects Unicode HTML wrapping, decoded whitespace and Markdown code/escaped-text handling, and resvg-safe hollow markers, leaves the core SVG canvas unset unless explicitly requested, and improves deep diagram cleanup and Block metadata consistency. Use `#mermaid(source, background: "white")` when the document needs opaque artwork. The plugin remains a deterministic, system-independent WASM surface; CLI native font discovery is not a Typst capability.

ELK defaults, the top-level Mermaid `layout` setting, and Dagre recipes already belong to the published 0.4.0 package. They are not new 0.5.0 migrations. Review snapshots containing long labels, preserved newlines, non-breaking spaces, soft hyphens, or Markdown code/escaped markup.

The Typst package does not expose the native ABI or the new edge-geometry operation. Its five callable ABI operations and capability catalog remain the authority. The development smoke interpreter upgrade is a tool change, not an added Typst runtime feature.

## Preparation validation

On 2026-10-12, the exact `typst-wasm` recipe passed the unchanged size limits with Rust 1.95.0 and Binaryen 133: raw 13,936,547, optimized/stripped 8,822,440, gzip 3,433,230, and Brotli 2,552,937 bytes. The prepared 0.5.0 package passed interpreter validation, then compiled 21 fixtures and verified one expected diagnostic on each of Typst 0.15.0 and 0.15.1. The API fixture checked bundled Merman 0.9.0. The Wasmi 2 host dispatcher migration includes a long-loop regression and passed the real package run.

The final package source is `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`, also tagged as workspace v0.9.0. The approved intended publication date is 2026-10-12, and the package changelog is dated accordingly. It was rebuilt from that source, with staged README and changelog bytes matching their source files, then passed 21 compiled fixtures and one expected diagnostic on each of Typst 0.15.0 and 0.15.1. The local optimized WASM digest is `d16db86a2290777a86094f6289c23ac9c8b16c59097c618044e5540958df1495`.

[Immutable workspace preflight 38242910387, attempt 2](https://github.com/Latias94/merman/actions/runs/38242910387) passed all 24 jobs. The separate [Linux Typst owner job](https://github.com/Latias94/merman/actions/runs/38250149414/job/114826095865) passed generated-profile and closure checks, size budgets, package construction, interpreter validation, and all 21 fixtures plus the expected diagnostic. Its host-specific WASM measurements were raw 13,932,914, optimized/stripped 8,819,309, gzip 3,431,334, and Brotli 2,553,400 bytes; do not treat them as byte-identical to the local Windows artifact. Retain the exact chosen package and private receipt for submission. Universe submission and acceptance remain pending even though the workspace crates and CLI/LSP have shipped.

## Owner checklist

- [x] Confirm wrapper 0.5.0, bundled workspace 0.9.0, ABI 2, compiler minimum, and final reviewed source SHA.
- [x] Run generated-profile verification and the exact `typst-wasm` dependency-closure check.
- [x] Run the Typst artifact size matrix and inspect the owner report.
- [x] Build the `publish` package from the reviewed source and inspect the staged README, changelog, legal material, and exact WASM digest.
- [x] Run the full Typst package smoke and example suite with the admitted interpreter/toolchain.
- [x] Freeze the private artifact receipt and final package source; require new evidence after any source change.
- [ ] Obtain explicit Typst shipping authorization separately from workspace shipping.
- [ ] Submit the frozen package once, then verify exact registry version and observed publication date before updating status.

Use the owner commands in [RELEASING.md](RELEASING.md) rather than a duplicate packaging script. Build receipts and detailed audits belong under `target/typst-wasm-artifacts/` or release artifacts; the user changelog records the final capability contract.
