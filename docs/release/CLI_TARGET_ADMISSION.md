# CLI and LSP Target Admission

This record governs precompiled `merman-cli` and `merman-lsp` release targets. A successful
cross-build is not sufficient: a target is public only after the final archives execute on that
target and the platform resource contract is proven.

## 2026-07-30: Linux ARM64 remains unadmitted

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

The decision leaves every public target descriptor unchanged:

```text
aarch64-apple-darwin
x86_64-apple-darwin
x86_64-pc-windows-msvc
x86_64-unknown-linux-gnu
```

`scripts/test_release_artifact_workflow.py` checks that the cargo-dist runner matrix and the exact
`cli-release` and `lsp-stdio-release` target sets remain equal. The release bundle contract also
rejects a CLI/LSP target-set mismatch.

## Retry conditions

Re-evaluate Linux ARM64 only after a non-publishing workflow has produced all of the following on
one source commit:

1. An `aarch64-unknown-linux-gnu` CLI and LSP build from a controlled glibc baseline.
2. Execution of both final cargo-dist archives on the oldest supported ARM64 Linux environment.
3. The complete CLI runtime contract, including deterministic HTTPS/system-certificate and
   system-font resource smokes.
4. The complete LSP stdio lifecycle and clean termination check.
5. Exact cargo-dist plan, archive, checksum, hardened installer, immutable-bundle, isolated native
   matrix, aggregate, and attestation closure for the candidate.

Only after that evidence is green should one change add the target atomically to
`dist-workspace.toml`, both artifact profiles, release surfaces, cargo-binstall metadata, the
native runner matrix, package candidates, and their exact tests.

## Sources

- [GitHub-hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [GitHub: ARM64 hosted runners for public repositories](https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/)
- [GitHub: ARM64 standard runners for private repositories](https://github.blog/changelog/2026-01-29-arm64-standard-runners-are-now-available-in-private-repositories/)
