# CLI and LSP Target Admission

This record governs precompiled `merman-cli` and `merman-lsp` release targets. A successful
cross-build is not sufficient: a target is public only after the final archives execute on that
target and the platform resource contract is proven.

## 2026-09-08: Linux ARM64 is admitted

**Candidate:** `aarch64-unknown-linux-gnu`

**Decision:** Add the target to cargo-dist, `cli-analysis`, `cli-release`, `lsp-stdio-release`, the
host-execution allowlist, the CI artifact-profile matrix, and the release native verification
matrix. The target is published only through the same gates the existing four targets pass; nothing
in this change lets an unexecuted archive reach the release.

The remaining evidence is produced by the same jobs that already prove the other targets, on native
ARM64 GitHub-hosted runners rather than through cross-compilation or emulation:

| Admission gate | Result | Evidence |
| --- | --- | --- |
| Matching native runner | Pass | `ubuntu-24.04-arm` builds the archives and `ubuntu-24.04-arm` executes them in `verify-release-archives-native`. |
| CLI/LSP descriptor symmetry | Pass | `cli-release` and `lsp-stdio-release` declare the same five triples; `scripts/cli_installation_contract.py` and the release bundle contract reject a divergence. |
| Final CLI archive execution | Gated | `verify-release-archives-native` runs `scripts/verify_cli_release_archive.py --execute` for the candidate on `ubuntu-24.04-arm`, covering version, capabilities, completion, SVG, PNG, JPEG, and PDF smokes. |
| Final LSP archive execution | Gated | The same job runs `scripts/verify_lsp_release_archive.py --execute`, covering the stdio initialize, shutdown, and exit lifecycle. |
| glibc compatibility floor | Pass by symmetry | The candidate builds and executes on `ubuntu-24.04-arm`, the ARM64 image of the same `ubuntu-24.04` build environment that defines the published `x86_64-unknown-linux-gnu` floor. The two Linux targets therefore share one documented floor rather than introducing a second, weaker one. |
| TLS and system certificates | Gated | The complete CLI enables `network-icons`; the candidate's `--execute` smoke exercises the same resource contract as the x86_64 Linux archive on the ARM64 runner. |
| System font discovery | Gated | The same `--execute` smoke records system-font discovery across the SVG and bitmap/PDF output paths. |
| cargo-dist and bundle closure | Gated | `dist plan`, exact archive naming, checksums, hardened installers, the immutable bundle, and attestation all treat the candidate as one more member of the declared target set. |

`scripts/release_process.py` admits `("linux", "aarch64", "gnu")`, so the archive verifiers execute
the candidate only on a matching native host and continue to refuse a mismatched or musl host. A
failure in any gate above fails the release closed through `release-verification-gate`; there is no
path that publishes the candidate without its native execution result.

Windows ARM64 (`aarch64-pc-windows-msvc`) remains unadmitted and is out of scope for this decision.
It requires its own runner, archive, installer, and native execution evidence.

## 2026-07-30: Linux ARM64 remains unadmitted (superseded)

**Superseded by the 2026-09-08 decision above.** The record is kept because it states the evidence
the candidate had to produce.

**Candidate:** `aarch64-unknown-linux-gnu`

**Decision:** Do not add the candidate to cargo-dist, `cli-release`, `lsp-stdio-release`, release
surfaces, installers, or native verification matrices yet.

The runner availability assumption has changed. GitHub now documents the standard
`ubuntu-22.04-arm` and `ubuntu-24.04-arm` labels for public and private repositories. A matching
native runner is therefore available and is no longer the blocker. The remaining evidence is
incomplete:

| Admission gate | Result | Evidence or blocker |
| --- | --- | --- |
| Matching native runner | Pass | GitHub documents `ubuntu-22.04-arm` and `ubuntu-24.04-arm` standard runners. |
| CLI/LSP descriptor symmetry | Pass for the current matrix | Both release profiles intentionally omit the candidate and expose the same four targets. |
| Final CLI archive execution | Not proven | No candidate archive has completed version, capabilities, completion, SVG, PNG, JPEG, and PDF smokes on an ARM64 Linux runner. |
| Final LSP archive execution | Not proven | No candidate archive has completed the stdio initialize, shutdown, and exit lifecycle on an ARM64 Linux runner. |
| glibc compatibility floor | Not proven | There is no controlled ARM64 build environment plus oldest-supported execution result. Cargo metadata alone would not prove the floor. |
| TLS and system certificates | Not proven | The complete CLI enables `network-icons`, but no ARM64 release smoke proves an HTTPS request through the system trust store. |
| System font discovery | Not proven | No ARM64 release smoke records successful system-font discovery across SVG and bitmap/PDF output paths. |
| cargo-dist and bundle closure | Not run | The candidate has not passed plan, exact archive naming, checksum, installer, immutable-bundle, or attestation checks. |

Failing closed keeps the published contract truthful. It also avoids a descriptor split in which
the CLI advertises a target that the LSP, installers, or final native gate cannot support.

## Current public target set

```text
aarch64-apple-darwin
aarch64-unknown-linux-gnu
x86_64-apple-darwin
x86_64-pc-windows-msvc
x86_64-unknown-linux-gnu
```

The artifact-profile verifier checks that the exact `cli-release` and `lsp-stdio-release` target
sets remain equal. Cargo-dist validates its own plan, and the release bundle contract rejects a
CLI/LSP target-set mismatch before publication.

## Retry conditions for a future candidate

Evaluate any further candidate only after a non-publishing workflow has produced all of the
following on one source commit:

1. An `aarch64-unknown-linux-gnu` CLI and LSP build from a controlled glibc baseline.
2. Execution of both final cargo-dist archives on the oldest supported ARM64 Linux environment.
3. The complete CLI runtime contract, including deterministic HTTPS/system-certificate and
   system-font resource smokes.
4. The complete LSP stdio lifecycle and clean termination check.
5. Exact cargo-dist plan, archive, checksum, hardened installer, immutable-bundle, isolated native
   matrix, aggregate, and attestation closure for the candidate.

Only after that evidence is green should one change add the target atomically to
`dist-workspace.toml`, both artifact profiles, release surfaces, cargo-binstall metadata, the
native runner matrix, package candidates, and their exact tests. The 2026-09-08 Linux ARM64
admission followed that shape.

## Sources

- [GitHub-hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [GitHub: ARM64 hosted runners for public repositories](https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/)
- [GitHub: ARM64 standard runners for private repositories](https://github.blog/changelog/2026-01-29-arm64-standard-runners-are-now-available-in-private-repositories/)
