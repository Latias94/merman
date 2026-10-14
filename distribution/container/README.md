# Merman CLI container

Build and run the complete CLI without Rust, Node.js, or Chromium. The image uses a GNU runtime,
includes CA certificates and DejaVu fonts, and runs as an unprivileged user. It supports native
`linux/amd64` and `linux/arm64` builds from their matching verified release archives.

The GHCR publisher is a manual workflow. A merged Dockerfile does not establish registry
availability; check the workflow result and package visibility before using a published image.

## Build locally

Download the CLI archive and adjacent SHA-256 file for the selected version and architecture from
[GitHub Releases](https://github.com/Latias94/merman/releases). Verify the checksum and release
attestation using the [CLI packaging guide](../../docs/releasing/CLI.md), then extract the archive.
Pass its wrapper directory as the build context:

```sh
docker build --file distribution/container/Dockerfile \
  --tag merman-cli:local merman-cli-x86_64-unknown-linux-gnu
docker run --rm merman-cli:local --version
printf 'flowchart LR\nA --> B\n' | docker run --rm -i merman-cli:local render - > diagram.svg
```

For files, mount a directory and use your own user and group so generated files have the expected
ownership:

```sh
docker run --rm --user "$(id -u):$(id -g)" \
  --volume "$PWD:/data" merman-cli:local render diagram.mmd --format png -o diagram.png
```

PowerShell users can use `--volume "${PWD}:/data"` and omit the Linux user override. Docker must
be running Linux containers.

The CLI entry point also accepts `batch`, `lint`, and `rustdoc`. Pin an exact image version or
digest in automation. The image does not include the LSP executable.

## Fonts and networking

DejaVu provides the default font environment, not universal language coverage. Mount your own
font files when using Chinese, Japanese, Korean, or a document-specific typeface:

```sh
docker run --rm --user "$(id -u):$(id -g)" \
  --volume "$PWD:/data" --volume "$PWD/fonts:/fonts:ro" \
  merman-cli:local render diagram.mmd --format png -o diagram.png --font-dir /fonts
```

Set Mermaid's `fontFamily` to the supplied family. SVG output does not embed font files. Network
icons remain opt-in through the CLI; the presence of a CA bundle does not enable network access.

## Publication

Run **CLI Container Release** with an existing stable workspace release tag. It checks the tag's
immutable source, verification-manifest attestation, archive identity, and complete CLI payload;
builds and smoke-tests each native image; then publishes architecture tags and combines them into
`ghcr.io/latias94/merman-cli:VERSION`. The workflow does not rebuild or modify release binaries.
The initial GHCR package needs public visibility configured by its owner.

The PR workflow builds and runs images from that PR's GNU archive artifacts without logging into
a registry or publishing. Docker Hub mirroring is not configured.
