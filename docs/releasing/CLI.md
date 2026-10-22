# Releasing and Packaging the CLI

This document is the maintainer contract for `merman-cli` binary archives, installers, and
package-manager integrations. It separates artifacts published by this repository from external
registry metadata maintained by Homebrew, Scoop, or WinGet.

## Canonical release profile

The standard CLI release is defined by the `cli-release` entry in
`capabilities/artifact-profiles-v2.json`. Its direct features must match in two places:

- `cli-release.cargo.features`;
- `crates/merman-cli/Cargo.toml` under `package.metadata.dist.features`;

Current source defaults and the official archive both include `layout-elk`, its EPL-2.0
closure, and the corresponding notice/source obligations. They differ in math support:
`cli-release` and cargo-dist select `math` explicitly, while current source defaults omit it.
Source installations can opt in with `cargo install merman-cli --features math`; see the
[math capability decision](../adr/0097-explicit-math-capability.md).
Published releases retain their version-specific feature contract. The published
`0.8.0-alpha.7` default and archives include
the Rustdoc authoring command. Custom source builds can omit ELK or Rustdoc by disabling defaults
and selecting the required individual features.

Both cargo-dist and `cli-release` use Cargo's `dist` profile and disable Cargo default features
before selecting that explicit list. Run the installation contract before changing a target,
archive format, build profile, or feature:

```bash
python3 scripts/cli_installation_contract.py
```

The check binds the Cargo manifest to `dist-workspace.toml`, the artifact profile, and the archive
layout accepted by `scripts/verify_cli_release_archive.py`.

## GNU Linux compatibility

New GNU CLI and LSP builds use native Ubuntu 22.04 and Ubuntu 22.04 ARM64 runners. The
`readelf` version-needs check in `scripts/linux_binary_compatibility.py` rejects requirements
above GLIBC 2.35, and the final archive executes on that same baseline before publication.
The VS Code runtime builds use the same baseline. This supports systems such as Ubuntu 22.04
and Debian 12 without relying on a newer build host's libc symbols.

Published archives retain their original compatibility. Both Linux CLI archives in `0.9.0`
require GLIBC 2.38; this workflow change does not replace those assets. A lower baseline takes
effect only in a subsequent release built and verified by the updated workflow. The GNU
archives do not cover musl/Alpine or NixOS; the Nix integration builds from source.

## CLI contract migration

