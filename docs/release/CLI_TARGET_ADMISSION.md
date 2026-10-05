# CLI and LSP Target Admission

This record governs evidence required to admit precompiled `merman-cli` and `merman-lsp` targets. Admission requires both native execution of the final archives and proof of the platform resource contract. A successful cross-build or a completed publication does not replace those checks. Dated records distinguish shipped artifacts from any admission evidence still outstanding.

## 0.8.0 candidate: resource probes configured, native result pending

The ARM64 `release-preflight.yml` archive job now checks the final CLI's resource paths on `ubuntu-24.04-arm`. A loopback HTTPS icon fixture must reject an untrusted certificate, then load the expected icon using a temporary CI certificate installed in Ubuntu's system trust store. No CA-path override or insecure TLS option is used. `strace` must observe successful opens of the installed DejaVu Sans font during PNG, JPEG, and PDF exports; `pdftotext` and `pdffonts` additionally verify actual PDF text and the selected font. These probes establish system-resource discovery on this runner, not pixel identity, font availability on other hosts, or an older glibc floor.

Configuration alone does not close admission. Record the successful immutable preflight run and its source identity before shipping Linux ARM64 again. The private test key remains in the ephemeral runner and is not uploaded with archive evidence.

## 2026-09-30: Linux ARM64 archives verified and published

**Admission status:** native archive execution verified; platform-resource admission remains pending. The published alpha.7 archives establish availability and the execution results below. Publication does not satisfy or waive the outstanding HTTPS/system-certificate and system-font discovery gates. Before the next release for this target, complete those checks and record the run, source identity, and supported platform scope.

`0.8.0-alpha.7` ships `aarch64-unknown-linux-gnu` CLI and LSP archives from immutable commit `580e39b69cc1b0ca35c4f8272683e622b2e9b8db`. Both the [native ARM64 preflight job](https://github.com/Latias94/merman/actions/runs/36719056708/job/109899341069) and the complete [CLI/LSP release run](https://github.com/Latias94/merman/actions/runs/36727907026) succeeded on that source. This closes the archive-execution gap recorded in September; the independently untested resource paths below remain unverified.

| Evidence | Result and scope |
| --- | --- |
| Matching native preflight | Passed on `ubuntu-24.04-arm`; the job builds and executes the final archives and uploads `preflight-cli-lsp-aarch64-unknown-linux-gnu`. |
| Final CLI archive | [Native release verification passed](https://github.com/Latias94/merman/actions/runs/36727907026/job/109952119504), including version, capabilities, completion, SVG, PNG, JPEG, PDF, and rustdoc smokes. |
| Final LSP archive | [Native release verification passed](https://github.com/Latias94/merman/actions/runs/36727907026/job/109952119433), including stdio initialize, shutdown, and exit. |
| Build and execution environment | Native Ubuntu 24.04 ARM64. These results do not establish compatibility with an older distribution or glibc floor. |
| Release closure | The five-target matrix, archive/checksum checks, immutable bundle, release verification gate, installers, and asset attestation completed in the same release run. |
| TLS and system certificates | No dedicated `network-icons` HTTPS request with `--allow-network` through the platform trust store was verified by these archive smokes. |
| System font discovery | No dedicated host-system font discovery assertion for native PNG/JPEG/PDF output was verified by these archive smokes. |

The release's `release-verification.json` binds the archive inventory to the source and version. Its ARM64 archive identities are:

| Archive | SHA-256 |
| --- | --- |
| `merman-cli-aarch64-unknown-linux-gnu.tar.xz` | `3564efe953133521a9cb5bf1000bc100ac52eef5383a4f693802cecc22798b73` |
| `merman-lsp-aarch64-unknown-linux-gnu.tar.xz` | `e57898202879a4cb9cf25ac6055eede7ed211e4d51d021fe6e1eba15bbae2b80` |

See the [publication snapshot](PUBLISH_ORDER.md#alpha7-publication-snapshot) for the full alpha.7 channel record. Windows ARM64 remains unadmitted; the Linux ARM64 release is not evidence for that target.

## 2026-09-08: Linux ARM64 admission pending native preflight

**Historical configuration-stage decision.** The pending results below describe the state recorded on 2026-09-08; the 2026-09-30 record above supplies the later archive evidence without rewriting this earlier checkpoint.

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

The following five targets have matching CLI and LSP archives in `0.8.0-alpha.7`. Availability is established by the dated release record, not merely by configuration. Platform-resource claims remain bounded by the evidence above.

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
3. The complete CLI runtime contract, including deterministic `network-icons` HTTPS checks with
   `--allow-network` through the platform trust store and host-system font discovery for native
   PNG/JPEG/PDF output. These checks do not require identical installed fonts or pixel output
   across platforms, and do not extend those resource claims to SVG, ASCII, or LSP.
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
