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

## Installation channels

| Channel | Build source | Installs support assets | Repository claim |
| --- | --- | --- | --- |
| Direct GitHub archive | cargo-dist `cli-release` binary | Yes, under `completions/` and `man/` | Published release artifact |
| cargo-dist shell or PowerShell installer | Binary extracted from the release archive | No | Published release installer |
| `cargo binstall merman-cli` | Official release archive, then source fallback | No | Explicit manifest metadata |
| `cargo install merman-cli` | crates.io source | No | Complete defaults; custom features supported |
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
payload contains the executable, `completions/`, `man/`, `THIRD_PARTY_NOTICES.md`, and
`THIRD_PARTY_LICENSES/`.

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
gate. A Homebrew ARM64 Linux bottle belongs to a different build and verification channel.

## Scoop and WinGet draft candidate contract

The repository can generate draft candidate files for a stable release, but it does not
claim that either central registry already provides Merman. Do not add `scoop install` or
`winget install` instructions to user documentation until the corresponding external submission
is accepted and `docs/release/SURFACES.json` is updated from `manual-registry`.

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
python3 scripts/verify-release-surfaces.py
```

Before submission, run `winget validate <generated-winget-directory>` on Windows. When U7 wires
candidates into publication, that command will become a Windows release gate over the real verified
archive; stable releases will require it and prereleases will take an explicit no-candidate branch.
Until then, draft generation is an independently runnable maintainer check and is not a
release-workflow claim.

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