Complete releases governed by the current source contract report capability document schema 2
and CLI contract 6. Default source builds and the official release profile include `native-fonts`
and `system-fonts`, while ordinary Mermaid SVG/PNG/JPEG/PDF output defaults to deterministic
measurement without a native font context or system discovery for layout. PNG/JPEG/PDF painting
can still load system fonts through the existing codec path. Explicit font files/directories
enable native measurement with a closed set; `--font-measurement native` without sources or
`--font-system on` explicitly requests system discovery when available. Fixed font contents
and configuration avoid catalogue discovery for faster, reproducible native output. See the [CLI font guide](../../crates/merman-cli/README.md#explicit-fonts).

Contract 6 retains `-f/--format`, text-first `lint`, narrowed `detect`, the feature-gated top-level
`rustdoc` workflow, the ASCII capability subcontract, and the Plain JSON stderr failure channel
selected by `--ascii-report`. Rust library, FFI, ASCII, and committed Rustdoc generation defaults
remain unchanged. Published tags keep their own contract and verifier. The standard
release command inventory is `batch`, `capabilities`, `completion`, `detect`, `fix`, `layout`,
`lint`, `lint-rules`, `mmdc`, `parse`, `render`, and `rustdoc`.
The archive, installation, and Homebrew verifiers require the standard profile's exact contract.
SVG-enabled artifacts also expose `theme_presets` with alpha maturity, compiler-policy availability,
and empty built-in qualification scope; archive qualification remains a separate host-scoped
companion rather than a claim from the installed binary alone.

Root invocations beginning with an `mmdc`-owned option are permanently and silently forwarded to
the explicit compatibility command while remaining absent from help and completions. Native `render` / `batch` use `-f/--format`; the former native `-e` aliases are removed in 0.9.0 as scheduled by 0.8.x. See the [migration guide](../release/V080_TO_V090_UPGRADE_GUIDE.md#fonts-and-cli-discovery). The explicit `merman-cli mmdc` command and its `-e/--outputFormat` option remain supported. Failed ASCII report-mode invocations retain exactly one schema-1 Plain JSON stderr
document and no mixed diagnostic text.

## Installation channels

| Channel | Build source | Installs support assets | Repository claim |
| --- | --- | --- | --- |
| Direct GitHub archive | cargo-dist `cli-release` binary | Yes, under `completions/` and `man/` | Published release artifact |
| cargo-dist shell or PowerShell installer | Binary extracted from the release archive | No | Published release installer |
| `cargo binstall merman-cli` | `0.8.0-alpha.5` and later: official release archive, then source fallback | No | Version-scoped manifest metadata |
| `cargo install merman-cli` | crates.io source | No | Current source defaults include ELK and Rustdoc; math requires `--features math`. Published versions retain their own defaults. |
| Nix | Repository source | Yes, in Nix integration directories | First-party source package and locked Flake |
| Homebrew | Formula source build or Homebrew bottle | Inspect the selected formula; owner validation checks required assets | External stable registry |
| Scoop | Verified Windows x86_64 archive | No | Published in Scoop's main bucket; its manifest governs the selected version |
| WinGet candidate | Verified Windows x86_64 archive | No | Generated for stable releases; external submission pending |

The shell and PowerShell installers, as well as cargo-binstall, keep only the executable as their
installed payload. They do not copy completion or man files. Cargo-dist installers may create an
environment file and update shell startup configuration to expose the installation on `PATH`;
cargo-binstall follows its own Cargo binary-directory behavior. Every complete binary can generate
completion at runtime:

```bash
merman-cli completion bash
merman-cli completion zsh
merman-cli completion fish
merman-cli completion powershell
merman-cli completion elvish
```

Users extracting an archive directly should verify its adjacent `.sha256` file first. Unix
archives use a `merman-cli-<target>/` wrapper; the Windows ZIP is flat. In both cases, the logical
payload contains the executable, package README, repository changelog and licenses,
`THIRD_PARTY_NOTICES.md`, and `THIRD_PARTY_LICENSES/`. CLI archives additionally contain
`completions/` and `man/`. The `0.9.0` standard profile ships 15 manual pages, including
Rustdoc build/check; its checked completions and manual pages match the compiled command tree.

The script installers start from the pinned cargo-dist `0.32.0` output, then pass through one
repository-owned deterministic hardening step. PowerShell binds the downloaded Windows ZIP with
`Get-FileHash`; the shell installer tries `sha256sum`, `shasum`, then OpenSSL. A checksum mismatch
or the absence of every supported SHA-256 tool stops installation. Template drift stops the release
instead of falling back to an unmodified installer.

Every release produced by this contract, beginning with `0.8.0-alpha.5`, includes
`release-verification.json`. It binds every other workflow-owned release asset to its SHA-256 digest
and size, records the source commit and release version, and maps each CLI and LSP archive to its
artifact profile and Rust target. It describes the exact payload files copied from the verified
workflow bundle; the release job does not repack them. The manifest cannot hash itself and is instead
covered by the GitHub attestation described below.

For those releases, GitHub artifact attestations cover the binary archives, adjacent checksums,
installers, checksum index, and verification manifest. GitHub's automatically generated tag source
snapshots are outside this workflow-owned bundle. After downloading a binary asset, verify its
release-workflow identity and expected tag with GitHub CLI:

```bash
gh attestation verify merman-cli-x86_64-unknown-linux-gnu.tar.xz \
  --repo Latias94/merman \
  --signer-workflow Latias94/merman/.github/workflows/release.yml \
  --source-ref "refs/tags/v<VERSION>"
```

Replace `<VERSION>` with the selected release version. The adjacent checksum detects byte corruption;
the signer and source-ref constraints bind the attestation to the repository's release workflow and
tag. Use both checks when consuming a binary outside a package manager.

## Final archive verification

Cargo-dist outputs for each target are downloaded into an isolated producer directory. Exact
producer and file inventories reject cross-target or unexpected payloads. The producer-local
manifest, archive, and adjacent checksum are checked for mutual consistency, but this check does
not establish independent provenance because cargo-dist produced all three.

The central verifier is the independent trust boundary. It binds every raw archive to its adjacent
checksum, safely validates the structure and product contract of every configured CLI and LSP
archive, and copies the accepted bytes into a verified snapshot. Global installer and checksum
generation receives only those snapshot archives. The final bundle derives its exact asset inventory
from the validated cargo-dist plan and is uploaded as one read-only `verified-release-assets`
workflow artifact. This central phase never executes target binaries.

Target-native jobs consume only that bundle: one isolated job for each product and target.
CLI jobs execute version, capability, completion, SVG, PNG, JPEG, and PDF smokes. LSP jobs execute
initialize, initialized, shutdown, and exit. Each job reports only one product, so no product's
verification result is produced after another product's binary runs in the same writable job. A
clean aggregate job requires the complete matrix to succeed and reverifies the immutable bundle
before any registry candidate, attestation, or GitHub Release job can run.

Stable releases additionally generate Scoop and WinGet candidates from the verified Windows
archive through the independent candidate workflow. Separate Windows jobs validate the Scoop JSON
and the WinGet manifest. The WinGet job also installs the manifest and smoke-tests its portable
command alias. Candidate files remain workflow artifacts for external submission; they are not
uploaded as product release assets. Prereleases take an explicit successful no-candidate path.

## cargo-binstall

`crates/merman-cli/Cargo.toml` resolves the five configured targets as follows. Stable `0.9.0`
includes both GNU Linux architectures; consult the selected release's asset inventory for availability.

| Target | Format | Executable path inside the archive |
| --- | --- | --- |
| `aarch64-apple-darwin` | `.tar.xz` | `merman-cli-aarch64-apple-darwin/merman-cli` |
| `aarch64-unknown-linux-gnu` | `.tar.xz` | `merman-cli-aarch64-unknown-linux-gnu/merman-cli` |
| `x86_64-apple-darwin` | `.tar.xz` | `merman-cli-x86_64-apple-darwin/merman-cli` |
| `x86_64-unknown-linux-gnu` | `.tar.xz` | `merman-cli-x86_64-unknown-linux-gnu/merman-cli` |
| `x86_64-pc-windows-msvc` | `.zip` | `merman-cli.exe` |

The immutable URL shape is:

```text
https://github.com/Latias94/merman/releases/download/v<VERSION>/merman-cli-<TARGET>.<FORMAT>
```

The metadata disables cargo-binstall's third-party QuickInstall strategy. If an official archive
is absent, cargo-binstall falls back to `cargo install` instead of silently substituting an
uncontrolled binary. Do not disable the `compile` strategy.

For current source, that fallback uses the math-free Cargo defaults rather than the official
archive's explicit `cli-release` selection. Use `cargo install merman-cli --features math`
when a source installation needs formula rendering. Nix and the documented Homebrew source
recipe select the release features explicitly; the container copies the verified archive
binary and therefore keeps its capabilities. The selected external Homebrew formula or
bottle remains governed by its own version-scoped validation.

Linux ARM64 uses the same generic `pkg-url`, `pkg-fmt`, and `bin-dir` templates as
the other Unix targets; only Windows keeps an override. The admission record for that target is
[`docs/release/CLI_TARGET_ADMISSION.md`](../release/CLI_TARGET_ADMISSION.md). A Homebrew ARM64 Linux
bottle belongs to a different build and verification channel.

## Additional distribution integrations

The repository prepares three integrations independently of registry publication:

- [Arch source package](../../distribution/arch/README.md): `PKGBUILD` and `.SRCINFO` for a
  checksum-pinned stable source tag, using the canonical `cli-release` recipe. Arch CI builds,
  installs and checks commands, capabilities, completions, manual pages and legal materials.
  The recipe's presence does not establish an AUR listing.
- [Setup Action](../../distribution/github-action/README.md): a dependency-free Node 24 action
  that installs an exact official release on the five supported runner platforms. It verifies
  the adjacent SHA-256 before extraction and adds the isolated installation to `PATH`. Pin its
  repository commit separately from its CLI version. It is not a Marketplace listing.
- [GNU container](../../distribution/container/README.md): an unprivileged image with CA
  certificates and DejaVu fonts, built from a verified CLI archive. PR CI builds and executes
  the x64 and ARM64 images without registry access. Manual `release-cli-container.yml` uses
  the published release's attested verification manifest, exact source and archive digest,
  tests both native images, and publishes an exact-version GHCR index. Publication and public
  package visibility are separate maintainer actions.

These channels keep the existing CLI executable name and complete feature contract. Container
images include CLI legal and shell assets but do not include the LSP. Additional fonts can be
mounted for native text measurement and raster/PDF output.

## Nix source package

The repository provides a reusable `nix/package.nix` derivation, a `default.nix` adapter, and a
thin locked Flake. All three build from source. They do not claim that the precompiled
`x86_64-unknown-linux-gnu` archive is compatible with NixOS.

The Flake is the reproducible user entry point:

```bash
nix build .
nix run . -- --version
nix profile install .
```

Consumers that own their Nixpkgs revision can call the derivation without Flakes:

```bash
nix-build --no-out-link default.nix
```

The derivation reads the `cli-release` Cargo profile directly from
`capabilities/artifact-profiles-v2.json`; it does not copy the feature list into Nix. Its install
check executes the built binary and verifies the exact command, capability, output, completion,
and man-page contracts. Bash, Zsh, and Fish completions use their conventional discovery paths.
PowerShell and Elvish snapshots are installed under `share/pwsh` and `share/elvish`; users may need
to load them explicitly depending on their shell configuration. Licenses and third-party notices
are installed under `share/doc/merman-cli`.

The source filter is intentionally independent of Git state so `default.nix` remains reusable. It
admits the workspace members declared by `Cargo.toml`, generated capability authority, legal
materials, and the single installed-surface verifier. It rejects nested build output, package
output, dependency directories, repository references, and unrelated project trees. The Python
contract test keeps that filtered source below its explicit size budget. Nixpkgs still vendors the
repository's complete workspace `Cargo.lock`; the exact `cli-release` feature selection controls
what is compiled, but this channel does not claim a minimal crate-download set.

Run the local static checks with:

```bash
nixfmt --check flake.nix default.nix nix/package.nix
python3 scripts/test_nix_package.py
```

CI injects the Flake's locked Nixpkgs into `default.nix`, evaluates all four declared systems, and
native-builds and runs the package on x86_64 Linux. The other system outputs remain source package
interfaces until they gain native Nix build jobs. A Nix source interface for Linux ARM64 is separate
from the precompiled Linux ARM64 release target; the archive channel is governed by its own
admission record and native execution gate.

## Scoop and WinGet draft candidate contract

The repository generates draft candidate files for a stable release; candidate validation does not
publish them to a central registry. Scoop's main bucket already provides `merman-cli`, and its
automated updates can advance the manifest before a manual PR is reviewed. Check the current
upstream manifest and open PRs before submitting an update. WinGet availability remains subject
to acceptance of its external submission. Add installation instructions only after checking the
corresponding registry; the [dated release record](../release/PUBLISH_ORDER.md#090-publication-snapshot)
records verified versions and pending submissions.

The generator consumes the exact Windows x86_64 archive and adjacent checksum from the verified
release bundle. It runs the release-archive verifier again, requires the requested version to
match the tagged workspace manifest, rejects prereleases and build metadata, and derives only an
immutable `releases/download/v<VERSION>/...` installer URL. Run it independently with:

```bash
python3 scripts/generate_cli_registry_candidates.py \
  <VERIFIED_RELEASE_BUNDLE> \
  --version <STABLE_VERSION> \
  --output-dir <NEW_OUTPUT_DIRECTORY>
```

The output contains:

- `scoop/merman-cli.json`, limited to the published Windows x86_64 archive;
- a three-file WinGet manifest under `winget/manifests/l/Latias94/MermanCLI/<VERSION>/`.

The WinGet candidate models the cargo-dist ZIP as a nested portable installer and currently
declares the x64 Microsoft Visual C++ runtime package dependency used by the MSVC target. Its
installer hash is uppercase as expected by WinGet; the Scoop hash is lowercase. The templates under
`distribution/cli/registry-templates/` are one Scoop JSON template and three directly reviewable WinGet YAML
templates. The generator replaces only the validated version, repository URLs, and archive digest;
it does not implement a second WinGet schema validator.

Run the repository checks with:

```bash
python3 -m unittest scripts.test_generate_cli_registry_candidates
```

After a stable GitHub Release is published, run the independent candidate workflow:

```bash
gh workflow run release-cli-registries.yml --ref main -f release_tag=v0.9.0
```

The workflow requires an existing non-draft stable release. It resolves the tag to a commit, verifies the release receipt's GitHub attestation, and checks every CLI/LSP bundle file against that receipt before generating candidates. The generator compares the Windows archive with the tagged source rather than the current branch. No binaries are rebuilt and no release assets are replaced.

WinGet and Scoop validate in separate jobs. WinGet uses the explicit `1.29.380` client and the version directory (`winget/manifests/l/Latias94/MermanCLI/<VERSION>/`); Scoop uses the official schema from Scoop `v0.6.0`. After `winget validate`, the WinGet job enables local manifests on its disposable hosted runner and installs the generated manifest with user scope and normal dependency/hash verification. It executes the installed `Microsoft/WinGet/Links/merman-cli.exe` alias directly, checking the exact release version, explicit help, no-argument usage, and a minimal SVG render. No-argument invocation must display usage and exit `2`; this is a CLI usage result, not a Windows file-not-found error. The version, help, and render probes must exit `0`.

Only after these checks pass does the job upload `winget-validated-<VERSION>` with the release receipt. Scoop uploads its separately schema-validated `scoop-validated-<VERSION>` artifact. The shared `cli-registry-candidates-<VERSION>` artifact is generated input, not proof that either validator or installation test passed. Retry only failed jobs when a validator or runner fails. These checks do not establish upstream acceptance or catalog availability.

This read-only workflow is manually dispatched so package-manager review can proceed independently of binary publication. It does not rely on a downstream `release` event from the tag publisher, and it does not submit upstream PRs or claim registry availability. Submit the validated manifests through the external repository's review process separately. A candidate-validation failure must not block or recreate the GitHub Release.

## Homebrew formula contract

The Homebrew Core formula is maintained outside this repository. Merman's scheduled
`homebrew.yml` workflow verifies the published formula on macOS and Linux, but it does not edit or
submit Homebrew metadata. It checks out the exact `v<FORMULA_VERSION>` source tag and uses that
release's verifier and artifact profile, so a later `main` branch cannot redefine an older stable
Formula's command or capability contract. Starting at `0.8.0`, a release tag without its verifier
is invalid and cannot fall back to the current branch's implementation. The installed binary's
capability schema and digest must also match that tag's declared capability authority.

Formula versions below `0.8.0` retain the legacy binary-only contract. For `0.8.0` and later,
verify the CLI contract declared by the selected release tag, match that tag's complete
`cli-release` capability set, install four Homebrew completion files, and install all 15
standard-profile man pages. Current-source CLI contract 6 does not redefine historical tags'
contract 5; a new release advances the contract through its own tag-owned verifier. `SUPPORT_ASSETS_SINCE` in
`homebrew.yml` is the single operational threshold. Invalid or prerelease formula versions fail
rather than falling back to the weaker check.

Use this shape when preparing the upstream Formula change for `0.9.0`:

```ruby
def install
  features = %w[
    all-diagrams analysis ascii icons jpeg layout-cytoscape layout-elk markdown math
    native-fonts network-icons parallel-markdown pdf png rustdoc shell-completions svg
    system-clock system-fonts system-random system-timezone system-timing
  ]

  args = std_cargo_args(path: "crates/merman-cli", features:)
  system "cargo", "install", "--no-default-features", *args

  assets = buildpath/"crates/merman-cli/assets"
  bash_completion.install assets/"completions/merman-cli.bash" => "merman-cli"
  zsh_completion.install assets/"completions/_merman-cli"
  fish_completion.install assets/"completions/merman-cli.fish"
  pwsh_completion.install assets/"completions/merman-cli.ps1" => "_merman-cli.ps1"
  man1.install Dir[assets/"man/*.1"]
end
```

This copies the release-checked snapshots instead of creating a second generator in the Formula.
Homebrew has no standard Elvish completion helper, so Elvish users keep using runtime generation.
Installing `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES/` under `pkgshare` is also recommended
to preserve the repository's legal-material policy in bottles.

After editing the Formula in a Homebrew Core checkout, run the upstream-required checks, including
a source build, formula test, strict audit, style check, and linkage check:

```bash
HOMEBREW_NO_INSTALL_FROM_API=1 brew install --build-from-source merman-cli
brew test merman-cli
brew audit --strict --online merman-cli
brew style --formula merman-cli
brew linkage --test merman-cli
```

The Merman-side verification is:

```bash
gh workflow run homebrew.yml -f expected_version=<VERSION>
```

The workflow compares every installed completion file with the installed binary's runtime output,
checks the exact man-page inventory, verifies the complete capability sets, renders SVG, runs
`brew linkage --test`, and runs the Formula test. Autobump can update a version and checksum, but it
cannot add the support-asset installation stanza; the `0.8.0` transition therefore requires an
explicit Homebrew Formula change.

## Upstream references

- [cargo-binstall package metadata](https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md)
- [cargo-dist archive layout](https://axodotdev.github.io/cargo-dist/book/artifacts/archives.html)
- [cargo-dist shell installer behavior](https://axodotdev.github.io/cargo-dist/book/installers/shell.html)
- [Homebrew Formula API](https://docs.brew.sh/rubydoc/Formula.html)
- [Opening a Homebrew pull request](https://docs.brew.sh/How-To-Open-a-Homebrew-Pull-Request)
- [Scoop app manifests](https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests)
- [WinGet multi-file manifests](https://learn.microsoft.com/en-us/windows/package-manager/package/manifest)
- [Submitting a package to WinGet](https://learn.microsoft.com/en-us/windows/package-manager/package/repository)
- [Nix source filtering](https://nixos.org/manual/nixpkgs/stable/#sec-pkgs-lib-sources)
- [Nix Flakes](https://nix.dev/concepts/flakes.html)
