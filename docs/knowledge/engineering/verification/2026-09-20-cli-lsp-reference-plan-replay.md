---
type: Verification Record
title: CLI and LSP archive replay after reference graph reuse
timestamp: 2026-09-20T18:52:21Z
source_commit: f3a783c2944a77bc3b561b29f64b41af450ebe8b
---

# Source and archive execution

The macOS ARM64 CLI and LSP archives were rebuilt from immutable source
`f3a783c2944a77bc3b561b29f64b41af450ebe8b`, including the reference-graph allocation change in
`b707f6e5b`. Subsequent changes through `043106442` are documentation only. cargo-dist 0.32.0's
local download was rechecked against the recorded SHA-256
`aa343b2ff78ec2981f17a65140250c5ad6062c74072163f68c5c2686d94763a7`.

The non-publishing dist plan passed the repository's artifact-bundle verifier for the
`macos-15` / `aarch64-apple-darwin` route. Execution was local macOS 26.6.2 ARM64 with Rust
1.95.0, not a hosted macos-15 job. The canonical `dist` profile built both archives serially
with `CARGO_BUILD_JOBS=1` and the existing target cache.

| Archive | Packed bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13,732,212 | `b433d214180678f84ee5ec9116ec4ee3af96bf5e033ce7c4d8242fdd6be4aac1` |
| `merman-lsp-aarch64-apple-darwin.tar.xz` | 4,077,460 | `d35dd64071662f13c37b1c6b15a4d7ff3a080ad7f483385eec5aadca461d94c5` |

The archived CLI executable is 52,337,504 bytes, SHA-256
`e6a84bf056187b908c3305f620d3bd27774c044a4b2376cf63f6b5558f69fe57`.
The LSP executable is 17,950,032 bytes, SHA-256
`6646e5a0f35d35103d8c68b77a6816309df33d34d2f67a56eaf74ef16490e9fd`.
These are current artifact identities, not matched alpha.6 footprint attribution.

Both owner verifiers passed `--execute`, checking the target-specific archive layout, checksums
and exact tracked legal payloads before running the archived binaries. CLI execution checks
`--version`, capabilities, the Bash completion snapshot, valid SVG and real PNG/JPEG/PDF payloads.
LSP execution covers its framed initialize/shutdown/exit lifecycle and successful termination.
The LSP handshake has no version field; its version attribution uses the fixed-source build and
cargo-dist manifest.

An initial caller supplied a directory to `--verified-output`; the owner correctly rejected
that destination because it must retain the archive filename. The corrected invocation passed
with the same built bytes. The preserved final archives and checksums are under `verified/`;
no source or verifier change was required.

# Archive-bound qualification replay

After archive construction, the task-owned `target` cache symlink was removed from the detached
checkout. `CARGO_TARGET_DIR` continues to reuse the same cache. The qualification owner's original
clean-checkout rule then passes, including untracked files, before and after qualification.
Its existing build recipe uses two Cargo jobs; Cargo commands remain sequential and the recipe
was not modified.

The CLI archive passes both `--preset-qualification-output` and a subsequent
`--preset-qualification-check` against the saved record. Each invocation runs the existing Rust
qualification owner and compares the actual archived CLI's output bytes with its observations.
The second invocation reexecutes the qualification program and CLI, verifies source/lockfile/
executable identities, and compares the complete saved record and archive catalog companion.
It can reuse the unchanged compiled runner; this is a fresh execution check, not a forced
recompilation comparison.

| Preset | Profile | Families | SVG/PNG observations |
| --- | --- | --- | ---: |
| Brutalist | `native-flowchart-state-sequence-system-fonts-v1` | Flowchart, State, Sequence | 6 |
| Spotless | `native-flowchart-state-sequence-system-fonts-v1` | Flowchart, State, Sequence | 6 |
| Cyberpunk | `native-cyberpunk-full-scenes-system-fonts-v1` | Flowchart, Sequence, XY Chart | 6 |

All 18 observations are `host_dependent`. The local archive companion carries those 18 scoped
cells and binds the exact CLI archive SHA-256 above. The shared production catalog still has
zero qualified cells. This does not promote unrelated presets, portable fonts, PDF qualification,
other operating systems or other artifacts.

The qualification record SHA-256 is
`f1b47efe66a36a3081ebe82a75f03e5153564389d6b8b8ce3d18fada6f1dd910`;
its catalog companion is
`6186f85d3b9edf30632f5d507046f687ab5ac4cf1c64e927f1f0730b36711f8c`.
The full record binds source `f3a783c29`, the current lockfile, toolchain, executable, host and
observed recipe/resource identities. The companion carries the archive/executable, source, host,
profile, scenario and recipe/resource identities.

# Preparation checks and evidence

The archive, artifact-bundle, preset-qualification and catalog contract selection passes 110/110
Python tests. Version projections, release-surface declarations, CLI installation declarations
and preparation-mode changelog checks also pass at the fixed source.

Evidence is retained under `target/bench/experiments/archive-replay-f3a783c29/`, including the
plan/build JSON, tool versions, source-state checks, contract tests, final CLI/LSP verification
logs and readonly copies of the exact archives. These are local candidate artifacts, not
published release assets. The evidence identity manifest SHA-256 is
`6f8fbd4eb1ac1ae5e9a1e24d37f3e17f4d2cbdeff08c019833092a82e20fc2c3`.

The build command is `dist build --tag=v0.8.0-alpha.7 --artifacts=local
--target=aarch64-apple-darwin --output-format=json`. Each archive is passed to its existing
`verify_cli_release_archive.py` or `verify_lsp_release_archive.py` owner with the adjacent
checksum, target, version, exact source root and `--execute`. `--verified-output` receives a
new full archive path inside an existing nonsymlink directory. Reproduce qualification with the
same CLI verifier command plus `--preset-qualification-output <record.json>`, then
`--preset-qualification-check <record.json>`.

No tag, push, publication or hosted workflow was performed. Linux, Windows, Intel macOS,
registry installation, date-bound immutable preflight and broader performance admission remain
unverified or open. Passing these archive checks does not complete C7a.
