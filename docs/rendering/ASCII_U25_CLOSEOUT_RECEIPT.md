# ASCII U25 Closeout Receipt

Status: source-bound evidence for the U25 closeout. The measured ASCII product source is commit 861fc0ba33ac6f0a724263b3a6f303a3f26eee15. This receipt and its attachments are documentation-only evidence for that source point; they are not a whole-repository receipt for unrelated render families or platform changes carried by later commits.

## Evidence boundary

| Item | Value |
| --- | --- |
| measured source commit | 861fc0ba33ac6f0a724263b3a6f303a3f26eee15 |
| measured source tree | fc552d5c52f9555c619a67294d523baaf2c89203 |
| measured tracked state | clean detached measurement worktrees; protected legacy drafts and platform directories in the main worktree remained untracked and were excluded |
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

The benchmark projection, scorecard, and resource audit in this tree are indexes of this receipt; they do not carry independent revision or result authority.

## Performance evidence

The measured entry point is the public Renderer::render(RenderRequest::ascii(...)) path in crates/merman/benches/ascii_pipeline.rs. The runner is tools/bench/compare_self.py schema v2 with the ascii feature, plain ASCII output, one logical operation, a 10% relative threshold, and Bonferroni-adjusted 95% simultaneous confidence. The 50 microsecond absolute value is the preregistered materiality boundary for this low-latency lane, not a repository-wide hard limit; other lanes may register a profile-specific formula or structural objective as described in RUNBOOK.md.

The tracked evidence attachments are the Markdown projections in docs/performance/evidence/ascii-u25/861fc0ba3/. The schema-v2 JSON files are build artifacts under target/performance/u25/, not durable repository files, in accordance with the performance runbook; their names, sizes, and SHA-256 values below identify the retained local/CI artifacts.

### Medium comparison against the benchmark backport

current-medium-861fc.json (target artifact) is 405318 bytes (SHA-256 1dc13406aa71ab9c6063f0f1462489fa6b4076d95318d374a80304a4493f7164). Its tracked Markdown projection is 1220 bytes (SHA-256 9a18f3c5b2b89cd21bf98e005c05d7149a12df8ed2376aacc2c088acdb61f3e2). It contains two comparable rows and zero contract failures. Output identities matched. Both rows are statistically inconclusive: relative intervals cross the registered decision boundary, while absolute upper bounds remain below 50 microseconds.

| Fixture | Output identity | Base | Source | Absolute interval | Disposition |
| --- | --- | ---: | ---: | ---: | --- |
| sequence_medium | 3721 bytes / 4ac5a151d177b47e44b06b038d7496385dd066062b4e1032f54d8ce77ae488f0 | 118120 ns | 161000 ns | 41100..44850 ns | Accepted below this lane's absolute materiality boundary; relative result inconclusive |
| class_medium | 2695 bytes / 4313de5080beb30197158cee630f0d5f3bcf94a9e5dd6ae5bded7781100f7fd7 | 307780 ns | 341940 ns | 32530..35910 ns | Accepted below this lane's absolute materiality boundary; relative result inconclusive |

This is not a claim that the relative slowdown is zero.

### Five-family large A/A observation

current-large-aa-861fc.json (target artifact) is 1079620 bytes (SHA-256 442225004208d2927e5f9e15bb23de3d52533e93424cad01c7a807e6205805ff). Its tracked Markdown projection is 1176 bytes (SHA-256 6ab83718731e85ef4125885a98f0b49688c214ee4ec08f711502b4592b4e7083). Five rows were comparable, with zero contract failures and matched output identities; Flowchart, Sequence, and Class A/A calibration remained inconclusive, while ER and XYChart showed same-source A/A stability. This is supporting measurement evidence, not a cross-version product comparison.

| Fixture | Output bytes / SHA-256 | Result |
| --- | --- | --- |
| flowchart_large | output identity matched | Inconclusive: A/A calibration did not stabilize within the registered pair cap |
| sequence_mermaid_api_large | output identity matched | Inconclusive: A/A calibration did not stabilize within the registered pair cap |
| class_large | output identity matched | Inconclusive: A/A calibration did not stabilize within the registered pair cap |
| er_large | output identity matched | Same-source A/A stability observed; relative interval -0.44%..+0.84% |
| xychart_large | output identity matched | Same-source A/A stability observed; relative interval -0.15%..+0.98% |

The old baseline changes output identity for Class, ER, and XYChart. The runner correctly rejects those rows as causal A/B comparisons; this receipt does not claim old-version performance equivalence for them.

## Serial verification matrix

All commands used the repository default target directory and one build job where applicable:

- cargo fmt --all -- --check
- cargo nextest run -p merman-core -j 1: 1539 passed
- cargo nextest run -p merman-ascii -j 1: 1246 passed
- cargo nextest run -p merman-render -j 1: 1591 passed, 1 skipped (after the Flowchart stylesheet preflight change)
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

The static resource/panic audit found no new production P0/P1. Failure-terminal precedence, cancellation precedence, exact/N-1 boundaries, binding envelopes, and trace staging are covered by the serial matrix above. The independent review through the preceding source freeze found no P0/P1 or static compilation blocker; the current Flowchart stylesheet change then passed the full 1,591-test render package gate.

Accepted P2 residuals:

1. merman-core still exposes low-level terminal helpers through a doc-hidden public module.
2. OperationLedgerError is an obsolete public name for the operation-wide sticky terminal.
3. Canvas and relation tests still contain probe orchestration that should eventually use one production seam.
4. PR performance contracts and the scheduled/manual measurement workflow still overlap in ownership.
5. grapheme_safe_trim and some layered sorting/lane scans lack fine-grained mid-loop checkpoints; this receipt does not claim complete cancellation-latency closure.
6. AsciiRenderer::render_model remains a documented downgrade boundary for parser-owned CSS/source provenance and is not claimed as parser parity.

These are P2 follow-ups, not unresolved closeout rows. The maintainer disposition is: accept the medium observations as below this lane's 50-microsecond materiality boundary while retaining their relative inconclusive status; accept the large Flowchart/Sequence/Class A/A statistical insufficiency; retain the ER/XYChart same-source A/A stability observations as supporting evidence only; and exclude any changed-output baseline rows from causal claims.

## Validity rule

This receipt is valid for the U25 evidence closure at product source commit 861fc0ba33ac6f0a724263b3a6f303a3f26eee15 and tree fc552d5c52f9555c619a67294d523baaf2c89203. A later change requires a new source freeze and affected evidence rerun when it changes the ASCII production path, Cargo.lock, the ASCII benchmark harness or corpus, a recorded U25 verification contract, or the retained performance evidence. Changes outside that closure, such as another SVG family's implementation, a test-only stack allowance, generated documentation receipts, or CI size budgets, require their own gates but do not rewrite the measured ASCII result. The 50 microsecond value is lane-specific materiality, not a universal pass/fail promise.
