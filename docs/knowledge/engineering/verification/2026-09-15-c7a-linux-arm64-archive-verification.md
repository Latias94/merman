---
type: Verification Evidence
title: C7a Linux ARM64 archive verification
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 7e9c613fa85b389a3efbe84e27b876bd0779d909
tags: theme,c7a,linux,arm64,artifacts,verification
---

# Scope and result

At clean source `7e9c613fa85b389a3efbe84e27b876bd0779d909`, the Linux ARM64 CLI and LSP
release-profile archives built and passed their existing archive verifiers with `--execute`.
The checkouts were clean before and after execution. This supplies an actual ARM64 runtime
observation, not a cross-compile-only result or a GitHub-hosted preflight result.

The workspace still uses development version `0.8.0-alpha.6`. These archive bytes are not the
published alpha.6 release. No next version/date was selected, no artifact was published, and
C7a contract freeze remains open. The ARM64 workflow does not run preset qualification:
no cells are promoted or inherited from the Linux x86_64 HostDependent qualification record.

# Environment and execution

The task-owned `merman-c7a` Colima profile hosted an Ubuntu 24.04 `linux/arm64` container,
`merman-c7a-arm64-builder`, with two CPUs and a 7 GiB container memory ceiling inside the
existing 8 GiB VM. The Ubuntu image was selected by manifest digest
`sha256:224a1869083a311ef3f13648a154ba79832fbef6364d31493642ca03082da254`.
The guest reported `aarch64`, Linux `6.8.0-117-generic`, glibc `2.39-0ubuntu8.9` and
GCC `13.3.0-6ubuntu2~24.04.1`. This is macOS-hosted virtualization, not physical CI-runner
provenance. The user default Docker context and profile were unchanged.

The source was cloned into `/work/source` from a read-only host mount, then detached at the
exact source SHA. Cargo reused `/work/target` inside that container and ran with one build job;
no other Cargo task ran concurrently. Rust was `1.95.0` (`59807616e`, LLVM 22.1.2), with native
host `aarch64-unknown-linux-gnu`. cargo-dist was `0.32.0`; its ARM64 archive matched the
release sidecar SHA-256 `d29bcffeb3f8b0c517b4ce0dd2470926ed5cb0bb29d78c6bdd5f88d76ee14a6a`.
The observed container cgroup memory peak was 3,772,321,792 bytes across setup and execution;
this is not a renderer memory benchmark.

The existing release owners ran in this order:

1. `release-version.py check --version 0.8.0-alpha.6` and `cli_installation_contract.py` passed.
2. `dist plan --tag=v0.8.0-alpha.6 --output-format=json` and
   `release_artifact_bundle.py verify-plan` passed for `aarch64-unknown-linux-gnu` and the declared
   `ubuntu-24.04-arm` runner mapping. That argument checks workflow configuration, not actual host identity.
3. `dist build --tag=v0.8.0-alpha.6 --artifacts=local --target=aarch64-unknown-linux-gnu
   --output-format=json` built the declared CLI and LSP profiles. CLI compilation took 8m47s;
   LSP compilation took 2m06s. These cold-build observations are not comparative benchmarks.
4. `verify_cli_release_archive.py` and `verify_lsp_release_archive.py` passed with each archive,
   checksum sidecar, target, version, repository root, and `--execute`.

# Artifacts

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-aarch64-unknown-linux-gnu.tar.xz` | 14234412 | `6eb54c7a600b4a4c212e92082082541bebd9b3d914ed25a6d08679f8a21fbe39` |
| `merman-lsp-aarch64-unknown-linux-gnu.tar.xz` | 4235600 | `feae5bcf94134500fe6f98c6929ad3246a497697e5de7db8dd5a03359d98724b` |

Host copies, checksum sidecars, plan/build manifests and the local observation JSON are in
`/tmp/merman-c7a-7e9c613fa-arm64-artifacts/`. Both host archive copies matched the observed
sizes and SHA-256 digests. The complete owner log is
`/tmp/merman-c7a-7e9c613fa-arm64-archives.log`; setup logs are
`/tmp/merman-c7a-arm64-{image,apt-update,build-tools,prepare}.log`.

This closes the local Linux ARM64 archive observation gap. Windows execution, the Apple compiler
floor, final same-source cross-platform rollout, selected release version/date and immutable
preflight remain separate obligations. Shared SDK qualification cells remain unchanged.
