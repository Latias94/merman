---
type: Verification Record
title: Alpha.7 current-source CLI and LSP archive replay
timestamp: 2026-09-20
source_commit: 58f3a15310608a007600afe1fd9cd40083adcfe9
---

# Result

The cargo-dist `0.32.0` local ARM64 build was rerun from the current source tree with the
descriptor-owned `dist` profile and `--artifacts local --target aarch64-apple-darwin`. Both
current-source archives passed their structural and execute-mode verifiers:

| Archive | Bytes | SHA-256 | Verification |
| --- | ---: | --- | --- |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13,741,072 | `84e4961792fd2c58d040cce5f497f4c321938e670b0c2eefc951414594963594` | `verify_cli_release_archive.py --execute` |
| `merman-lsp-aarch64-apple-darwin.tar.xz` | 4,076,964 | `fed24cc9cd1612f747e0843a05d04e2aaf90ff6518c393d7f20a0eaae6e6a95a` | `verify_lsp_release_archive.py --execute` |

The verifier checked the adjacent checksum, target and version, exact legal payloads, and the
real archived binaries. The CLI smoke and LSP stdio lifecycle both exited successfully. Verified
copies and logs are retained under `target/bench/experiments/archive-replay-20260920/`; the
`dist-build.json` hash is `22fdb7c1590d9eb9f690704c442bab86e3b30155535badb56f9d1ce5d835a3d5`.

# Stale archive boundary

Before rebuilding, the previously retained archives were replayed against the current source and
both failed because `THIRD_PARTY_LICENSES/rust-cargo-dependencies.json` differed from the current
tracked legal projection. Those files were not treated as current evidence and were replaced by
the current-source cargo-dist output. This confirms why archive replay must bind the source and
legal tree together.

This record proves only macOS ARM64 archive assembly and execution. Linux, Windows and Intel
archive execution remain explicitly unverified; no hosted workflow or publication was performed.
