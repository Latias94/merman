# CLI and LSP Target Admission

This record governs precompiled `merman-cli` and `merman-lsp` release targets. A successful
cross-build is not sufficient: a target is public only after the final archives execute on that
target and the platform resource contract is proven.

## 2026-09-08: Linux ARM64 admission pending native preflight

**Candidate:** `aarch64-unknown-linux-gnu`

**Decision:** Configure the candidate atomically in cargo-dist, `cli-analysis`, `cli-release`,
`lsp-stdio-release`, the host-execution allowlist, and the CI, preflight, and release native
matrices. Configuration is not successful admission evidence. Before merging the candidate, run
the non-publishing `Release Preflight` workflow against its exact source commit and record the
native archive results below. Publication still requires the complete release verification gate.

The `cli-and-lsp-archives` preflight matrix builds and executes the final cargo-dist archives on
`ubuntu-24.04` and `ubuntu-24.04-arm`. It runs the full non-publishing `dist plan` first and checks
the candidate's archive inventory, runner, and host before building. It does not require creating
or pushing a release tag, and does not publish or attest anything.

No successful ARM64 preflight run is recorded here yet. A workflow definition or a passing
Python contract test must not be reported as a native execution result.

| Admission gate | Result | Evidence |
| --- | --- | --- |
| Matching native runner | Configured | The preflight ARM64 job uses `ubuntu-24.04-arm` and rejects a cargo-dist plan with a different runner, host, or container. |
| CLI/LSP descriptor symmetry | Pass | `cli-release` and `lsp-stdio-release` declare the same five triples; `scripts/cli_installation_contract.py` and the release bundle contract reject a divergence. |
| Final CLI archive execution | Pending run | Preflight runs `scripts/verify_cli_release_archive.py --execute`, covering version, capabilities, completion, SVG, PNG, JPEG, PDF, and rustdoc smokes. |
| Final LSP archive execution | Pending run | Preflight runs `scripts/verify_lsp_release_archive.py --execute`, covering the stdio initialize, shutdown, and exit lifecycle. |
| glibc compatibility floor | Pending run | Build and execution use the native Ubuntu 24.04 image for each Linux architecture; a successful ARM64 result is still required. No older distribution is claimed. |
| TLS and system certificates | Not proven by archive smokes | `network-icons` is enabled, but the current archive verifier does not make an HTTPS request through the system trust store. Separate runtime evidence is required. |
| System font discovery | Not proven independently | Archive smokes check capability metadata and render outputs, not a dedicated system-font discovery assertion. Separate resource evidence is required. |
| cargo-dist plan and local archive checksums | Pending run | Preflight validates the full planned asset contract and native routing, then verifies both final archives and adjacent checksums. |
| Global installers, immutable bundle, and attestation | Release-only gates | These require the complete release matrix and remain owned by `release.yml`; the local-archive preflight is not evidence that they have run. |

`scripts/release_process.py` admits `("linux", "aarch64", "gnu")`, so the archive verifiers execute
the candidate only on a matching native host and continue to refuse a mismatched or musl host. A
failure in the release archive matrix fails the release closed through `release-verification-gate`;
there is no path that publishes the candidate without its native execution result. That final
gate does not replace the pre-merge admission evidence.

For a completed preflight, record the run URL, exact source SHA, version, target, runner, and the
`preflight-cli-lsp-<target>` artifact. Keep the successful job logs with the archived plan,
build manifest, final archives, and checksums. Resource claims need their own evidence; do not
mark them passed merely because the archive jobs are green.

Windows ARM64 (`aarch64-pc-windows-msvc`) remains unadmitted and is out of scope for this decision.
It requires its own runner, archive, installer, and native execution evidence.

## 2026-07-30: Linux ARM64 remains unadmitted (historical)

**Historical baseline for the 2026-09-08 candidate above.** The record is kept because it states
the evidence gaps; adding the candidate's configuration does not itself close them.

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

## Configured release target set

The configuration below includes the pending Linux ARM64 candidate. It is not a claim that an
existing release contains that archive or that the candidate's admission evidence is complete.

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

Prepare each candidate as one focused change to cargo-dist, the artifact profiles, installation
metadata, native runner matrices, package candidates where applicable, and their exact tests.
Keep its admission status pending until a non-publishing workflow has produced all of the
following on one source commit, before merging that change:

1. CLI and LSP builds for the candidate target triple from a controlled platform and ABI baseline.
2. Execution of both final cargo-dist archives on the candidate's oldest supported native environment.
3. The complete CLI runtime contract, including deterministic HTTPS/system-certificate and
   system-font resource smokes.
4. The complete LSP stdio lifecycle and clean termination check.
5. The exact cargo-dist plan, candidate runner/host routing, archive inventory, and adjacent checksums.

Record the successful run and source identity before marking the candidate admitted. On release,
the full target matrix must still pass hardened installer generation, immutable-bundle assembly,
isolated native execution, aggregation, and attestation. These release-only gates are not replaced
by a candidate-local preflight, and a pending candidate must not be described as having passed them.

## Sources

- [GitHub-hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [GitHub: ARM64 hosted runners for public repositories](https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/)
- [GitHub: ARM64 standard runners for private repositories](https://github.blog/changelog/2026-01-29-arm64-standard-runners-are-now-available-in-private-repositories/)
