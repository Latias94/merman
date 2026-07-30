# Releasing and Packaging the CLI

This document is the maintainer contract for `merman-cli` binary archives, installers, and
package-manager integrations. It separates artifacts published by this repository from external
registry metadata maintained by Homebrew, Scoop, or WinGet.

## Canonical release profile

The complete CLI is defined by the `cli-release` entry in
`capabilities/artifact-profiles-v1.json`. The same 18 direct features must appear in three places:

- `cli-release.cargo.features`;
- `crates/merman-cli/Cargo.toml` under `package.metadata.dist.features`;
- the CLI's default feature list, which keeps `cargo install merman-cli` complete and predictable.

Both cargo-dist and `cli-release` use Cargo's `dist` profile and disable Cargo default features
before selecting that explicit list. Run the installation contract before changing a target,
archive format, build profile, or feature:

```bash
python3 scripts/cli_installation_contract.py
```

The check binds the Cargo manifest to `dist-workspace.toml`, the artifact profile, and the archive
layout accepted by `scripts/verify_cli_release_archive.py`.

## CLI contract migration

Complete releases governed by this contract, beginning with `0.8.0-alpha.4`, report capability
document schema 2 and CLI contract 3.
Contract 3 advertises `-f/--format` for native `render` and `batch`, makes `lint` text-first while
keeping explicit JSON stable, and removes no-op configuration and rendering controls from `detect`.
The archive, installation, and Homebrew verifiers require that exact contract.

Root invocations beginning with an `mmdc`-owned option are permanently and silently forwarded to
the explicit compatibility command while remaining absent from help and completions. The separate
native `render -e` / `batch -e` migration aliases map to `-f`, retain their bounded warning even
when quiet, and are removed in `v0.9.0`. The explicit `merman-cli mmdc` command and its
`-e/--outputFormat` option remain supported.

## Installation channels

| Channel | Build source | Installs support assets | Repository claim |
| --- | --- | --- | --- |
| Direct GitHub archive | cargo-dist `cli-release` binary | Yes, under `completions/` and `man/` | Published release artifact |
| cargo-dist shell or PowerShell installer | Binary extracted from the release archive | No | Published release installer |
| `cargo binstall merman-cli` | `0.8.0-alpha.4` and later: official release archive, then source fallback | No | Version-scoped manifest metadata |
| `cargo install merman-cli` | crates.io source | No | Complete defaults; custom features supported |
| Nix | Repository source | Yes, in Nix integration directories | First-party source package and locked Flake |
| Homebrew | Formula source build or Homebrew bottle | Formula `0.8.0+` installs assets | External stable registry |
| Scoop candidate | Verified Windows x86_64 archive | No | Generated for stable releases; external submission pending |
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
`completions/` and `man/`.

The script installers start from the pinned cargo-dist `0.32.0` output, then pass through one
repository-owned deterministic hardening step. PowerShell binds the downloaded Windows ZIP with
`Get-FileHash`; the shell installer tries `sha256sum`, `shasum`, then OpenSSL. A checksum mismatch
or the absence of every supported SHA-256 tool stops installation. Template drift stops the release
instead of falling back to an unmodified installer.

Every release produced by this contract, beginning with `0.8.0-alpha.4`, includes
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
checksum, safely validates the structure and product contract of all four CLI and all four LSP
archives, and copies the accepted bytes into a verified snapshot. Global installer and checksum
generation receives only those snapshot archives. The final bundle derives its exact asset inventory
from the validated cargo-dist plan and is uploaded as one read-only `verified-release-assets`
workflow artifact. This central phase never executes target binaries.

Eight target-native jobs consume only that bundle: one isolated job for each product and target.
CLI jobs execute version, capability, completion, SVG, PNG, JPEG, and PDF smokes. LSP jobs execute
initialize, initialized, shutdown, and exit. Each job reports only one product, so no product's
verification result is produced after another product's binary runs in the same writable job. A
clean aggregate job requires the complete matrix to succeed and reverifies the immutable bundle
before any registry candidate, attestation, or GitHub Release job can run.

Stable releases additionally generate Scoop and WinGet candidates from the verified Windows
archive. The Windows job parses the Scoop JSON and requires `winget validate` to pass without
interactive prompts. Prereleases take an explicit successful no-candidate path. Candidate files
remain workflow artifacts for external submission; they are not uploaded as product release assets.

## cargo-binstall

`crates/merman-cli/Cargo.toml` resolves the four published targets as follows:

| Target | Format | Executable path inside the archive |
| --- | --- | --- |
| `aarch64-apple-darwin` | `.tar.xz` | `merman-cli-aarch64-apple-darwin/merman-cli` |
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

The metadata does not claim Linux ARM64 until that target passes the repository's full admission
gate. The current decision and required evidence are recorded in
[`docs/release/CLI_TARGET_ADMISSION.md`](../release/CLI_TARGET_ADMISSION.md). A Homebrew ARM64 Linux
bottle belongs to a different build and verification channel.

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
`capabilities/artifact-profiles-v1.json`; it does not copy the feature list into Nix. Its install
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
from admitting a precompiled Linux ARM64 release target; the latter still requires U8's native
archive and glibc evidence.

## Scoop and WinGet draft candidate contract

The repository can generate draft candidate files for a stable release, but it does not
claim that either central registry already provides Merman. Do not add `scoop install` or
`winget install` instructions to user documentation until the corresponding external submission
is accepted and the user-facing installation guidance is updated with the approved registry command.

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
`packaging/cli-registry/` are one Scoop JSON template and three directly reviewable WinGet YAML
templates. The generator replaces only the validated version, repository URLs, and archive digest;
it does not implement a second WinGet schema validator.

Run the repository checks with:

```bash
python3 -m unittest scripts.test_generate_cli_registry_candidates
```

Before submission, run `winget validate <generated-winget-directory>` on Windows. The tag workflow
runs this command as a release gate over the real verified archive for stable releases. Prereleases
take an explicit no-candidate branch. Candidate generation remains independently runnable for
maintainer review and does not submit to either registry.

## Homebrew formula contract

The Homebrew Core formula is maintained outside this repository. Merman's scheduled
`homebrew.yml` workflow verifies the published formula on macOS and Linux, but it does not edit or
submit Homebrew metadata. It checks out the exact `v<FORMULA_VERSION>` source tag and uses that
release's verifier and artifact profile, so a later `main` branch cannot redefine an older stable
Formula's command or capability contract. Starting at `0.8.0`, a release tag without its verifier
is invalid and cannot fall back to the current branch's implementation. The installed binary's
capability schema and digest must also match that tag's declared capability authority.

Formula versions below `0.8.0` retain the legacy binary-only contract. The `0.8.x` release line must
expose CLI contract 3, match the complete `cli-release` capability set, install four Homebrew
completion files, and install all 12 man pages. A later release line may advance the contract through
its tag-owned verifier. `SUPPORT_ASSETS_SINCE` in `homebrew.yml` is the single operational threshold.
Invalid or prerelease formula versions fail rather than falling back to the weaker check.

Use this shape when preparing the upstream Formula change for `0.8.0`:

```ruby
def install
  features = %w[
    analysis ascii icons jpeg layout-cytoscape layout-elk markdown math
    network-icons parallel-markdown pdf png shell-completions svg
    system-clock system-random system-timezone system-timing
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
