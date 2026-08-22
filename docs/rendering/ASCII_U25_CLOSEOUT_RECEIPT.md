# ASCII U25 Closeout Receipt

Status: exact-current-source evidence for the U25 closeout. The measured product source is commit 99c567e89b922401adaad7ad521783eead895834. This receipt and its attachments are documentation-only evidence for that source point.

## Evidence boundary

| Item | Value |
| --- | --- |
| measured source commit | 99c567e89b922401adaad7ad521783eead895834 |
| measured source tree | 26ed5a1d956649d381ce3b34fa57d2566d96696c |
| measured tracked state | clean; protected legacy drafts and platform directories remained untracked and were excluded |
| merge-base | 16b84122615b2dd60c67577bd8708ef5f226f755 |
| benchmark backport | 76bbe6f351dea54d378680b40248f8310803c6d3 |
| benchmark backport product parent | 13d2ef8247c9b1cf679adb9679b8d333718d8269 |
| Cargo.lock SHA-256 | b3e19ec38bcdd8852b7411c8add8d9e04ff8f0baaf21514971b7fb0293592695 |
| benchmark source SHA-256 | 2dc6c25333fbfa0e4b9e0a79b2c2cecce7ffdf499ef2b4720e5723c59f766306 |
| corpus SHA-256 | 50fe9fd8744e98a1f9caadf71ee22fe4ba7857862cb9ff193515f69ab44218a3 |
| comparison harness SHA-256 | fad3f112d08071a5490076730e6ebe1a6d510f7a5aa9e2ac54ab0200303245b1 |
| host | macOS 26.6.1, Mac16,10, arm64, 10 CPUs, 16 GiB |
| Rust / nextest | rustc 1.95.0 / cargo-nextest 0.9.115 |
| Node / npm / Python | v26.7.0 / 11.19.0 / 3.14.6 |

The older untracked closeout drafts remain untouched. They bind an obsolete revision/API and contain Pending items; they are not evidence for this receipt.

## Performance evidence

The measured entry point is the public Renderer::render(RenderRequest::ascii(...)) path in crates/merman/benches/ascii_pipeline.rs. The runner is tools/bench/compare_self.py schema v2 with the ascii feature, plain ASCII output, one logical operation, a 10% relative threshold, a 50 microsecond absolute threshold, and Bonferroni-adjusted 95% simultaneous confidence.

Evidence attachments are in docs/performance/evidence/ascii-u25/99c567e89/.

### Medium comparison against the benchmark backport

medium-confirmation.json is 332381 bytes (SHA-256 9a0962b0326fa6890b7b5ec056c7c9c818a72cabf2ced09cad1ad473ac6256fd). Its Markdown projection is 1834 bytes (SHA-256 ed03661fe42480d7eed49482d894fff570622582b15852e5423d4e7ee92e9f7e). It contains two comparable rows and zero contract failures. Output identities matched. Both rows are statistically inconclusive: the relative intervals cross the registered decision boundary, while the absolute upper bounds remain below 50 microseconds.

| Fixture | Output identity | Base | Source | Absolute interval | Disposition |
| --- | --- | ---: | ---: | ---: | --- |
| sequence_medium | 3721 bytes / 4ac5a151d177b47e44b06b038d7496385dd066062b4e1032f54d8ce77ae488f0 | 120878.75 ns | 162705.00 ns | 40898.7..42637.5 ns | Accepted below absolute threshold; relative result inconclusive |
| class_medium | 2695 bytes / 4313de5080beb30197158cee630f0d5f3bcf94a9e5dd6ae5bded7781100f7fd7 | 313327.50 ns | 351071.25 ns | 34615.0..40036.3 ns | Accepted below absolute threshold; relative result inconclusive |

This is not a claim that the relative slowdown is zero.

### Five-family large A/A observation

large-aa-discovery.json is 71341 bytes (SHA-256 0535cb0b431257666a540b9bf64fce58362453d0f4344ee508ab272f0680afad). large-aa-confirmation.json is 1903950 bytes (SHA-256 ba2ae40a24b99dea3ddeb1539c1a36513672fdc2662729cc6e494522f3c00450). Their Markdown projections are 2047 bytes (SHA-256 f0044a64a6dae2d8ff859cfc9bc9da35e0d975938ccd833777232c5dc6d3f3d3) and 2275 bytes (SHA-256 b39e2f7ab226d6926a8891062d61c5ad947682c87cd91f683209226c0b2eb7cf). Five rows were comparable, with zero contract failures and matched output identities.

