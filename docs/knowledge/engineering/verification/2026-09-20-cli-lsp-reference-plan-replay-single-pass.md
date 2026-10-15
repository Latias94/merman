---
type: Verification Record
title: CLI and LSP archives after XML reference single-pass integration
timestamp: 2026-09-20
source_commit: bdb209e1166af960121ce0e2b3231d9929d3bca1
---

# Source and archive execution

The macOS ARM64 CLI and LSP archives were rebuilt from clean integrated source
`bdb209e1166af960121ce0e2b3231d9929d3bca1`, which contains the accepted XML/reference
single-pass SVG optimization. cargo-dist 0.32.0 built the descriptor-owned `dist` profile with
one Cargo job and no publishing operation. The detached checkout was clean before archive
verification.

| Archive | Packed bytes | SHA-256 | Executable bytes |
| --- | ---: | --- | ---: |
| `merman-cli-aarch64-apple-darwin.tar.xz` | 13,734,672 | `d7870d545a19cbc97a4a937d91b263dc75fc14d74fc0f9f64a0341ad3fed3882` | 52,408,640 |
| `merman-lsp-aarch64-apple-darwin.tar.xz` | 4,078,408 | `23c16edda8c8d0fad47b927532fd7448e5fcb765d9eedab8b66c96802256432e` | 17,950,032 |

The archived CLI executable SHA-256 is
`5207373c69d8c393aa2d7f0057ad753c55de0617c5a0d5a1fbcf690f7b572a45`; the LSP executable
SHA-256 is `210ca334f7cc8b90512497b44c081bf40abc34dbc979fd8020a1cd7f2d75873b`.

# Archive verification and qualification

`verify_cli_release_archive.py --execute` passed the target, version, checksum, legal payload,
capability, completion and real SVG/PNG/JPEG/PDF archive checks. The subsequent qualification
check passed against the saved record. The CLI archive produced 18 matching observations:
Brutalist, Spotless and Cyberpunk each cover six host-dependent SVG/PNG cells across their
qualified family profiles. The shared catalog remains unqualified outside those scoped cells.

`verify_lsp_release_archive.py --execute` passed the checksum, target/version, archive layout and
framed initialize/shutdown/exit lifecycle. The LSP protocol has no version field; its version
identity is bound to the fixed source and cargo-dist manifest.

The qualification record is bound to source `bdb209e1166af960121ce0e2b3231d9929d3bca1`, the
current lockfile, Rust 1.95.0, the ARM64 host and the CLI archive SHA-256. Its SHA-256 is
`8b5cfc395b209b19c3a730cf45bfce128145b3d0b5a602b348879f3ba054b4de`.

# Preparation and evidence boundaries

The retained artifacts and logs are under
`target/bench/experiments/archive-replay-bdb-20260920/`, including the cargo-dist build log,
archives, checksums, CLI and LSP verifier output, and the qualification record. This is a local
macOS ARM64 archive witness. Linux, Windows, Intel macOS, hosted CI, registry installation,
publication-date immutable preflight and matched alpha.6 footprint attribution remain unverified.
Passing this local archive route does not close C7a.
