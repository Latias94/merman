# Arch Linux source package

This directory prepares the stable `merman-cli` source package for AUR submission. It does not
claim that an AUR entry exists. Publishing the AUR repository is a separate maintainer action.

The package builds `v0.9.0` from the official GitHub tag archive, checked with SHA-256. That tag
points to commit `f754a3c3077d35d2a372c2fec04f9e9b87db1c3f`. It uses the source archive's canonical
`cli-release` artifact profile through `scripts/artifact_profile_recipe.py`; features are not
duplicated in the package recipe. Supported architectures are `x86_64` and `aarch64`.
The complete profile includes the EPL-2.0 ELK layout port and its third-party notices.

## Build and install

Run these commands on Arch Linux, as a regular user, from this directory:

```sh
makepkg --syncdeps --cleanbuild
sudo pacman -U merman-cli-0.9.0-1-*.pkg.tar.zst
```

Install `base-devel` first. The package requires Rust 1.95 or newer and Python 3.11 or newer.
CMake and Ninja build the locked `aws-lc-sys` native dependency; the bundled TLS provider does
not require OpenSSL at runtime. Both `build()` and `check()` select the
[upstream CMake builder](https://aws.github.io/aws-lc-rs/resources.html#build-system) so
[jitterentropy's per-source `-O0`](https://github.com/aws/aws-lc/blob/02561621ffa4cf17c0c4f70bc11a82df36b42ae9/third_party/jitterentropy/CMakeLists.txt#L63)
follows makepkg's optimization flags. The package retains Arch's compiler flags and the CPU
jitter entropy source. Following the
[Arch Rust packaging guidelines](https://wiki.archlinux.org/title/Rust_package_guidelines#Build),
`options=('!lto')` disables makepkg's GCC LTO injection into native C objects, which Rust's LLD
cannot link. The canonical Cargo `dist` profile retains its own ThinLTO and other Arch build
flags remain enabled. `prepare()` fetches the locked Cargo dependencies into the build directory.
`build()` and `check()` use frozen dependency resolution and do not fetch crates.

The package installs:

- `/usr/bin/merman-cli` with the complete release capability set;
- Bash, Zsh, and Fish completions in their standard system directories;
- PowerShell completion in `/usr/share/pwsh/completions/_merman-cli.ps1` and Elvish completion in
  `/usr/share/elvish/lib/merman-cli.elv`, which require the user's shell integration;
- all 15 command and subcommand manual pages under `/usr/share/man/man1`;
- the CLI README, project licenses, third-party notices, and complete `THIRD_PARTY_LICENSES` tree
  under `/usr/share/doc/merman-cli`, plus project licenses under `/usr/share/licenses/merman-cli`.

System fonts are optional. Install a font package such as `ttf-dejavu` for native text measurement
or text in raster/PDF output; `fontconfig` supplies system discovery configuration. Ordinary
Mermaid graphical rendering retains the release's deterministic measurement default. See
`merman-cli render --help` for explicit font controls.

## Verify and update

`check()` runs the upstream `distribution_assets` contract against the canonical profile. CI
builds the package on Arch Linux, installs it, and verifies the installed binary, capabilities,
all five completions, and all manual pages against the same stable source tree:

```sh
python src/merman-0.9.0/scripts/verify_cli_installation.py \
  --package-version 0.9.0 --prefix /usr \
  --contract-root src/merman-0.9.0 --completion-layout nix
```

The verifier's existing `nix` layout is also the system layout used by this package. A Windows
syntax check cannot establish an Arch build or installation result; the Arch CI job owns that
evidence. The `aarch64` recipe uses the upstream admitted host target; native ARM64 validation
remains necessary before claiming an ARM64 build result.

For a new stable release, update `pkgver`, reset `pkgrel` to `1`, verify the release tag and source
archive checksum, and regenerate `.SRCINFO` with Arch's own tooling:

```sh
makepkg --verifysource
makepkg --printsrcinfo > .SRCINFO
```

For packaging-only changes to the same upstream release, increment `pkgrel`. Review the metadata
diff and complete the Arch build and installation check before publishing. These files follow
the [PKGBUILD reference](https://man.archlinux.org/man/PKGBUILD.5.en) and
[Arch Rust packaging guidelines](https://wiki.archlinux.org/title/Rust_package_guidelines).
