# Theme assignment WASM size reduction — 2026-09-12

## Decision

Keep the extraction of `ThemeState::commit_computed` from generic `ThemeState::assign`.
It shares dependency expansion and map updates across the assignment callbacks instead of
instantiating that body for each callback. The callback still runs once, before mutation, and
its error returns before any assignment. Truthy/nullish short-circuit rules, dependency expansion,
subtree replacement, explicit replay, and theme iteration limits are unchanged.

This is a measured artifact-size improvement. It does not establish a latency improvement or
close the Web size gate: all five profiles still exceed the existing budgets. No budget,
capability, output policy, resource limit, or diagnostic contract was changed.

## Controlled comparison

Base: `227c89531`, with runtime artifacts built at its runtime-identical parent `2401e2ee1`.
Candidate: the `staged.rs` extraction committed with this report. Base artifact hashes and
installation evidence are recorded in
[the artifact checkpoint](../knowledge/engineering/verification/2026-09-12-theme-artifact-2401e2ee1.md).
Base WASM files and provenance were recovered from those recorded tarballs. Candidate artifacts
were rebuilt using the same Web scripts, profile features, Cargo lockfile, host, and toolchain.

One final artifact per profile per variant was measured. Both variants were measured with the same freshly
compiled Release `xtask`; base and candidate commands returned the expected over-budget failure
after emitting all measurements. Their raw sizes were also checked directly against their files.
Gzip and Brotli measurements use the existing stripped-artifact pipeline.

The preregistered slice criterion was at least 0.5% improvement in ASCII raw and gzip bytes, with
no size growth in the other profiles and unchanged semantic gates. ASCII raw decreased 2.06%
and gzip decreased 0.69%; raw, stripped, gzip, and Brotli bytes decreased in all five profiles.

| Profile | Base raw | Candidate raw | Raw bytes saved | Base gzip | Candidate gzip |
| --- | --- | --- | --- | --- | --- |
| `web-analysis` | 3,784,417 | 3,674,988 | 109,429 | 1,426,681 | 1,412,184 |
| `web-ascii` | 5,307,546 | 5,198,118 | 109,428 | 1,928,170 | 1,914,933 |
| `web-editor` | 3,895,567 | 3,786,123 | 109,444 | 1,470,276 | 1,456,747 |
| `web-full` | 15,935,669 | 15,824,788 | 110,881 | 5,923,271 | 5,907,959 |
| `web-render` | 14,048,792 | 13,938,111 | 110,681 | 5,278,212 | 5,263,344 |

Host: macOS ARM64. Toolchain:

```text
rustc 1.95.0 (59807616e 2026-04-14)
binary: rustc
commit-hash: 59807616e1fa2540724bfbac14d7976d7e4a3860
commit-date: 2026-04-14
host: aarch64-apple-darwin
release: 1.95.0
LLVM version: 22.1.2
wasm-opt version 131
wasm-pack 0.15.0
wasm-tools 1.253.0
v26.6.0
```

The ignored experiment ledger and full metrics are under
`target/bench/experiments/theme-wasm-size-227c89531/`. They include lockfile, artifact and
measurement-tool digests, attribution output, and the candidate diff digest.

## Attribution and limits

`twiggy top` over the named, pre-wasm-bindgen ASCII artifact identified the three staged theme
update functions among the largest functions. After extraction, the linked artifact shrank from
8,202,094 to 8,073,444 bytes while `.rodata` remained unchanged at 1,096,058 bytes. This linked
artifact includes metadata removed by the release pipeline; the final package measurements above
are the admission evidence. The code-sharing explanation does not attribute the entire historical
size regression to this assignment function.

The optimized final ASCII WASM remains 73,118 bytes above its raw budget; render remains 1,313,111
bytes above its raw budget. Further work must attribute those remaining costs and preserve the
same product contracts. Removing debug names alone cannot solve the final package overruns:
the optimized package WASM already lacks that section.

## Verification

- 54 core theme tests passed, including source-backed stage oracles, explicit overrides,
  dependency ownership, nested fields, and theme iteration-limit rejection.
- All five Web profiles rebuilt. Full `npm run smoke --prefix platforms/web` passed, including
  input freshness, package contracts, actual WASM execution, profile isolation and DOM safety.
- Full SVG structure comparison passed.
- Core Clippy with `operation-deadlines`, `cargo fmt --all -- --check`, and diff hygiene passed.
- The unchanged global size-budget gate remains failed; the measurements above do not replace it.

Commands:

```text
CARGO_BUILD_JOBS=2 cargo nextest run --locked -p merman-core --lib -E 'test(theme::)' --cargo-quiet --test-threads 2
CARGO_BUILD_JOBS=2 npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
CARGO_BUILD_JOBS=2 RAYON_NUM_THREADS=2 cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
CARGO_BUILD_JOBS=2 cargo clippy --locked -p merman-core --lib --no-default-features --features operation-deadlines
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- wasm-size-matrix --surface web --web-package-root platforms/web/packages --budget-file docs/release/WASM_SIZE_BUDGETS.json
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- wasm-size-matrix --surface web --web-package-root target/bench/experiments/theme-wasm-size-227c89531/baseline-web --budget-file docs/release/WASM_SIZE_BUDGETS.json
```