| Fixture | Output bytes / SHA-256 | Result |
| --- | --- | --- |
| flowchart_large | 66719 / 2640bece3a7df41c48b52e189845261d3ea1230ab78a7fa0a29786d48359ddae | Inconclusive: A/A calibration required 17890 pairs, above the 64-pair cap |
| sequence_mermaid_api_large | 147450 / 8aae5385d292539f2d3052ea2d32f9b0cc0aa8e6d5ecb8a13cbca54816f9ffeb | Inconclusive: A/A calibration was not stable within the pair cap |
| class_large | 16597 / 3e87c09709ca99dd5efaaf33147e8a26ff85e916643b3cfd020ce3e7034672a1 | Confirmed non-regression; relative interval -0.14%..+0.43% |
| er_large | 21802 / 25ffc3983eae86ed61b966958208dde07cd646b78dd61272c557e3a7afc67b83 | Confirmed non-regression; relative interval -0.35%..+0.40% |
| xychart_large | 11649 / 3637865ae01de610f9caf3dc55088bf6a65ec86a7dc28caa644e67e4b2aaeee | Confirmed non-regression; relative interval +0.13%..+0.85% |

The old baseline changes output identity for Class, ER, and XYChart. The runner correctly rejects those rows as causal A/B comparisons; this receipt does not claim old-version performance equivalence for them.

## Serial verification matrix

All commands used the repository default target directory and one build job where applicable:

- cargo fmt --all -- --check
- cargo nextest run -p merman-core -j 1: 1539 passed
- cargo nextest run -p merman-ascii -j 1: 1246 passed
- cargo nextest run -p merman-render -j 1: 1589 passed, 1 skipped
- cargo nextest run -p merman --no-default-features --features ascii -j 1: 30 passed
- cargo nextest run -p merman-bindings-core --no-default-features --features ascii -j 1: 150 passed
- cargo nextest run -p merman-uniffi --no-default-features --features ascii -j 1: 23 passed
- cargo nextest run -p merman-wasm --no-default-features --features ascii -j 1: 14 passed
- cargo nextest run -p merman-cli --no-default-features --features ascii -j 1: 80 passed
- strict render-path Clippy with -D warnings: passed
- strict ASCII bindings Clippy with -D warnings: passed
- cargo run --locked -p xtask -- verify-generated: passed
- python3 scripts/verify-platform-bindings.py: passed
- Web contracts, 126 Web tests, TypeScript build, and ASCII WASM build: passed
- Playground ASCII support, typecheck, examples (6 passed), and catalog verification: passed

## Safety, resource, and independent review

The static resource/panic audit found no new production P0/P1. Failure-terminal precedence, cancellation precedence, exact/N-1 boundaries, binding envelopes, and trace staging are covered by the serial matrix above. Independent Standards review of 16b841226...99c567e89 found no P0/P1 or static compilation blocker.

Accepted P2 residuals:

1. merman-core still exposes low-level terminal helpers through a doc-hidden public module.
2. OperationLedgerError is an obsolete public name for the operation-wide sticky terminal.
3. Canvas and relation tests still contain probe orchestration that should eventually use one production seam.
4. PR performance contracts and the scheduled/manual measurement workflow still overlap in ownership.
5. grapheme_safe_trim and some layered sorting/lane scans lack fine-grained mid-loop checkpoints; this receipt does not claim complete cancellation-latency closure.
6. AsciiRenderer::render_model remains a documented downgrade boundary for parser-owned CSS/source provenance and is not claimed as parser parity.

These are P2 follow-ups, not Pending items. The maintainer disposition is: accept the medium absolute-under-50-microsecond inconclusive observations; accept the large Flowchart/Sequence A/A statistical insufficiency; accept the confirmed Class/ER/XYChart large non-regressions; and exclude changed-output baseline rows from causal claims.

## Validity rule

This receipt is valid only for product source commit 99c567e89. Any later product, lockfile, fixture, benchmark-harness, CI-contract, or performance-evidence change requires a new source freeze and affected evidence rerun. The documentation child commit carrying this receipt is not a new product measurement point.
