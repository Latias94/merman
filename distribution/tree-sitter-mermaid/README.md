# tree-sitter-mermaid

[![crates.io](https://img.shields.io/crates/v/tree-sitter-mermaid.svg)](https://crates.io/crates/tree-sitter-mermaid) [![npm](https://img.shields.io/npm/v/%40mermanjs%2Ftree-sitter-mermaid.svg)](https://www.npmjs.com/package/@mermanjs/tree-sitter-mermaid) [![MIT license](https://img.shields.io/badge/license-MIT-59636e.svg)](#license-and-provenance)

A tolerant, incremental [Tree-sitter] grammar for Mermaid source. It provides structured concrete
syntax trees and editor queries for all 35 public diagram families in Mermaid 11.16.1, including
the ZenUML integration backed by ZenUML Core 3.50.1.

Use this package for syntax highlighting, syntax-aware selection, folding, and other editor features
that must keep working while a document is incomplete. Use [`@mermanjs/web`] or the Merman Rust
crates when you need strict validation, semantic models, diagnostics, rendering, navigation, or safe
refactoring. A recovered Tree-sitter tree is useful editor state; it is not proof that Mermaid will
accept or render the document.

## Packages

| Consumer | Package or artifact | Provides |
| --- | --- | --- |
| Node.js | [`@mermanjs/tree-sitter-mermaid`] | Native Node binding, TypeScript declarations, queries, and language WASM |
| Browser or Worker | [`@mermanjs/tree-sitter-mermaid`] + [`web-tree-sitter`] | Language WASM and portable queries for the generic browser runtime |
| Rust | [`tree-sitter-mermaid`] | `LANGUAGE`, `NODE_TYPES`, and the portable queries |
| C/C++ and editors | Repository source, or a matching [GitHub Release] when available | Generated C parser/scanner, public header, Make, CMake, and pkg-config metadata |

The grammar has its own version line, independent of Merman. Its npm and Cargo artifacts share that
grammar version, but their registry names differ: the npm package is scoped under `@mermanjs`, while
the Rust crate and C library retain the standard `tree-sitter-mermaid` name.

## Node.js

```console
npm install tree-sitter @mermanjs/tree-sitter-mermaid
```

```js
const Parser = require('tree-sitter');
const Mermaid = require('@mermanjs/tree-sitter-mermaid');

const parser = new Parser();
parser.setLanguage(Mermaid);

const tree = parser.parse('flowchart TD\nA --> B\n');
console.log(tree.rootNode.toString());
```

The `tree-sitter` peer dependency is optional so a browser-only install does not pull in the native
Node runtime. Node applications should use the native binding rather than the browser WASM.

Node ESM consumers use default imports for the CommonJS native bindings:

```js
import Parser from 'tree-sitter';
import Mermaid from '@mermanjs/tree-sitter-mermaid';
```

## Browser and Workers

```console
npm install web-tree-sitter @mermanjs/tree-sitter-mermaid
```

The package exports `@mermanjs/tree-sitter-mermaid/tree-sitter-mermaid.wasm`. Copy that asset to a
public URL with your bundler, then load it with the generic `web-tree-sitter` runtime:

```js
import { Language, Parser } from 'web-tree-sitter';

await Parser.init();
const language = await Language.load('/assets/tree-sitter-mermaid.wasm');

const parser = new Parser();
parser.setLanguage(language);
const tree = parser.parse('sequenceDiagram\nAlice->>Bob: Hello\n');
```

A no-build browser prototype can pin the exact grammar version on jsDelivr:

```js
const language = await Language.load(
  'https://cdn.jsdelivr.net/npm/@mermanjs/tree-sitter-mermaid@0.1.0/tree-sitter-mermaid.wasm',
);
```

The grammar package deliberately does not wrap runtime initialization, Worker lifecycle, or bundler
URL resolution in another JavaScript SDK. Merman's Playground follows the same boundary: a syntax
Worker runs this WASM and the portable highlight query, while a separate semantic Worker owns
diagnostics, completion, navigation, and rename.

## Rust

```console
cargo add tree-sitter tree-sitter-mermaid
```

```rust
let language: tree_sitter::Language = tree_sitter_mermaid::LANGUAGE.into();
let mut parser = tree_sitter::Parser::new();
parser.set_language(&language)?;

let tree = parser
    .parse("flowchart TD\nA --> B\n", None)
    .expect("Tree-sitter returned no tree");
assert!(!tree.root_node().has_error());
# Ok::<(), Box<dyn std::error::Error>>(())
```

The crate exports a `tree-sitter-language::LanguageFn` and does not force a Tree-sitter runtime
version on applications.

## C and C++

The committed parser and scanner build without the grammar generator:

```console
cmake -S . -B build -DBUILD_SHARED_LIBS=OFF
cmake --build build
cmake --install build --prefix /usr/local
```

Unix-like consumers may alternatively use `make` and `make install`.

```c
#include <tree_sitter/api.h>
#include <tree_sitter/tree-sitter-mermaid.h>

int main(void) {
  TSParser *parser = ts_parser_new();
  ts_parser_set_language(parser, tree_sitter_mermaid());
  ts_parser_delete(parser);
  return 0;
}
```

## Compatibility

| Contract | Version |
| --- | --- |
| Mermaid syntax baseline | 11.16.1 |
| ZenUML Core syntax baseline | 3.50.1 |
| Tree-sitter language ABI | 15 |
| Tested Rust and Web runtime | 0.26.12 |
| Native Node runtime contract | 0.25.x |

Before 1.0, a minor release may change named nodes, fields, canonical captures, the language ABI, or
the selected Mermaid baseline. Pin a compatible minor version for application integrations and an
immutable release commit for editor integrations.

## Queries and Editors

`tree-sitter.json` selects the canonical portable queries under `queries/portable/`:

- `highlights.scm` for base syntax highlighting;
- `injections.scm` for embedded languages;
- `locals.scm` for syntax-local scopes; and
- `tags.scm` for syntax-level symbols.

The `queries/neovim`, `queries/helix`, and `queries/zed` directories are pre-1.0 adoption assets.
Those editors own their final query copies and release cadence, so publishing this package does not
update an editor automatically. Downstream integrations should pin a released Merman commit and use
`distribution/tree-sitter-mermaid` as the grammar subdirectory.

## Development

From the Merman repository root, install the pinned package-local toolchain:

```console
npm ci --ignore-scripts --prefix distribution/tree-sitter-mermaid
npm rebuild tree-sitter-cli --prefix distribution/tree-sitter-mermaid
```

Run the ordinary grammar and binding checks:

```console
npm run check:generated --prefix distribution/tree-sitter-mermaid
npm run test:corpus --prefix distribution/tree-sitter-mermaid
cargo nextest run --locked -p tree-sitter-mermaid --no-fail-fast
npm run test:node --prefix distribution/tree-sitter-mermaid
npm run test:c --prefix distribution/tree-sitter-mermaid
```

Language-WASM freshness and execution are separate, slower checks:

```console
npm run check:wasm --prefix distribution/tree-sitter-mermaid
npm run test:wasm --prefix distribution/tree-sitter-mermaid
```

See the [development guide] and [release guide] for grammar ownership, generation, testing, and
publication details.

## License and Provenance

The package is MIT licensed. Source-derived syntax and template attributions are recorded in
`metadata/provenance.json`, `metadata/derivations.json`, `THIRD_PARTY_NOTICES.md`, and
`THIRD_PARTY_LICENSES/`.

[Tree-sitter]: https://tree-sitter.github.io/tree-sitter/
[`@mermanjs/web`]: https://www.npmjs.com/package/@mermanjs/web
[`@mermanjs/tree-sitter-mermaid`]: https://www.npmjs.com/package/@mermanjs/tree-sitter-mermaid
[`web-tree-sitter`]: https://www.npmjs.com/package/web-tree-sitter
[`tree-sitter-mermaid`]: https://crates.io/crates/tree-sitter-mermaid
[GitHub Release]: https://github.com/Latias94/merman/releases
[development guide]: https://github.com/Latias94/merman/blob/main/docs/development/TREE_SITTER_MERMAID.md
[release guide]: https://github.com/Latias94/merman/blob/main/docs/release/TREE_SITTER_MERMAID.md
