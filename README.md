# merman

`merman` is a Rust, headless, 1:1 re-implementation of Mermaid pinned to `mermaid@11.12.2`.

## Reference upstreams

This repository uses optional local checkouts under `repo-ref/` to support parity work against
upstream projects. These checkouts are **not committed** and are **not** git submodules. The pinned
revisions live in `repo-ref/REPOS.lock.json`.

Typical upstreams:

- `mermaid-js/mermaid` → `repo-ref/mermaid`
- `dagrejs/dagre` → `repo-ref/dagre`
- `dagrejs/graphlib` → `repo-ref/graphlib`
- `cure53/DOMPurify` → `repo-ref/dompurify`
- `braintree/sanitize-url` → `repo-ref/sanitize-url`

Populate `repo-ref/*` by cloning each repo at the pinned commit shown in
`repo-ref/REPOS.lock.json`.

## Development

- Verify generated artifacts:
  - `cargo run -p xtask -- verify-generated`
- Format:
  - `cargo fmt`
- Tests (preferred):
  - `cargo nextest run -p merman-core`
- Update golden fixtures:
  - `cargo run -p xtask -- update-snapshots`
- Canonical SVG XML compare (stricter than DOM parity):
  - `cargo run -p xtask -- compare-svg-xml --check`

## CLI (headless JSON)

- Parse a diagram and print the semantic JSON model:
  - `cargo run -p merman-cli -- parse path/to/diagram.mmd --pretty`
- Read from stdin:
  - `cat path/to/diagram.mmd | cargo run -p merman-cli -- parse --pretty -`
- Detect diagram type:
  - `cargo run -p merman-cli -- detect path/to/diagram.mmd`
- Compute headless layout JSON (meta + semantic + layout):
  - `cargo run -p merman-cli -- layout path/to/diagram.mmd --pretty`
- Render SVG (prints to stdout by default):
  - `cargo run -p merman-cli -- render path/to/diagram.mmd --out out.svg`

## Library (headless)

The `merman` crate can be used as a headless library. Enable the optional `render` feature to get a
convenience API for layout + SVG rendering:

- `merman = { path = "...", features = ["render"] }`

## License

Dual-licensed under MIT or Apache-2.0.
