# Typst 0.5.0 preparation

`@preview/merman:0.5.0` is the selected Typst candidate, bundling Merman 0.9.0. It is prepared alongside the workspace release and remains an independent manual registry submission. This document does not claim that Typst Universe accepts or serves 0.5.0.

Published `@preview/merman:0.4.0` remains the 2026-10-09 package bundling Merman 0.8.0, plugin ABI 2, and minimum Typst 0.15.0. Its source/version mapping and immutable artifact history remain unchanged; see the [publication receipt](PUBLISH_ORDER.md#typst-040-publication).

## User contract

The candidate retains plugin ABI 2 and the minimum Typst 0.15.0 declared by its package. Use the [package README](../../distribution/typst/merman/README.md) for imports and rendering options. The prepared documentation uses `@preview/merman:0.5.0`; local staging belongs in development instructions and is not evidence of registry availability.

Relative to the 0.4.0 bundle, Merman 0.9.0 corrects Unicode HTML wrapping and resvg-safe hollow markers, leaves the core SVG canvas unset unless explicitly requested, and improves deep diagram cleanup and Block metadata consistency. Use `#mermaid(source, background: "white")` when the document needs opaque artwork. The plugin remains a deterministic, system-independent WASM surface; CLI native font discovery is not a Typst capability.

The Typst package does not expose the native ABI or the new edge-geometry operation. Its five callable ABI operations and capability catalog remain the authority. The development smoke interpreter upgrade is a tool change, not an added Typst runtime feature.

## Local preparation validation

On 2026-10-12, the exact `typst-wasm` recipe passed the unchanged size limits with Rust 1.95.0 and Binaryen 133: raw 13,936,547, optimized/stripped 8,822,440, gzip 3,433,230, and Brotli 2,552,937 bytes. The prepared 0.5.0 package passed interpreter validation, then compiled 21 fixtures and verified one expected diagnostic on each of Typst 0.15.0 and 0.15.1. The API fixture checked bundled Merman 0.9.0. The Wasmi 2 host dispatcher migration includes a long-loop regression and passed the real package run.

These are local preparation results. Final source freeze, dated package documentation, immutable preflight, and registry submission still need their owner evidence.

## Owner checklist

- [ ] Confirm wrapper 0.5.0, bundled workspace 0.9.0, ABI 2, compiler minimum, and final reviewed source SHA.
- [ ] Run generated-profile verification and the exact `typst-wasm` dependency-closure check.
- [ ] Run the Typst artifact size matrix and inspect the owner report.
- [ ] Build the `publish` package from the reviewed source and inspect the staged README, changelog, legal material, and exact WASM digest.
- [ ] Run the full Typst package smoke and example suite with the admitted interpreter/toolchain.
- [ ] Freeze the private artifact receipt and final package source; require new evidence after any source change.
- [ ] Obtain explicit Typst shipping authorization separately from workspace shipping.
- [ ] Submit the frozen package once, then verify exact registry version and observed publication date before updating status.

Use the owner commands in [RELEASING.md](RELEASING.md) rather than a duplicate packaging script. Build receipts and detailed audits belong under `target/typst-wasm-artifacts/` or release artifacts; the user changelog records the final capability contract.
