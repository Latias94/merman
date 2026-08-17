# tree-sitter-mermaid

A tolerant, incremental Tree-sitter grammar for Mermaid source, with structured support for all 35
public diagram families in Mermaid 11.16.1.

This package is deliberately adjacent to Merman's semantic parser. Tree-sitter owns the concrete
syntax tree, error recovery, incremental reparsing, and editor queries. `merman-core` remains the
authority for strict validity, semantic models, diagnostics, IR, rendering, navigation identity,
and refactoring safety.

## Distribution

The independently versioned `tree-sitter-mermaid` release provides:

- a Rust crate exposing `LANGUAGE`, `NODE_TYPES`, and the canonical portable queries;
- an npm package with native Node bindings, TypeScript declarations, source-build fallback, and
  release-built Node prebuilds;
- `tree-sitter-mermaid.wasm` at the npm and GitHub release root for `web-tree-sitter` consumers;
- generated C sources, a public header, Make and CMake install surfaces, and pkg-config metadata;
- canonical portable queries plus pre-1.0 query assets for Neovim, Helix, and Zed.

The language is generated explicitly for Tree-sitter ABI 15 with Tree-sitter CLI 0.26.12. The
selected syntax baseline is Mermaid 11.16.1 with ZenUML Core 3.50.1.

## Node.js

```console
npm install tree-sitter tree-sitter-mermaid
```

```js
const Parser = require('tree-sitter');
const Mermaid = require('tree-sitter-mermaid');

const parser = new Parser();
parser.setLanguage(Mermaid);
const tree = parser.parse('flowchart TD\nA --> B\n');
```

The `tree-sitter` peer dependency is optional so browser-only consumers can install the grammar
asset without pulling in the native Node runtime.

## Browser and workers

Use the external `web-tree-sitter` runtime and the exported language WASM. The grammar package does
not wrap runtime initialization, worker policy, or bundler URL resolution in a second SDK. Copy the
exported asset to a public URL, or let the application's bundler resolve the package export.

```js
import { Language, Parser } from 'web-tree-sitter';

await Parser.init();
const language = await Language.load(
  '/assets/tree-sitter-mermaid.wasm',
);
const parser = new Parser();
parser.setLanguage(language);
```

Merman's Playground continues to use its existing semantic-token provider by default. A future
Tree-sitter CST or query inspector can load this WASM lazily without replacing the semantic path.

## Rust

```rust
let language: tree_sitter::Language = tree_sitter_mermaid::LANGUAGE.into();
let mut parser = tree_sitter::Parser::new();
parser.set_language(&language)?;
let tree = parser.parse("flowchart TD\nA --> B\n", None).unwrap();
# Ok::<(), Box<dyn std::error::Error>>(())
```

The crate intentionally does not depend on the Tree-sitter runtime. Applications select a
compatible `tree-sitter` runtime and use the exported `tree-sitter-language::LanguageFn`.

## C

The committed parser and scanner build without the grammar generator:

```console
cmake -S . -B build -DBUILD_SHARED_LIBS=OFF
cmake --build build
cmake --install build --prefix /usr/local
```

Unix-like consumers may alternatively use `make` and `make install`. Regeneration goes through the
pinned npm `generate` script and is never part of the default C source build.

## Queries and editors

`tree-sitter.json` points to the stable portable query set under `queries/portable/`. The
`queries/neovim`, `queries/helix`, and `queries/zed` directories are adoption assets rather than a
promise that npm or crates.io publication automatically updates those editors. Editor integrations
must pin a released repository revision and its `distribution/tree-sitter-mermaid` subdirectory.

## Development

Install the pinned package-local toolchain first:

```console
npm ci --ignore-scripts --prefix distribution/tree-sitter-mermaid
npm rebuild tree-sitter-cli --prefix distribution/tree-sitter-mermaid
```

The normal checks are intentionally small:

```console
npm run check:generated --prefix distribution/tree-sitter-mermaid
npm run test:corpus --prefix distribution/tree-sitter-mermaid
cargo nextest run --locked -p tree-sitter-mermaid --no-fail-fast
npm run test:node --prefix distribution/tree-sitter-mermaid
npm run test:c --prefix distribution/tree-sitter-mermaid
```

WASM freshness is a separate, slower lane:

```console
npm run check:wasm --prefix distribution/tree-sitter-mermaid
npm run test:wasm --prefix distribution/tree-sitter-mermaid
```

The standard Tree-sitter corpus owns CST and recovery expectations. One Rust integration test
projects Merman's strict-valid fixture corpus into Tree-sitter and checks family root selection and
error-free structure. Focused incremental/scanner tests cover the stateful mechanics. The package
does not maintain a second semantic test engine, support-tier lattice, receipt graph, or duplicated
capture forest.

See `docs/development/TREE_SITTER_MERMAID.md` and `docs/release/TREE_SITTER_MERMAID.md` in the
Merman repository for maintenance and release procedures.

## License and provenance

The package is MIT licensed. Source-derived syntax and template attributions are recorded in
`metadata/provenance.json`, `metadata/derivations.json`, `THIRD_PARTY_NOTICES.md`, and
`THIRD_PARTY_LICENSES/`.
