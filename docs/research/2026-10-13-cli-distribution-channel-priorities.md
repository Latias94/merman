---
title: "CLI Distribution Priorities and Published Binary Coverage"
type: "research"
date: "2026-10-13"
status: "research complete; no publication or implementation performed"
---

# CLI Distribution Priorities and Published Binary Coverage

## Recommendation

Merman already has a useful distribution foundation: Cargo source installation, cargo-binstall,
versioned GitHub archives and installers, Homebrew, Scoop, and Conda-forge. WinGet and Nixpkgs
remain pending external acceptance in the inspected release record. The next useful additions are
Arch AUR, a GNU-based OCI image on GHCR, and an installation action for GitHub Actions. An npm CLI
package is worthwhile if JavaScript documentation projects are a target audience. These rankings
are engineering judgments about fit and maintenance, not measured adoption forecasts.

Before expanding Linux packaging broadly, lower and test the GNU binary compatibility baseline.
Both published Linux CLI binaries require the non-weak `GLIBC_2.38` version. Repackaging those
binaries as `.deb` or `.rpm` would not make them work with older system glibc versions.

The maintainer reported about 595 GitHub stars. That is useful evidence of interest, but it does
not establish package adoption or acceptance. Most inspected channel rules focus on packaging,
licenses, functioning builds, and maintenance. One concrete exception is mise's current registry
guidance, which normally expects thousands of stars; its direct GitHub backend does not require
registry admission. [mise registry policy](https://mise.jdx.dev/contributing/registry.html)

## Inspection scope

- Local source inspected at `3a4b8c393939b69372a8f05a2b971f7cdf6ae529`.
- GitHub's authenticated Release API identified the latest workspace release as `v0.9.0`, published
  on `2026-10-12T13:12:02Z`. Its downloaded verification manifest identifies immutable source
  `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f` and version `0.9.0`.
- The Windows x64 archive and both Linux GNU archives were downloaded from that exact release.
  Their SHA-256 digests match the adjacent checksum files and the public Release asset digests.
- The Windows archive additionally passed `gh attestation verify` constrained to this repository,
  `.github/workflows/release.yml`, and `refs/tags/v0.9.0` (exit status 0).
- Only the downloaded Windows binary was executed locally. Linux findings are structural ELF
  analysis, not new Linux runtime tests. No new macOS, Windows ARM64, musl, or BSD support is claimed.
- Channel status comes from the maintainer and the dated local release record unless an explicit
  live query is described. The pending WinGet/Nixpkgs PRs were not independently re-audited.

Primary project evidence: [0.9.0 Release](https://github.com/Latias94/merman/releases/tag/v0.9.0),
[CLI install guide](../../crates/merman-cli/README.md), [release record](../release/PUBLISH_ORDER.md),
[target admission record](../release/CLI_TARGET_ADMISSION.md), and [cargo-dist configuration](../../dist-workspace.toml).

## Current binary coverage

The five-target archive matrix covers macOS Intel and Apple Silicon, Windows x64, and Linux x64
and ARM64 GNU. There are no CLI musl, Windows ARM64, or FreeBSD archives in this release.
The public API reports the following compressed sizes; MB means decimal megabytes.
[Release assets](https://github.com/Latias94/merman/releases/tag/v0.9.0)

| CLI target | Archive bytes | Archive MB | Executable bytes inspected |
| --- | ---: | ---: | ---: |
| `aarch64-apple-darwin` | 12,241,452 | 12.24 | Not downloaded |
| `x86_64-apple-darwin` | 13,770,200 | 13.77 | Not downloaded |
| `x86_64-pc-windows-msvc` | 19,983,667 | 19.98 | 50,822,656 |
| `aarch64-unknown-linux-gnu` | 12,883,900 | 12.88 | 51,275,136 |
| `x86_64-unknown-linux-gnu` | 14,303,040 | 14.30 | 56,892,360 |

These sizes do not by themselves justify another maintained slim CLI product. Such a product
would add capability and publication variants; consider it only with a concrete constrained-host
requirement or measured download/startup problem.

### Windows artifact observations

The actual binary reports `merman-cli 0.9.0`, CLI contract 6, capability schema 2, Mermaid 12.1.0,
and mmdc 12.0.0. It lists 34 diagram families and 12 commands. Its output inventory contains SVG,
PNG, JPEG, PDF, and ASCII; this is a compiled capability inventory, not proof of complete visual
or ASCII coverage for every family.

One minimal Flowchart successfully rendered to SVG, PNG, JPEG, PDF, and Unicode on the current
Windows 11 x64 host. XML, image signatures, PDF markers, and Unicode label presence were checked.
An explicit local Arial font file also produced valid SVG with matching `fontFamily`; native-font
information was emitted on stderr as expected. These are format and invocation smokes, not visual
comparisons or a benchmark.

The ZIP contains 96 entries, 15 man pages, five shell completion files, and legal material. Its
uncompressed payload totals 52,225,604 bytes. The existing installers and cargo-binstall install
the executable only; support files are available in the archive, and completions can be generated
at runtime. [CLI packaging contract](../releasing/CLI.md)

### Linux ABI observations

Both archives contain ELF64 dynamically linked executables. The ELF `.gnu.version_r` requirement
table was parsed, then mapped through `.gnu.version` and `.dynsym` to undefined imported symbols.
The `GLIBC_2.38` requirements have `vna_flags=0`; both binaries import `__isoc23_sscanf` and
`__isoc23_strtol` at that version. This conclusion does not rely on searching arbitrary strings.
Missing a required non-weak version prevents normal dynamic loading.
[LSB symbol versioning](https://refspecs.linuxfoundation.org/LSB_3.1.1/LSB-Core-generic/LSB-Core-generic/symversion.html)

| ELF property | Linux x64 | Linux ARM64 |
| --- | --- | --- |
| Machine | x86-64 (`e_machine=62`) | AArch64 (`e_machine=183`) |
| `PT_INTERP` | `/lib64/ld-linux-x86-64.so.2` | `/lib/ld-linux-aarch64.so.1` |
| Highest required `libc.so.6` version | `GLIBC_2.38` | `GLIBC_2.38` |
| Highest required `libm.so.6` version | `GLIBC_2.35` | `GLIBC_2.35` |
| Highest required `libgcc_s.so.1` version | `GCC_4.2.0` | `GCC_4.2.0` |
| Loader version requirement | `GLIBC_2.3` | `GLIBC_2.17` |

These four libraries are the complete `DT_NEEDED` set for each binary. This does not enumerate
resources or libraries discovered during execution. `GCC_4.2.0` names a libgcc symbol version,
not a compiler that end users must install.
[System V ABI dynamic linking](https://refspecs.linuxbase.org/elf/gabi4%2B/ch5.dynamic.html)

Consequently the published GNU artifacts cannot run unmodified against the stock glibc versions
in Debian 12 (2.36) or Ubuntu 22.04 (2.35). Debian 13's 2.41 satisfies this particular version
requirement, but that fact alone does not establish complete runtime compatibility.
[Debian 12 libc6](https://packages.debian.org/bookworm/libc6),
[Ubuntu 22.04 toolchain release notes](https://discourse.ubuntu.com/t/jammy-jellyfish-release-notes/24668),
[Debian 13 libc6](https://packages.debian.org/trixie/libc6)

The next release should choose an explicit oldest supported GNU environment and run the final
artifacts there. A glibc 2.35 build baseline is a reasonable candidate if Ubuntu 22.04 and Debian
12 are intended users; feasibility still needs a real build of the complete dependency closure.
Modern Rust compiler requirements and target-system glibc compatibility are separate constraints.

Artifact identifiers retained for repeat inspection:

| Archive | SHA-256 |
| --- | --- |
| Windows x64 | `c56f2b3b286ff11375840fcf0adfa5d3a74d768a34d05e17a1cf423007ca3431` |
| Linux x64 GNU | `73942e12b5a38b37e1a7d8b1d6d2436fc9ac1acbab7e666060c81755f270578e` |
| Linux ARM64 GNU | `c46f786106a356c7625c6c90f3d81130bbba706dd72e86dd0a8bfb40724cf58b` |

### Runtime resources and installation experience

The CLI removes Node/Chromium requirements, but a binary alone does not define a reproducible
font environment. Ordinary graphical layout defaults to deterministic measurement; PNG/JPEG/PDF
painting can still discover system fonts. Explicit font files enable native measurement, and SVG
does not embed fonts. An OCI image can provide a documented font and CA environment; source and
system packages should preserve the same documented capability and notice closure.
[CLI fonts](../../crates/merman-cli/README.md#explicit-fonts)

The current five architectures are a useful starting matrix. Additional Windows ARM64 or BSD
artifacts should follow demonstrated user demand and native execution evidence. musl has a more
direct connection to container/Alpine requirements, but needs its own builds and font/TLS tests.
Prefer a compatible GNU image first; do not treat the current archives as Alpine or scratch-ready.

## Channel priorities

| Channel | Priority and rationale | Maintenance / admission implications |
| --- | --- | --- |
| Arch AUR | First system-package addition. Serves an uncovered Linux installation workflow. | Maintain a PKGBUILD and `.SRCINFO`; source package first, `-bin` once the actual binary runs in the target environment. AUR is user-maintained, separate from Arch's official binary repositories. |
| GHCR OCI image | High priority for CI and document builds. Makes fonts and GNU runtime explicit. | Reuse verified release bytes in a compatible GNU image; maintain base updates, version tags, amd64/arm64 execution, and font conventions. Docker Hub can mirror the same image later. |
| GitHub Marketplace setup action | High priority for GitHub document/Markdown/Rustdoc workflows. | Install a pinned CLI version and expose PATH. Keep command execution with the caller. Use a separate action repository and validate downloaded assets. |
| npm CLI | Useful if JavaScript documentation users are a target. | Add a distinct executable package such as `@mermanjs/cli`; preserve the Rust CLI contract rather than recreating commands through the Node SDK. Requires platform packaging and another coordinated publisher. |
| `.deb` / `.rpm` Release assets | Useful after choosing a Linux compatibility baseline. | Provide normal install/uninstall and support assets. Standalone packages do not create apt/DNF repositories or automatic upgrades. |
| Chocolatey Community Repository | Reasonable Windows follow-up, with smaller incremental coverage after Scoop/WinGet. | Can reuse the Windows ZIP with a pinned checksum. Requires community moderation and ongoing package updates. |
| Fedora COPR / openSUSE OBS | Use when users request RPM repository updates. | Personal/community repositories on supported infrastructure; neither implies admission to the distro's default official repository. Choose one relevant service first. |
| Debian / Ubuntu official archives | Valuable long-term integration with relatively high initial cost. | Rust dependency packaging, source/license review, offline builds, sponsorship and ongoing distro maintenance. Prefer a contributor who will own the package. |
| Alpine aports | Conditional on container or Alpine demand. | New musl/native execution evidence, target-branch compiler support, and testing/community maintenance. |
| FreeBSD ports | Demand-driven, lower immediate priority. | Requires real FreeBSD build/runtime evidence and a maintainer; Linux archive availability is irrelevant. |

Channel evidence:

- AUR's official RPC returned no packages named `merman-cli`, `merman-cli-bin`, or `merman` on the
  inspection date. Precompiled packages use the `-bin` convention; source builds follow Arch's
  Rust packaging guidelines. [AUR RPC](https://aur.archlinux.org/rpc/v5/info?arg%5B%5D=merman-cli&arg%5B%5D=merman-cli-bin&arg%5B%5D=merman),
  [submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines_%28%E7%AE%80%E4%BD%93%E4%B8%AD%E6%96%87%29),
  [Rust guidelines](https://wiki.archlinux.org/title/Rust_package_guidelines).
- GHCR supports Docker/OCI and anonymous pulls for public images; publishing from Actions can
  use `GITHUB_TOKEN`. Public visibility must be configured. Mermaid's upstream CLI already
  documents npm/npx, Docker Hub, GHCR, and Podman workflows, supporting the fit of these channels
  for diagram rendering. [GHCR](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry),
  [Mermaid CLI](https://github.com/mermaid-js/mermaid-cli#alternative-installations).
- Marketplace action publication requires a public repository, root metadata, unique name,
  developer agreement, and 2FA; the documented criteria do not impose a star threshold or manual
  review. [Marketplace publication](https://docs.github.com/en/actions/how-tos/create-and-publish-actions/publish-in-github-marketplace).
- The existing `platforms/node/packages/node/package.json` has no `bin`: `@mermanjs/node` is a
  Node SDK, not an npm-distributed CLI. npm's `bin` field creates executable entry points.
  [npm bin](https://docs.npmjs.com/cli/v11/configuring-npm/package-json/#bin).
- Chocolatey supports ZIP downloads/checksums or embedded executables and applies moderation;
  update automation is maintained by the package owner. [ZIP helper](https://docs.chocolatey.org/en-us/create/functions/install-chocolateyzippackage/),
  [moderation](https://docs.chocolatey.org/en-us/community-repository/moderation/),
  [automatic packaging](https://docs.chocolatey.org/en-us/create/automatic-packages/).
- A third-party apt repository needs indexes, signing and user configuration, beyond a `.deb`
  asset. [Debian third-party repository guidance](https://wiki.debian.org/DebianRepository/UseThirdParty).
- COPR has lower contributor barriers than Fedora's official Koji workflow. OBS provides user
  home projects and configurable build targets. [COPR vs Koji](https://docs.pagure.org/copr.copr/_sources/koji_vs_copr.rst.txt),
  [OBS guide](https://openbuildservice.org/help/manuals/obs-user-guide/art-obs-bg).
- Debian's Rust team policy expects packaged crate/feature build dependencies; initial packages
  need sponsorship/review and NEW processing. Ubuntu recommends pursuing Debian first for most
  new packages. [Rust policy](https://wiki.debian.org/Teams/RustPackaging/Policy),
  [Debian new maintainers](https://mentors.debian.net/intro-maintainers/),
  [Ubuntu new packages](https://documentation.ubuntu.com/project/contributors/new-package/create-a-new-package/).
- Alpine Rust packaging separates dependency fetch from frozen builds. New packages normally
  enter edge/testing; movement to community needs a responsible maintainer. Compiler availability
  must be checked per branch against Merman's Rust 1.95 requirement, rather than assumed from edge.
  [Rust APKBUILD](https://wiki.alpinelinux.org/wiki/APKBUILD_examples%3ARust),
  [aports branches](https://wiki.alpinelinux.org/wiki/Aports_tree).
- FreeBSD supports `USES=cargo` and crate dependency generation; new ports are reviewed by a
  committer. [Cargo ports](https://docs.freebsd.org/en/books/porters-handbook/book/#using-cargo),
  [new port submission](https://docs.freebsd.org/en/books/porters-handbook/quick-porting/#porting-submitting).

## Channels to defer

Flathub currently excludes console software except related Flatpak/Flathub tooling; it is not
an appropriate submission target for this CLI. Snap supports CLI applications, but confinement
adds filesystem/font integration work and classic confinement requires manual approval; with the
existing channels, defer it unless requested. [Flathub requirements](https://docs.flathub.org/docs/for-app-authors/requirements),
[Snap confinement](https://snapcraft.io/docs/explanation/security/snap-confinement/)

mise can consume `github:owner/repo` without registry admission. First verify selecting the CLI
asset rather than the LSP and document a version-pinned install. Its shorthand registry is not
an immediate priority at the reported star count. Avoid a new asdf plugin solely to duplicate
that install path. mise's ubi backend is deprecated in favor of its GitHub backend; this does
not mean the standalone ubi installer is deprecated. [GitHub backend](https://mise.jdx.dev/dev-tools/backends/github.html),
[registry policy](https://mise.jdx.dev/contributing/registry.html)

## Suggested order of work

1. Document the observed GNU symbol requirement and select/test an older build baseline for the
   next release. Do not replace accepted 0.9.0 assets.
2. Prepare AUR packaging and a GNU OCI image, reusing the existing release artifact ownership.
3. Add a small setup action; assess npm CLI demand from documentation-tooling users.
4. Add `.deb`/`.rpm` assets with explicit supported systems, then consider repository hosting.
5. Add Chocolatey or a maintained distro package when the incremental audience justifies it.

A small documentation cleanup is also warranted: the inspected CLI README's workflow table still
mentions `mmdc@11.16.0`, while the actual release capability document reports 12.0.0; the
cargo-binstall section of `docs/releasing/CLI.md` retains an ARM64-pending sentence superseded by
the target admission record. These observations are not binary failures.

## Research limitations

No adoption telemetry was available, so platform coverage cannot be converted into a user
percentage. No new packages, manifests, workflows, registry submissions, or external comments
were created. The only repository change is this note; downloaded artifacts remain in isolated
system temporary directories. Unrelated existing untracked files were preserved.

The unauthenticated GitHub API was rate-limited; authenticated `gh` reads succeeded. Some official
pages timed out or exposed access-protection pages; relevant official document sources, package
indexes, or release notes were used instead. An unresponsive batched search was terminated rather
than repeatedly retried. No access protection was bypassed and no credentials were recorded.
