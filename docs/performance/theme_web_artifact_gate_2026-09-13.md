---
type: Performance Decision
title: Current Web artifact gate and rejected Binaryen size-profile experiment
timestamp: 2026-09-13
git_commit: eda8c3846692803f37aadb7daa45fe32c25c04bf
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
tags: theme,wasm,artifact-size,web,c7a,verification
---

# Decision and source

Keep the existing build recipe and size budgets. At `eda8c3846692803f37aadb7daa45fe32c25c04bf` all five Web
profiles pass the production package smoke, and freshly installed packages load through their
public browser entries in real Chromium. **All twenty final WASM size checks still fail.**
These functional observations do not admit the release or close C7a.

The isolated checkout is `/tmp/merman-theme-wasm-eda8c3846`, using macOS ARM64, Rust 1.95.0,
wasm-pack 0.15.0, Binaryen 131, wasm-tools 1.253.0 and Node 26.6.0. The exact profile recipes
come from `capabilities/artifact-profiles-v1.json`. Cargo already uses `opt-level = "z"`, LTO,
and one codegen unit. Builds ran serially with `CARGO_BUILD_JOBS=2` and the existing shared target.
Web TypeScript used the existing dependency directory through a temporary symlink; it was removed
before the final clean-status check. This was a source rebuild, not a cold dependency installation.

# Current final package sizes

A freshly built Release `xtask` measured the final assembled npm-package WASM, using the existing
strip/Gzip/Brotli pipeline and unchanged `docs/release/WASM_SIZE_BUDGETS.json`. Every measurement
exceeds its corresponding budget, including the three slim profiles. These are artifact bytes,
not latency observations.

| Profile | Raw | Stripped | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| `web-analysis` | 3,674,836 | 3,674,571 | 1,412,173 | 1,075,009 |
| `web-ascii` | 5,197,963 | 5,197,698 | 1,915,011 | 1,448,198 |
| `web-editor` | 3,785,967 | 3,785,702 | 1,456,635 | 1,105,668 |
| `web-full` | 15,880,100 | 15,879,835 | 5,928,770 | 4,365,870 |
| `web-render` | 13,994,997 | 13,994,732 | 5,283,021 | 3,895,487 |

The previous assignment extraction remains landed; this record does not revert it or attribute
the entire overrun to one theme function. Compared with the fixed raw limits, the remaining excess
is 74,836 bytes for analysis, 72,963 for ASCII, 10,967 for editor, 1,080,100 for full and 1,369,997
for render. Compressed bytes are independent gates, not inferred from raw size.

# Rejected post-link experiment

wasm-pack 0.15.0's `manifest/mod.rs` sets an unspecified custom post-link profile to `-O`.
The local candidate explicitly selected `wasm-opt = ["-Oz"]` in
`merman-wasm`'s custom-profile metadata, leaving Cargo, features, locks and tool versions unchanged.
The registered render threshold was at least 2% less raw data and 1% less Gzip data, with no growth
in control metrics before broader confirmation.

| Render metric | Baseline | Direct `-Oz` recipe | Change |
| --- | ---: | ---: | ---: |
| Raw | 13,994,997 | 13,811,170 | -183,827 (-1.31%) |
| Stripped | 13,994,732 | 13,810,905 | -183,827 |
| Gzip | 5,283,021 | 5,290,701 | +7,680 |
| Brotli | 3,895,487 | 3,903,478 | +7,991 |

Reject the candidate: raw reduction missed the threshold, both compressed metrics regressed,
and all four render limits still failed. No timing or behavior-admission run was warranted after
this failure. An earlier `-Oz` pass on a copy of the already optimized module produced 13,805,108
raw bytes; that was attribution only and was not substituted for the direct-recipe measurement.

The only edited manifest was restored after checking its complete bytes against the saved original.
The original render recipe was rebuilt and packages reassembled. **All five restored WASM files
match the saved baseline byte for byte.** Production package smoke then passed again. No optimizer
change, budget relaxation, capability removal, or unverified optimization was committed.

Fresh `twiggy` output also rules out a simplistic interpretation of the biggest shared functions:
the roughly 102 KB raw-family dispatcher invokes different writers, and the roughly 60 KB evidence
merge contains different family receipts. Common root finalization and the final evidence merge
already live outside the main family match. These are not thirty-three copies of one removable
body. The unoptimized render symbol inventory has 95,310 shallow bytes mentioning the theme
contract and 176,882 mentioning `diagram_theme`; generic names can overlap other owners. Debug
names, metadata and shallow symbol sizes are attribution data, not final-package savings.

# Packed and installed browser observations

