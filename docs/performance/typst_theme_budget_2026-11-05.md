# Typst Mermaid 12.3 and theme budget

The maintainer authorized a size-budget increase for reasonable capability growth on 2026-11-05. Only `typst-wasm` is reassessed. Each ceiling remains its measured value plus 3%, rounded upward to 1,000 bytes; optimizer settings, compression settings and all five Web budgets are unchanged.

## Measurement and ceilings

The baseline is the Linux measurement from [PR 277 CI run 41986465583](https://github.com/Latias94/merman/actions/runs/41986465583/job/127108376363), using Rust 1.95.0, Binaryen 133 and wasm-tools 1.261.0. Its tested merge is `f0330f6732b309d239f3765fb715c83b90e765c0`, combining PR head `99c466b76ccf9fd439960db61573f3456f3c88ec` with main `0649021aa9bca54add6911b3551d4228943c0987`.

| Metric | Previous ceiling | Linux measurement | New ceiling | Local repaired-source control |
| --- | ---: | ---: | ---: | ---: |
| Raw bytes | 20,729,000 | 20,979,113 | 21,609,000 | 20,978,154 |
| Stripped bytes | 12,663,000 | 12,793,758 | 13,178,000 | 12,792,986 |
| Gzip bytes | 4,801,000 | 4,849,760 | 4,996,000 | 4,848,633 |
| Brotli bytes | 3,543,000 | 3,588,751 | 3,697,000 | 3,589,934 |

The local control uses macOS ARM64, the same Rust/Binaryen/wasm-tools versions and repaired source `dfd0d8e98`. Its production-input digest is `698e46738ee429fb8c5635603221e2e6445724c0d1b45183ce735e3cce7e56ae`; its raw WASM SHA-256 is `474af02b46a0c892c8581b540379087982107d71693d7019c7e78ed7909cef45` and its final stripped WASM SHA-256 is `5012c7019edf0c0b5ca301542c0be3c3e035579cf9bb7c46bf34bf763ce91468`. The control fits every new ceiling. It is a separate artifact, not a replacement for the Linux measurement.

Both use the descriptor-owned `publish` recipe: `all-diagrams,analysis,layout-cytoscape,layout-elk,svg`, defaults off, target `wasm32-unknown-unknown`, Cargo profile `wasm-size`, Binaryen `-Oz` and `wasm-tools strip --all`. The existing size owner applies gzip best and Brotli quality 11/window 22 to the final stripped artifact. There is no additional platform margin or cross-metric offset.

## Why the growth is accepted

Compared with the [previous Typst measurement](semantic_wasm_budget_2026-10-25.md), the Linux artifact grows by approximately 4.1%–4.3% across the four metrics. That historical comparison includes intervening changes; it does not isolate this PR's cost or establish an equal-capability performance regression.

The current source adds Mermaid 12.3 Flowchart Step behavior, nine BPMN shapes and ELK 0.12 behavior, together with core theme paint and typography consumers across the 34 logical diagram families. The [Mermaid migration](../migrations/mermaid-12-3.md) and [theme coverage](../rendering/diagram-theme-coverage.md) describe the new behavior and its limits. Relevant production owners include `crates/merman-render/src/flowchart/bpmn.rs`, `crates/merman-render/src/svg/parity/flowchart/step.rs`, the family theme adapters and `crates/merman-elk-layered`.

The PR retains its base's `Cargo.lock`, Typst dependency recipe, Cargo optimization profile and artifact descriptors. Growth therefore includes additional reachable implementation under the existing complete-product recipe, rather than a newly included package or changed compression policy. Exact byte attribution to individual functions was not isolated. Supporting the added behavior is accepted; no capability, validation, cancellation or evidence check is removed to meet the budget.

## Verification and reproduction

The repaired-source package build passes all four ceilings, validates the plugin exports and shared theme vectors (`support=28`, `materialization=2`, `errors=3`), and installs the versioned package. Real Typst smoke compiles 23 fixtures and verifies nine expected failures with Typst 0.15.1 and the same verified WASM. Local generated artifacts and their manifest remain under `target/typst-wasm-artifacts/publish`; the manifest owns the complete input-file and toolchain fingerprints.

```text
cargo run --locked -p xtask -- build-typst-package --profile publish --budget-file docs/release/WASM_SIZE_BUDGETS.json
cargo run --locked -p xtask -- typst-package-smoke --profile publish --skip-wasm-build --typst <typst-binary>
```

The original Linux job remains failed at the previous budget. The repaired commit requires its own CI results. This reassessment establishes artifact-size acceptance only; it makes no latency or memory claim.
