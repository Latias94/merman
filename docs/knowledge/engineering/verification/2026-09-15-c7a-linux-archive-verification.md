---
type: Verification Evidence
title: C7a Linux archive qualification and replay
timestamp: 2026-09-15
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 6517f6b1f6a1096340ece337f81c4b92f0495b79
tags: theme,c7a,linux,artifacts,qualification,verification
---

# Scope and result

At clean source `6517f6b1f6a1096340ece337f81c4b92f0495b79`, both final Linux x86_64
CLI/LSP archives built and passed their runtime verifiers. All 18 preset qualification
cells matched the actual CLI output; a second complete execution matched the saved record
and public catalog companion. Every qualified cell remains `host_dependent` under
`native-flowchart-state-sequence-system-fonts-v1`. No shared SDK catalog was promoted.

This is an actual Linux executable observation under macOS-hosted virtualization and
Rosetta translation, not a native GitHub Ubuntu runner result. It closes this local Linux
archive tranche, not the final same-source cross-host release preflight or C7a freeze.

# Execution environment

A task-owned Colima profile `merman-c7a` used two CPUs, 4 GiB of VM memory and a 30 GB disk.
The pre-existing stopped default profile and default Docker context were left unchanged.
The task container used Docker context `colima-merman-c7a`, platform `linux/amd64`, two CPUs
and a 3500 MiB memory ceiling. Its Ubuntu 24.04 image was pinned to digest
`sha256:224a1869083a311ef3f13648a154ba79832fbef6364d31493642ca03082da254`.
The guest reported Linux `6.8.0-117-generic`, x86_64. This is container/VM provenance, not a
claim of physical Intel hardware.

The independent checkout `/work/source` was cloned from the read-only host repository and
detached at the exact source above. It remained clean before and after qualification,
including untracked-file checks. Linux Cargo output stayed in `/work/target`; distribution
archives stayed in `/work/source/target/distrib`. Host macOS build output was not reused.

Tools were Rust 1.95.0 (`59807616e`, LLVM 22.1.2), cargo-dist 0.32.0, GCC/G++ 13,
glibc `2.39-0ubuntu8.9`, fontconfig `2.15.0-1.1ubuntu2`, DejaVu `2.37-8` and Liberation
`1:2.1.5-3`. The cargo-dist download passed its published SHA-256 check. The task VM and
container used the allowed local proxy for dependency downloads without disabling TLS.
The original VM DNS path could not resolve registry names; this task's Docker daemon used
the proxy and its containers used an explicit DNS server. No repository network settings
or host-wide Docker configuration changed.

Archive Cargo compilation used one job. The existing qualification owner explicitly uses
two jobs; peak observed container memory during that compilation was about 2.4 GiB.
No other Cargo build ran concurrently with qualification.

# Executed owners

- `python3 scripts/release-version.py check --version 0.8.0-alpha.6`: all package/version projections passed.
- `python3 scripts/cli_installation_contract.py`: declared archive installation mappings passed.
- `dist plan --tag=v0.8.0-alpha.6 --output-format=json`, followed by `release_artifact_bundle.py verify-plan` for `x86_64-unknown-linux-gnu` and the declared `ubuntu-24.04` runner: plan contract passed. That runner value checks the intended workflow contract; it does not identify this local execution host.
- `dist build --tag=v0.8.0-alpha.6 --artifacts=local --target=x86_64-unknown-linux-gnu --output-format=json`: CLI and LSP archives built; optimized compilation took about 24 and five minutes respectively.
- `verify_lsp_release_archive.py` with archive/checksum, target, version, repository and `--execute`: archive contents and real LSP protocol smoke passed.
- `verify_cli_release_archive.py` with the same boundary arguments, `--execute --preset-qualification-output target/preset-qualification.json`: archive/resource/runtime contracts and all 18 exact renderer/CLI comparisons passed.
- The same CLI owner with `--preset-qualification-check target/preset-qualification.json`: fresh renderer and CLI execution matched the complete stored record and `.catalog.json` companion.

The companion contains ten alpha preset entries. Brutalist, Spotless and Cyberpunk each
have six cells: Flowchart/State/Sequence on SVG and PNG. The other seven entries remain
unqualified. Source, recipe/resource identities, host, executable and archive digests are
bound in the existing record and companion; no new qualification format was introduced.
The system-font observations do not qualify different font installations or arbitrary
source text, and they do not become Portable admission.

# Saved artifacts

Host copies are under `/tmp/merman-c7a-6517f6b1f-linux-artifacts/`, alongside the actual
`native-dist-manifest.json`, `plan-dist-manifest.json` and private qualification record.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-x86_64-unknown-linux-gnu.tar.xz` | 15780448 | `105e525744c7ba733db27df30adb9567884adb466095666c96c70c34d1ba4b78` |
| `merman-lsp-x86_64-unknown-linux-gnu.tar.xz` | 4613580 | `60142029646629a19ed4bda61b850bc47a3def5d816d09f966e741a55fa16bc4` |
| `preset-qualification.catalog.json` | 14655 | `411ad420cea21118cda610ce82ea9b37059dded87932c2fa3bf3a0d257a51607` |

The existing release bundle owner assigns the public companion its final archive-specific
filename. These files were not uploaded, tagged or published; `0.8.0-alpha.6` identifies
the unchanged development package number and not the bytes of the published release.

Logs are `/tmp/merman-c7a-6517f6b1f-linux-{plan,dist,lsp,qualification,replay}.log`.
Environment/tool downloads are separately logged as `/tmp/merman-c7a-linux-{environment,toolchain,dist-tool,build-tools}.log`.

# Remaining scope

Linux installed Python/Node profiles, native Linux ARM64 and Windows owner execution,
Apple compiler-floor verification, and the final clean same-source candidate/rollout/freeze
remain separate gates. The actual Linux companion is now available for final bundle
verification; earlier macOS records keep their own source and host identities.
