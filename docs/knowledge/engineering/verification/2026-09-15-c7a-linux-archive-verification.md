---
type: Verification Evidence
title: C7a Linux archives and installed Python wheel verification
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

# Installed Python wheel follow-up

The same clean source built the exact `python-uniffi-native` recipe for
`x86_64-unknown-linux-gnu`, using the `native-distribution` profile and
`analysis,ascii,layout-cytoscape,layout-elk,svg` features. The initial attempts recorded cgroup
OOM kills; increasing only the container ceiling could not overcome the 4 GiB VM limit.
With this task VM increased to 8 GiB and the container to 7 GiB, the build passed using one
Cargo job. The observed cgroup peak was 5082787840 bytes. Native compilation took 4m18s;
the separately compiled binding generator then produced the Python projection without
changing tracked scaffold files. These are build-environment observations, not a regression
benchmark or a reason to alter the artifact recipe.

`scripts/build-python-uniffi-wheel.py --run-smoke` built and installed the original Linux
wheel successfully. Auditwheel 6.8.2 repaired it into
`merman-0.8.0a6-py3-none-manylinux_2_35_x86_64.whl`; Twine 7.0.0 and the existing
target-specific wheel-license verifier both passed. A second, fresh Python 3.12 virtual
environment installed the repaired wheel with `--no-deps` and ran
`platforms/python/merman/examples/smoke.py`. The imported module came from that venv's
`site-packages`, not the source tree. The smoke passed shared authoring/error/catalog vectors,
three-family light/dark isolation, rule editing, cold-start complete specs, preset export,
22 support vectors, and all three budgeted authoring operations through one-shot and reusable
consumers. The checkout remained clean.

The repaired wheel is 10089565 bytes, SHA-256
`50d4774a9b66273add7f1aa1daef68a9d25eca2a9d14d4e8132c2473dfede7ae`.
Its bundled `merman/libmerman_uniffi.so` is 25613616 bytes, SHA-256
`c1d879149d14792728d32dc04aee6bee8dcfe8142e4dafbb5d3ae079bf2d20af`.
A host copy is under `/tmp/merman-c7a-6517f6b1f-linux-artifacts/python-wheels/`.
The manylinux tag reflects the ABI requirements inferred by auditwheel; execution was on
glibc 2.39, not a separate test on glibc 2.35. This default SVG SDK profile does not acquire
CLI PNG qualification cells.

Logs are `/tmp/merman-c7a-6517f6b1f-linux-python-wheel-8g.log` and
`/tmp/merman-c7a-6517f6b1f-linux-python-final-wheel.log`. No wheel was published.

This execution also exposed a preflight gap: the preflight owner smoked the original wheel
before Linux repair, whereas the release owner smoked the final repaired artifact. Commit
`a6b9f9ae5` moves preflight smoke after repair, using the same isolated install sequence and
removing the redundant earlier smoke. All 37 workflow contract tests, Python compilation,
actionlint on both owners and diff checks passed. A clean candidate checkout at
`a6b9f9ae57b627a92fe2940454a2ac6b6de5a022` additionally passed 67 combined workflow,
qualification-replay and release-bundle tests. Its log is
`/tmp/merman-c7a-a6b9f9ae5-release-contracts.log`. That source check does not relabel the
wheel built from `6517f6b1f` as a newer artifact or claim a GitHub preflight run.

# Remaining scope

Linux installed Node profiles, native Linux ARM64 and Windows owner execution,
Apple compiler-floor verification, and the final clean same-source candidate/rollout/freeze
remain separate gates. The actual Linux companion is now available for final bundle
verification; earlier macOS records keep their own source and host identities.

## Same-source refresh at `57ec788c0`

After the workspace and release-contract follow-ups, the Linux builder was refreshed to clean
source `57ec788c0a4b49227cf8226d4e0df43686ac6a1e`. The same cargo-dist plan, archive execution,
qualification run, and saved-record replay completed successfully. This refresh uses the existing
qualification and catalog schemas; it does not promote any catalog entry or change admission.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `merman-cli-x86_64-unknown-linux-gnu.tar.xz` | 15,820,240 | `d9aedbd54ae67047368c5c39dcd5341077c1e558677dc6b070b0b4eacbf6fcb6` |
| `merman-lsp-x86_64-unknown-linux-gnu.tar.xz` | 4,605,092 | `5fa242194c957a5eb983f969b5349b9f955e942dea74a8710666a9f014fcf90c` |
| `preset-qualification.json` | 44,180 | `e893943f0205fad54a42d74a1e32238fe3abdbd4856f2152278e14325d4d486b` |
| `preset-qualification.catalog.json` | 14,655 | `3005ef30dc446774d1f7bc2729d0160839a16bfc6373a057898620808a032e8e` |

The catalog companion binds the CLI archive SHA, executable SHA, source commit, target,
`resvg-safe` pipeline, and host-dependent system-font profile. Brutalist, Spotless, and Cyberpunk
retain six cells each (Flowchart/State/Sequence SVG and PNG); the seven retained compatibility
presets retain zero cells. The archive remains an unpublished development candidate labelled
`0.8.0-alpha.6`; it is not the immutable published alpha.6 release.

The actual archive log is `/tmp/merman-c7a-57ec788c0-linux-archives.log`; host copies are under
`/tmp/merman-c7a-57ec788c0-linux-artifacts/`. The clean builder checkout had no tracked or
untracked changes after verification.