The existing `web_package_group.py` packed all five public packages, then verified their bytes,
profile/resource paths, legal material, provenance and package-group contract. The final group
manifest records the full source SHA above. Initial local packing supplied its abbreviated ID;
`create-manifest` restamped the unchanged tarballs with the full ID before verification and
installation. No package was published.

| Installed package | Packed bytes | SHA-256 |
| --- | ---: | --- |
| `@mermanjs/web-analysis` | 1,509,267 | `3fee323b3558f5f81f964e854dbb1ed19f9852710499446fbfe3fdc8dafe4307` |
| `@mermanjs/web-render` | 5,423,173 | `5b27320c1a11d3f94c22951799486614c98193adfcae86fa32acc5778aaec5d7` |
| `@mermanjs/web-editor` | 1,559,186 | `73ebaab45c74644e07bbd31f4412f23c279699ed7a6c1c5f0e6c0cf491056a18` |
| `@mermanjs/web-ascii` | 2,010,128 | `29dcae5210bdc4067e42c196843726df2aedb76e03170de91f7b0fcb3f2b1979` |
| `@mermanjs/web` | 6,079,936 | `6ebbbb02fb2e4714f21ede08606fbf4da2d055d650b2d1e19dbf7f118bb77448` |

A fresh consumer installed the tarballs with `--ignore-scripts --offline --no-audit --no-fund`.
Chromium **Chrome/151.0.7922.34** then loaded each installed public `dist/package-entries` module
through a local HTTP server. The recorded requests include all five installed WASM files; no
workspace runtime or mocked transport supplied these browser executions.

For each of full and render, the browser checked the complete ten-preset golden, fourteen
revision-85 support responses, three complete authoring/resource-error vectors, and two shared
materialized light/dark spec oracles. Flowchart, State and Sequence each passed light/dark/light
isolation. A State fill-rule edit changed the actual Active shape to `#123abc` without changing
Sequence output. Three preset exports rendered successfully. Each package produced fifteen SVGs;
three were mounted with positive dimensions. Slim entries initialized successfully, exposed empty
structured-theme catalogs, and did not expose `materializeTheme`.

The driver initially failed before browser execution because Puppeteer's executable lookup is
asynchronous and its default Chrome installation was absent; the existing Chromium executable
was selected explicitly. Its first State assertion used an incorrect historical class name; it
was corrected to the existing `statediagram-state`/Active fixture identity and raw fill declaration.
The final complete five-package run passed. These driver corrections are not product defects.
This bounded witness is not the full browser suite, per-text visibility qualification, or a
performance comparison. Shared preset cells remain empty and no result becomes Portable.

# Reproduction and remaining work

From the detached checkout, with the shared target and two build jobs:

```text
npm run build --prefix platforms/web
npm run smoke --prefix platforms/web
cargo build --locked --release -p xtask
<shared-target>/release/xtask wasm-size-matrix --surface web --web-package-root platforms/web/packages --budget-file docs/release/WASM_SIZE_BUDGETS.json
python3 scripts/web_package_group.py pack --root . --descriptor platforms/web/web-surface-descriptor.json --artifact-dir target/web-package-group-eda8c3846 --version 0.8.0-alpha.6 --source-sha eda8c3846692803f37aadb7daa45fe32c25c04bf --target-dist-tag alpha
python3 scripts/web_package_group.py verify-artifact --manifest target/web-package-group-eda8c3846/web-package-group.json --artifact-dir target/web-package-group-eda8c3846 --version 0.8.0-alpha.6 --source-sha eda8c3846692803f37aadb7daa45fe32c25c04bf --descriptor platforms/web/web-surface-descriptor.json
```

The size command is expected to fail on the recorded source. Raw measurements, fixture/manifest
snapshots, exact artifact/tool digests, candidate bytes, the installed-browser driver and final
state live under `target/bench/experiments/theme-wasm-size-eda8c3846/`. Logs use
`/tmp/theme-wasm-eda8c3846-{build,smoke,size-base,oz-render-build,oz-render-size,restored-build,
restored-smoke,pack,manifest,verify-package,browser-confirmed}.log`.
The consumer path is recorded in `/tmp/theme-wasm-eda8c3846-consumer-path.txt`.
The measurement tool and driver versions/snapshots remain retained; no benchmark machinery was
added to ordinary CI. Final detached-checkout status is empty.

The current performance priority is to attribute the remaining capability-preserving size growth
against the budget anchors and test measured ownership/code-sharing candidates. Simple optimizer
flag substitution and treating aggregate family dispatch as duplicated code do not close this gate.
Native/Typst/Node artifact qualification, the Linux publication run, full browser regression and
formal C7a rollout/freeze remain separate. C7b bridge retirement also remains open.
