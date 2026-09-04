# Tree-sitter Mermaid Release

`tree-sitter-mermaid` is an independently versioned distribution for crates.io, npm, and optional
GitHub Releases. Passing this checklist prepares artifacts; it does not authorize registry
publication without the protected release environment and maintainer credentials.

## Release identity

- Package root: `distribution/tree-sitter-mermaid`
- Initial version: `0.1.0`
- Tag form: `tree-sitter-mermaid-v0.1.0`
- Cargo package: `tree-sitter-mermaid`
- npm package: `@mermanjs/tree-sitter-mermaid`
- Language symbol: `mermaid`
- Language ABI: 15
- Tree-sitter CLI/Rust/web runtime: 0.26.12
- Native Node runtime contract: 0.25.x
- Mermaid baseline: 11.16.1
- ZenUML Core baseline: 3.50.1

Cargo, npm, and `tree-sitter.json` versions must match. A release tag must resolve to the immutable
commit containing the generated parser, root WASM, queries, and legal material.

## Preflight

Install the fixed toolchain and run the non-duplicated gates:

```console
npm ci --ignore-scripts --prefix distribution/tree-sitter-mermaid
npm rebuild tree-sitter-cli --prefix distribution/tree-sitter-mermaid
npm run build:node --prefix distribution/tree-sitter-mermaid
npm run check:generated --prefix distribution/tree-sitter-mermaid
npm run test:corpus --prefix distribution/tree-sitter-mermaid
cargo fmt --all -- --check
cargo nextest run --locked -p tree-sitter-mermaid --no-fail-fast
cargo clippy --locked -p tree-sitter-mermaid --all-targets -- -D warnings
npm run test:node --prefix distribution/tree-sitter-mermaid
npm run test:c --prefix distribution/tree-sitter-mermaid
```

Run the slower language-WASM lane separately:

```console
npm run check:wasm --prefix distribution/tree-sitter-mermaid
npm run test:wasm --prefix distribution/tree-sitter-mermaid
```

Then verify clean consumer packages and legal projections:

```console
cargo package --locked -p tree-sitter-mermaid --list
npm pack ./distribution/tree-sitter-mermaid --dry-run --json
npm run test:package-smoke --prefix distribution/tree-sitter-mermaid
python3 scripts/verify-third-party-licenses.py
python3 scripts/verify_crate_package_legal_materials.py
git diff --check
```

## Release products

One candidate release stages:

- `tree-sitter-mermaid-0.1.0.crate` for crates.io;
- an `@mermanjs/tree-sitter-mermaid` tarball containing Node source fallback and release-built
  N-API prebuilds;
- root `tree-sitter-mermaid.wasm` for npm and GitHub Releases;
- a grammar-subdirectory source archive for editor and C consumers;
- SHA-256 checksums and GitHub's build provenance for the staged files.

The npm package contains the Node binding, `index.d.ts`, prebuilds, grammar/generated sources,
queries, C Make/CMake/header/pkg-config inputs, root WASM, and legal/provenance files. It excludes
tests, scripts, fixture oracles, metrics, receipts, local build directories, and dependency installs.

The Cargo crate contains only the Rust binding, generated parser/scanner sources, node types,
portable queries, package metadata, documentation, and legal files. It does not contain language
WASM, Node files, tests, or grammar-maintenance infrastructure.

## Native prebuilds

Release jobs run `prebuildify --napi` on the supported Linux, macOS, and Windows targets and
merge the resulting `prebuilds/**` directories before `npm pack`. The package keeps the standard
`node-gyp-build` source fallback. Release package smoke sets
`TREE_SITTER_MERMAID_REQUIRE_PREBUILDS=1` so a missing prebuild cannot be mistaken for a complete
npm candidate.

## Publication and retry

The crates.io package is `tree-sitter-mermaid`; the npm package is
`@mermanjs/tree-sitter-mermaid`. Ordinary publication uses the protected workflow and the exact
release commit. If one registry job fails, re-run only failed jobs from the same workflow run. Each
publish job reconciles an already-visible version against the staged candidate bytes: an exact
match is success, while any mismatch fails closed. Both registries reject overwriting an existing
immutable version.

The first npm publication requires a maintainer's short-lived 2FA-protected bootstrap credential
for `@mermanjs/tree-sitter-mermaid` before Trusted Publishing can be configured. Do not store that
bootstrap token in repository secrets. Publish the exact candidate from the tagged workflow run
that will own recovery, then rerun only that same run's failed jobs. Native prebuilds are not assumed
to be byte-reproducible across separate runs. crates.io publication uses the protected crates.io
environment.

Set `publish_github_release` only when the standalone source/WASM release should become public.
Deferring it is supported, but later publication must use the original verified candidate for that
tag rather than a separately rebuilt archive.

For `0.1.0`, crates.io and npm publication completed on 2026-08-18 from
`tree-sitter-mermaid-v0.1.0`. npm used a manual 2FA bootstrap and therefore has no npm provenance.
Trusted Publishing is configured for later versions through `release-tree-sitter-mermaid.yml` and
the `npm` environment. The standalone GitHub Release is intentionally deferred.

For a failed publication run, re-run the failed jobs from the same workflow run. Native prebuilds are
run-scoped artifacts and are deliberately not imported from another run; if the run's artifacts have
expired or are unavailable, start a new tagged run and treat it as a new candidate.

Downstream Neovim, Helix, and Zed changes happen only after the immutable GitHub release exists.
Those repositories pin the release commit and their own query copies; they do not consume the npm
or Cargo package directly.

Publish the Cargo package before any workspace release whose `merman-lsp` manifest names that exact
version. The Playground's monorepo build consumes the distribution source tree directly. External
browser consumers use the npm package's WASM and query surface, so installation instructions must
not assume those registry assets exist until the npm publication completes.

## Semver before 1.0

Use a minor release for named-node or field removals, canonical capture removals, language ABI
changes, or a selected Mermaid/ZenUML baseline change. Use a patch release for compatible grammar
fixes, recovery improvements, and additive query captures. Document migrations in
`distribution/tree-sitter-mermaid/docs/query-migration.md`.
