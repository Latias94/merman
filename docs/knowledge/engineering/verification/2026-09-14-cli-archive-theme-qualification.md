# CLI archive theme qualification — 2026-09-14

Source: `83c6405988f7a06e3e1bc456f6d92e8a0b75ff40`.

## Result

A detached clean checkout built the canonical macOS ARM64 CLI/LSP archives with cargo-dist
0.32.0 and Rust 1.95.0. Archive contents, checksums, and extracted-binary runtime checks passed.
The CLI archive then passed preset qualification and its independent rebuild/reexecution check.
The qualifier matched all 18 CLI outputs to the Rust-owned observations: Brutalist, Spotless,
and Cyberpunk, each with Flowchart/State/Sequence SVG and PNG. The accompanying catalog was
projected from the exact CLI metadata and fresh execution record, then checked during replay.

All 18 preset cells retain `host_dependent` admission under
`native-flowchart-state-sequence-system-fonts-v3`. The three recipes remain alpha; the seven
retained compatibility presets keep empty scope. This macOS observation does not qualify the
Linux publication artifact or promote the shared Rust/SDK catalog. No fonts or extra theme rules
were injected. These are development builds at the current workspace version `0.8.0-alpha.6`;
no tag, registry, or GitHub Release was changed.

## Exact artifacts

| Archive | Archive bytes | Binary bytes | Archive SHA-256 |
| --- | ---: | ---: | --- |
| CLI aarch64-apple-darwin | 13,491,120 | 51,325,504 | `f7ae969cf8499014e4fc4046515f7b0d2878345cc24cfa9346ed75744faf24cf` |
| LSP aarch64-apple-darwin | 4,053,572 | 17,807,376 | `f33ae02eb919e9c66dc480469c075f87c83c81cfb3a98429db5dbe1a48bf1660` |

The qualified CLI executable SHA-256 is
`3013daba112465ec057cdc2a71c582c513630fc4128f7e1a2dbcb50a142ecf67`.
The record and companion bind that executable, archive, source, host, recipe/resource identities,
and scoped output observations. Archive hashes were checked again after the verification run.
These sizes describe this build, without a same-profile historical A/B or a new budget decision.
They do not measure startup cost or peak memory.

## Public authoring and candidate prerequisites

The safely extracted CLI also consumed the shared light/dark definition fixtures through
`render --theme-definition` and their canonical complete specs through `render --theme-file`.
For each mode and each of the three qualification scenarios, SVG and PNG bytes matched between
those two inputs: **12 matching pairs, 24 successful process renders**. Light and dark outputs
differed in all six family/output combinations, preventing an ignored theme from satisfying
this comparison. The checks used `htmlLabels: false`, the `resvg-safe` SVG pipeline, and the
runner's recorded PNG scales. They establish transport equivalence for these inputs, not new
qualification cells or arbitrary-source portability.

The same clean source passed the existing independent Release owners:

- `c6_runtime`: **2/2 tests, no skips**, validating the fixed 18-cell / nine-render-group C6a
  ledger and its separate eligibility issuer.
- `theme_authoring`: **13/13 tests, no skips**, including light/dark state isolation, complete
  spec cold start, generated rows/palettes, root/typography terminals, representative family
  shapes, typed/JSON family rules, discovery, errors, and resource boundaries.
- The Python archive/qualification/catalog contract suites passed **67/67 tests** in the main
  worktree at the same source.

The standalone C6a ledger and the 18 preset observations have different authorities and scopes;
their counts must not be added into an expanded C6a eligibility claim.

## Reproduction

Run in a clean checkout. This run reused the main workspace target through `CARGO_TARGET_DIR`,
used two Cargo build jobs, and ran Cargo operations serially. cargo-dist writes archives under
the clean checkout's `target/distrib` directory.

```text
dist build --tag=v0.8.0-alpha.6 --artifacts=local --target=aarch64-apple-darwin
python3 scripts/verify_cli_release_archive.py <cli-archive> --checksum <cli-checksum> --target aarch64-apple-darwin --version 0.8.0-alpha.6 --repo-root . --execute --preset-qualification-output <record.json>
python3 scripts/verify_cli_release_archive.py <cli-archive> --checksum <cli-checksum> --target aarch64-apple-darwin --version 0.8.0-alpha.6 --repo-root . --execute --preset-qualification-check <record.json>
python3 scripts/verify_lsp_release_archive.py <lsp-archive> --checksum <lsp-checksum> --target aarch64-apple-darwin --version 0.8.0-alpha.6 --repo-root . --execute
python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman-theme-acceptance --no-default-features --features png,layout-cytoscape --test c6_runtime --test-threads 2
python3 scripts/run_theme_acceptance.py nextest run --release --locked -p merman --no-default-features --features svg,png --test theme_authoring --test-threads 2
python3 -m unittest scripts.test_verify_cli_release_archive scripts.test_qualify_theme_presets scripts.test_theme_preset_catalog_contract
```

The definition/spec process comparison uses the checked-in
`crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/{light,dark}.definition.json`
and corresponding `*.spec.canonical.json`, wrapped as `{"spec": ...}` for `--theme-file`.
Sources and PNG scales come from the first preset's cells in the fresh qualification record;
no additional source fixture or assertion framework was added.

The clean checkout's tracked and untracked Git status was empty before and after all checks.
The existing main-worktree knowledge directories were left untouched. Build logs, commands,
archive identities, qualification record/companion, CLI comparison results, and final checkout
status are retained under `target/bench/experiments/theme-cli-archive-revision90/`.

## Remaining delivery boundary

This closes the current macOS CLI archive/qualification and candidate-prerequisite verification
slice. It does not close C7a-candidate, rollout, or contract freeze. Complete the outstanding
Web/Typst/Python and cross-host artifact verification, reconcile the public discovery/catalog
rollout against its declared artifact profiles, and assemble the final candidate checklist from
existing owner evidence. The earlier 4899-test bridge/provider retirement run retains its own
source and scope; this run did not repeat the full workspace or browser matrices.
