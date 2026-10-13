# Setup Merman CLI

Install an exact official Merman CLI release in a GitHub Actions job. The action adds
`merman-cli` to `PATH` for subsequent steps. It runs with Node.js 24 provided by the runner,
uses the runner's `tar` command to extract archives, and has no npm dependencies.

This action lives in the Merman repository and can be used directly from its subdirectory.
It is not published in GitHub Marketplace. Pin the action to a reviewed full commit SHA;
pin the CLI independently with the required `version` input:

```yaml
steps:
  - uses: Latias94/merman/distribution/github-action@<FULL_COMMIT_SHA>
    with:
      version: '0.9.0'
  - run: merman-cli --version
  - run: merman-cli render diagram.mmd -o diagram.svg
```

Replace `<FULL_COMMIT_SHA>` with a commit containing this action. The version must be an
exact published stable or prerelease version without the `v` prefix, such as `0.9.0` or
`0.8.0-alpha.7`. Version ranges and `latest` are rejected. No release API or token is needed
to download these public release assets.

## Supported runners

| Runner platform | Architecture | Official release target |
| --- | --- | --- |
| Linux with glibc | x64 | `x86_64-unknown-linux-gnu` |
| Linux with glibc | ARM64 | `aarch64-unknown-linux-gnu` |
| macOS | x64 | `x86_64-apple-darwin` |
| macOS | ARM64 | `aarch64-apple-darwin` |
| Windows | x64 | `x86_64-pc-windows-msvc` |

Use a runner compatible with both Node.js 24 and the selected CLI binary. The action selects
the GNU Linux release; musl/Alpine and Windows ARM64 do not have supported official archives.
The selected release's Linux glibc baseline applies. This action does not change the
compatibility of previously published binaries. Self-hosted runners must provide `tar` with
XZ support on Unix or ZIP support on Windows; current GitHub-hosted runners provide it.

## Installation and outputs

The action downloads only the selected archive and its adjacent `.sha256` file from
`https://github.com/Latias94/merman/releases/download/v<VERSION>/`. It checks the SHA-256
digest of the downloaded bytes before extraction. An unavailable asset or invalid checksum
fails the step without publishing an installation to `PATH`. The checksum binds downloaded
bytes to the repository's release metadata; it is not an independent provenance check.
Archives are trusted as official release payloads after this check. Merman's release workflow
validates their structure before publication; see the [CLI release contract](../../docs/releasing/CLI.md).

Each invocation extracts into a separate directory under `RUNNER_TEMP` and preserves the
archive's licenses, notices and support assets. It writes only GitHub's `GITHUB_PATH` and
`GITHUB_OUTPUT` environment files. It does not update shell startup files or install Node.js,
Chromium, fonts, or a source-build fallback. Invoke rendering, linting and batch commands in
your own subsequent steps, with the inputs and fonts your workflow requires.

| Output | Value |
| --- | --- |
| `version` | Selected exact release version |
| `path` | Absolute path to the installed CLI executable |

## Development checks

With Node.js 24 or newer and the same archive tools as the runner:

```console
node --test distribution/github-action/setup.test.mjs
```

The tests use a local HTTP fixture server, real SHA-256 verification, the system archive
tool and an executable fixture. To test the official release on a runner, use the local
action after checkout and run `merman-cli --version` and a small SVG render in subsequent
steps. Run that smoke on all five supported platform/architecture pairs before claiming
cross-platform installation coverage.
