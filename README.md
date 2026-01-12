# merman

`merman` is a Rust, headless, 1:1 re-implementation of Mermaid pinned to `mermaid@11.12.2`.

## Reference upstreams

This repository uses git submodules under `repo-ref/` to pin upstream baselines:

- `repo-ref/mermaid` (`mermaid-js/mermaid`)
- `repo-ref/dompurify` (`cure53/DOMPurify`)
- `repo-ref/sanitize-url` (`braintree/sanitize-url`)

After cloning, initialize them:

```bash
git submodule update --init --recursive
```

## Development

- Verify generated artifacts:
  - `cargo run -p xtask -- verify-generated`
- Format:
  - `cargo fmt`
- Tests (preferred):
  - `cargo nextest run -p merman-core`
- Update golden fixtures:
  - `cargo run -p xtask -- update-snapshots`

## CLI (headless JSON)

- Parse a diagram and print the semantic JSON model:
  - `cargo run -p merman-cli -- parse path/to/diagram.mmd --pretty`
- Read from stdin:
  - `cat path/to/diagram.mmd | cargo run -p merman-cli -- parse --pretty -`
- Detect diagram type:
  - `cargo run -p merman-cli -- detect path/to/diagram.mmd`
