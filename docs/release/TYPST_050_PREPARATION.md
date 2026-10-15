# Typst 0.5.0 preparation

`@preview/merman:0.5.0` is the selected Typst candidate, bundling Merman 0.9.0. It remains an independent registry submission and is submitted in [Typst Universe PR #6098](https://github.com/typst/packages/pull/6098). This document does not claim that Typst Universe has accepted or serves 0.5.0.

Published `@preview/merman:0.4.0` remains the 2026-10-09 package bundling Merman 0.8.0, plugin ABI 2, and minimum Typst 0.15.0. Its source/version mapping and immutable artifact history remain unchanged; see the [publication receipt](PUBLISH_ORDER.md#typst-040-publication).

## User contract

The candidate retains the existing rendering functions, figures, profiles, and raw-block handlers, options schema 2, plugin ABI 2, and minimum Typst 0.15.0 declared by its package. Use the [frozen package README](https://github.com/Latias94/merman/blob/f754a3c3077d35d2a372c2fec04f9e9b87db1c3f/distribution/typst/merman/README.md) for this submitted ABI 2 contract. The unmerged theme branch now documents its local ABI 3/Options 3 source and does not alter the frozen submission. The prepared documentation uses `@preview/merman:0.5.0`; local staging belongs in development instructions and is not evidence of registry availability.

Relative to the 0.4.0 bundle, Merman 0.9.0 corrects Unicode HTML line breaking, decoded whitespace and Markdown code/escaped-text handling, and resvg-safe hollow markers, leaves the core SVG canvas unset unless explicitly requested, and improves deep diagram cleanup and Block metadata consistency. [PR #182](https://github.com/Latias94/merman/pull/182) adapts Mermaid's supported marker ID-suffix CSS selectors to exact matches understood by Typst's SVG consumer; the affected consumer is usvg/simplecss, rather than a general limitation of Merman's cssparser tokenizer. [PR #192](https://github.com/Latias94/merman/pull/192) and [PR #193](https://github.com/Latias94/merman/pull/193) supply the line-breaking and label corrections. These changes do not provide browser-complete CSS. Use `#mermaid(source, background: "white")` when the document needs opaque artwork. The plugin remains a deterministic, system-independent WASM surface; CLI native font discovery is not a Typst capability.

ELK defaults, the top-level Mermaid `layout` setting, and Dagre recipes already belong to the published 0.4.0 package. They are not new 0.5.0 migrations. Review snapshots containing long labels, preserved newlines, non-breaking spaces, soft hyphens, or Markdown code/escaped markup.

The Typst package does not expose the native ABI or the new edge-geometry operation. Its five callable ABI operations and capability catalog remain the authority. The development smoke interpreter upgrade is a tool change, not an added Typst runtime feature.

## Preparation validation

On 2026-10-12, the exact `typst-wasm` recipe passed the unchanged size limits with Rust 1.95.0 and Binaryen 133: raw 13,936,547, optimized/stripped 8,822,440, gzip 3,433,230, and Brotli 2,552,937 bytes. The prepared 0.5.0 package passed interpreter validation, then compiled 21 fixtures and verified one expected diagnostic on each of Typst 0.15.0 and 0.15.1. The API fixture checked bundled Merman 0.9.0. The Wasmi 2 host dispatcher migration includes a long-loop regression and passed the real package run.

The frozen runtime source is `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`, also tagged as workspace v0.9.0, with tree `470c8d5b2dcc0a7b0746da6c93ae44ebf14d5eb7`. The independent wrapper's intended publication date is 2026-10-13. Its README and changelog were refreshed for that submission without rebuilding or changing the frozen runtime, wrapper modules, or ABI. The staged documentation matches those source files. The local optimized WASM digest remains `d16db86a2290777a86094f6289c23ac9c8b16c59097c618044e5540958df1495`.

[Immutable workspace preflight 38242910387, attempt 2](https://github.com/Latias94/merman/actions/runs/38242910387) passed all 24 jobs. The separate [Linux Typst owner job](https://github.com/Latias94/merman/actions/runs/38250149414/job/114826095865) passed generated-profile and closure checks, size budgets, package construction, interpreter validation, and all 21 fixtures plus the expected diagnostic. Its host-specific WASM measurements were raw 13,932,914, optimized/stripped 8,819,309, gzip 3,431,334, and Brotli 2,553,400 bytes; do not treat them as byte-identical to the local Windows artifact. The Windows artifact is the chosen Universe submission; retain it together with its private receipt. Registry acceptance remains pending even though the workspace crates and CLI/LSP have shipped.

On 2026-10-13, the documentation refresh passed `build-typst-package --profile publish --skip-wasm-build`, including frozen artifact identity and interpreter validation, and the full `typst-package-smoke --profile publish --skip-wasm-build` suite passed 21 compiled fixtures and one expected diagnostic on each of Typst 0.15.0 and 0.15.1. All 51 non-documentation package files matched the prior preparation receipt. A subsequent wording-only changelog update repeated the artifact and interpreter checks successfully. The refreshed private receipt at `target/release-preflight-0.9.0/typst-0.5.0-preparation.json` binds the final 53-file package and submitted Git blobs, including normal Git text-line-ending normalization, to external commit `ff5ae967c160cb02c61eff2be9c81fd9e1f945d9` on `Latias94/packages:merman-0.5.0`. On the final commit, [packaging Test](https://github.com/typst/packages/actions/runs/38279126114/job/114911795316), [Universe Lint](https://github.com/typst/packages/actions/runs/38279123402/job/114911788549), and the `@preview/merman:0.5.0` package check all succeeded. [PR #6098](https://github.com/typst/packages/pull/6098) remains open; passing checks do not establish registry availability.

## Owner checklist

- [x] Confirm wrapper 0.5.0, bundled workspace 0.9.0, ABI 2, compiler minimum, and final reviewed source SHA.
- [x] Run generated-profile verification and the exact `typst-wasm` dependency-closure check.
- [x] Run the Typst artifact size matrix and inspect the owner report.
- [x] Build the `publish` package from the reviewed source and inspect the staged README, changelog, legal material, and exact WASM digest.
- [x] Run the full Typst package smoke and example suite with the admitted interpreter/toolchain.
- [x] Freeze the private artifact receipt and final package source; require new evidence after any source change.
- [x] Obtain explicit Typst shipping authorization separately from workspace shipping (2026-10-13).
- [x] Submit the frozen package once in [Typst Universe PR #6098](https://github.com/typst/packages/pull/6098).
- [ ] Verify the exact registry version and observed publication date after upstream acceptance.

Use the owner commands in [RELEASING.md](RELEASING.md) rather than a duplicate packaging script. Build receipts and detailed audits belong under `target/typst-wasm-artifacts/` or release artifacts; the user changelog records the final capability contract.
