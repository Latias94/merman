---
type: Verification Evidence
title: Clean native CLI archive qualification and open metadata consumer checks
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
git_commit: 5c0d1a3a5e8b9a8e5c65b9672e7ff4dbe0518b34
tags: theme,c7a,cli,artifacts,qualification,verification
---

# Exact source and artifact

The final macOS ARM64 CLI archive was rebuilt from the clean detached checkout
`5c0d1a3a5e8b9a8e5c65b9672e7ff4dbe0518b34`, version `0.8.0-alpha.6`, at
`/tmp/merman-c7a-cli-5c0d1a3a5`. Cargo builds ran sequentially with two jobs and the existing
primary-worktree target directory. Build logs identify the core, renderer, facade, CLI and
qualification runner under this detached checkout. The CLI used cargo-dist's repository-owned
`dist` profile and `aarch64-apple-darwin` release recipe. Its command also built the LSP archive;
this record only claims the CLI verification described below.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13459940 | `ed1c123dc79aefda3827985bf9c56c24dc4bf82ac09bee5c46600a003cfbef11` |
| `preset-qualification-5c0d1a3a5.catalog.json` | 14640 | `a02c32a1046fb64644f288543fa6e0898e6ba24fe7bcf035dcb8f1c04b5ef26e` |

The extracted CLI executable SHA-256 is
`b20cc7158e83f6d8126e4b3838b3df705d4b12cf609bf0cc0ca4d30506f68350`.
The independently built qualification runner SHA-256 is
`e83618b43f0124041167f6cf0a29dea9933f25f518eca37d8342cac581dd648f`.

# Executed observations

The existing archive verifier checked the archive checksum, contents and legal assets, then
executed the extracted binary's runtime contract. The private Rust runner qualified the exact
Brutalist, Spotless and Cyberpunk recipes. Each preset covered Flowchart, State and Sequence in
SVG and PNG, for **18 cells** under `native-flowchart-state-sequence-system-fonts-v3`.
All cells were `host_dependent`; none was upgraded to `portable`.

The extracted production CLI rendered the same 18 inputs. Every output digest matched the
runner's qualified bytes. The resulting public companion retained the ten-entry catalog:
the three tested recipes each carried six scoped cells and the other seven retained empty
qualification arrays. This companion is separate from the shared production catalog, which
remains unqualified.

A second invocation with `--preset-qualification-check` rebuilt/reused and reexecuted the runner,
rerendered the actual CLI outputs, and compared both complete stored records and the public
companion. It passed. An independent archive read confirmed that the executable bytes inside
the final archive match the recorded executable SHA-256, and the companion binds the same source
and archive digest. Git status was empty after collection and again after replay. Existing
compiler warnings were emitted; no compilation or verification command failed.

# Reproduction

Run from the detached checkout with the pinned repository toolchain. Set `CARGO_BUILD_JOBS=2`
and `CARGO_TARGET_DIR` to the existing primary-worktree target directory; run builds sequentially.

```text
dist build --tag=v0.8.0-alpha.6 --artifacts=local --target=aarch64-apple-darwin
python3 scripts/verify_cli_release_archive.py target/distrib/merman-cli-aarch64-apple-darwin.tar.xz --target aarch64-apple-darwin --version 0.8.0-alpha.6 --repo-root . --execute --preset-qualification-output target/preset-qualification-5c0d1a3a5.json
python3 scripts/verify_cli_release_archive.py target/distrib/merman-cli-aarch64-apple-darwin.tar.xz --target aarch64-apple-darwin --version 0.8.0-alpha.6 --repo-root . --execute --preset-qualification-check target/preset-qualification-5c0d1a3a5.json
```

Logs are `/tmp/theme-c7a-cli-5c0d1a3a5-{build,qualification,replay}.log`. The bounded artifact
summary, hashes and clean-checkout observations are in
`/tmp/theme-c7a-cli-5c0d1a3a5-state.json`. The execution record and companion are retained under
the detached checkout's `target/`. The static release surface contract also passed:
`python3 scripts/release_surface_contract.py --version 0.8.0-alpha.6`; its five declarations
are configuration checks, not publication observations.

# Open metadata consumers at a separate source

Commit `48f7f485c52e58b6c160174497ba29010691382d` adds the independent shared
`authoring-v1/qualified-cells.json` vectors. Rust contract roundtrips, shared binding metadata
projection, Web catalog consumption and Node metadata text transport now vary unknown profile
and admission IDs independently, including `future-profile` with `portable` and the current
native profile with `future-admission`. They preserve every field. Rust and Web also retain a
legal two-profile success case; Web checks defensive copies of the second row.

These are synthetic inputs. Node uses its existing mock transport, and Web uses its existing
runtime fixture; neither test issues qualification or proves installed artifacts. Existing
shared authoring/error/support/preset-catalog vectors already have C ABI, UniFFI, Typst, Node
and Web consumers. This change fills the independent open-ID coverage gap without adding
production inference, a profile registry or test-only production entry points.

Final working-tree checks passed Rust **2/2**, Node/Web **48/48**, TypeScript compilation,
`cargo fmt --all --check` and `git diff --check`. Review found a lost multi-cell positive case;
it was restored in Rust/Web before final verification. No review issue remains in this slice.
A fresh checkout at `/tmp/merman-c7a-qualified-cells-48f7f485c` then passed Rust **2/2** and
Node/Web **48/48** after rebuilding Web TypeScript. Cargo reused the existing target directory;
TypeScript used the existing dependency directory through a temporary symlink that was removed
after compilation. This was not a cold dependency installation. Final checkout status was empty.

```text
cargo nextest run --release --locked --lib -p merman-theme-contract -p merman-bindings-core --features merman-bindings-core/svg -E 'test(preset_metadata_round_trips_open_identifiers_and_host_conditions) | test(preset_qualification_projection_preserves_host_conditions_and_future_ids)'
# From platforms/web, with the existing dependencies available:
./node_modules/.bin/tsc -p tsconfig.build.json
# From the checkout root:
node --test platforms/node/tests/api-contract.test.mjs platforms/web/scripts/theme-catalog.test.mjs
```

Clean-source logs are `/tmp/theme-c7a-qualified-cells-{rust,js,ts}-clean.log` and
`/tmp/theme-c7a-qualified-cells-clean-state.json`. The Rust filter skips 271 other library tests;
it is not a complete crate/workspace run. The CLI artifact above remains bound to `5c0d1a3a5`,
not relabeled as a build from this later test-only commit.

# Delivery boundary

This is a real macOS archive/runner/CLI observation at the named source. It does not execute the
Linux-only final publication bundle lane introduced by `a7a4662f5`, publish a catalog or package,
change a release tag, or certify other hosts. No current Node, Web, Python or Typst package build,
full browser suite, or complete workspace suite is claimed here. Previous installed-consumer
records remain bound to their original source and artifacts. Remaining artifact matrix, public
discovery rollout and contract-freeze decisions keep C7a open; C7b bridge retirement is separate.
