# Tree-sitter Mermaid distribution and README research

**Researched:** 2026-08-18

**Merman baseline:** [`ebdd5164e`](https://github.com/Latias94/merman/tree/ebdd5164ee7998d3b7eb27197795b2ae873843c1)

**Scope:** npm naming, Tree-sitter ecosystem compatibility, artifact identity, and a consumer-facing
README for `distribution/tree-sitter-mermaid`. This is a research record, not a publication
authorization.

## Recommendation

1. Publish the npm package as **`@mermanjs/tree-sitter-mermaid`**. Keep the repository/product name,
   crates.io crate, C library, exported language symbol, and WASM filename as
   **`tree-sitter-mermaid`**. A scoped npm name is a registry coordinate, not a different grammar
   identity.
2. Keep one independently versioned grammar release and align its Cargo, npm, and
   `tree-sitter.json` versions. Do not align it to Merman's application version. Use the existing
   monorepo tag form `tree-sitter-mermaid-v0.1.0`.
3. Rewrite the README around four consumer paths: Node, browser/WASM, Rust, and editor/C adoption.
   Retain the current semantic-parser boundary and provenance material, but move generator, CI, and
   V8 troubleshooting details out of the first-use path.
4. Do not add a badge wall or a repeated diagram gallery. Mature official grammars are generally
   terse; they add detail only where a consumer needs a special invocation, a limitation, or a
   measured property.

## Evidence boundary

Tree-sitter recommends publishing a stable grammar to GitHub, crates.io, npm, and PyPI, and treats
node-shape changes as semantic-versioning events because downstream queries and tree traversal can
break ([Tree-sitter publishing guide](https://tree-sitter.github.io/tree-sitter/creating-parsers/6-publishing.html)).
Its `tree-sitter version` command updates every selected language binding together so a release has
one grammar version across ecosystems
([Tree-sitter version reference](https://tree-sitter.github.io/tree-sitter/cli/version.html)).
The preferred repository convention is `tree-sitter-<language>`
([Tree-sitter getting started guide](https://tree-sitter.github.io/tree-sitter/creating-parsers/1-getting-started.html)).
Neither source requires the npm registry coordinate to be unscoped.

The npm and crates.io availability observations below are date-bound. On 2026-08-18, the official
registry endpoints returned `404` for both
[`tree-sitter-mermaid`](https://registry.npmjs.org/tree-sitter-mermaid) and
[`@mermanjs/tree-sitter-mermaid`](https://registry.npmjs.org/%40mermanjs%2Ftree-sitter-mermaid), and
crates.io reported that
[`tree-sitter-mermaid`](https://crates.io/api/v1/crates/tree-sitter-mermaid) did not exist. These
names must be rechecked immediately before the first publication; availability is not a durable
property.

## What mature grammar READMEs do

### High-value patterns

| Source | What the README contains | Lesson for Mermaid |
| --- | --- | --- |
| [`tree-sitter-javascript`](https://github.com/tree-sitter/tree-sitter-javascript/blob/58404d8cf191d69f2674a8fd507bd5776f46cb11/README.md) | A one-line product definition, the language/specification boundary, and authoritative references. | State the Mermaid baseline and tolerance boundary once; link the source rather than reproducing its documentation. |
| [`tree-sitter-typescript`](https://github.com/tree-sitter/tree-sitter-typescript/blob/75b3874edb2dc714fb1fd77a32013d0f8699989f/README.md) | A short special-case usage example because one package exports two grammars. | Show code only where package behavior is not obvious. Mermaid needs Node/WASM examples because it intentionally exposes two runtime surfaces. |
| [`tree-sitter-rust`](https://github.com/tree-sitter/tree-sitter-rust/blob/77a3747266f4d621d0757825e6b11edcbf991ca5/README.md) | One measured performance characteristic and language references. | Include measurements only when reproducible and decision-relevant; do not turn CI counts into product claims. |
| [`tree-sitter-grammars/template`](https://github.com/tree-sitter-grammars/template/blob/9c46d09d688d27c7aef31c2b32f50260de4e7906/README.md) | A description and reference section; registry badges are commented out until publication. | The default is a small README. Extra sections must serve a real consumer path. |
| [`tree-sitter-yaml`](https://github.com/tree-sitter-grammars/tree-sitter-yaml/blob/a1c4812a73ec5e089de8e441fdea3a921e8d5079/README.md) | A concise README while its npm package is scoped as `@tree-sitter-grammars/tree-sitter-yaml` and its crate remains `tree-sitter-yaml`. | Scoped npm plus unscoped Cargo is established Tree-sitter practice, not an ecosystem incompatibility. |
| [`tree-sitter-markdown`](https://github.com/tree-sitter-grammars/tree-sitter-markdown/blob/a0a00f817d02412bd92c54d316f164d827b57b5c/README.md) | Goals, explicit correctness limitations, editor direction, and special standalone/WASM usage. | Mermaid should prominently say that the CST is tolerant/editor-oriented and that `merman-core` remains the strict semantic authority. |

Tree-sitter's own Web binding documentation recommends installing a grammar package from npm to
obtain its language WASM and distinguishes that browser artifact from the faster native Node
binding
([Web Tree-sitter README](https://github.com/tree-sitter/tree-sitter/blob/master/lib/binding_web/README.md#getting-the-wasm-language-files)).
That distinction is already architecturally correct in the current Mermaid README.

### Mermaid-specific comparisons

The existing Mermaid grammars provide useful examples, but none should be copied wholesale:

| Source | Useful signal | Limitation for this README |
| --- | --- | --- |
| [`monaqa/tree-sitter-mermaid`](https://github.com/monaqa/tree-sitter-mermaid/blob/90ae195b31933ceb9d079abfa8a3ad0a36fee4cc/README.md) | Very concise description and an honest support checklist. It is also the Mermaid grammar currently listed in Tree-sitter's community parser index ([parser list](https://github.com/tree-sitter/tree-sitter/wiki/List-of-parsers)). | It does not explain package consumption and its checklist is tied to a much narrower grammar. A long copied checklist would become a second support database. |
| [`pappasam/tree-sitter-mermaid`](https://github.com/pappasam/tree-sitter-mermaid/blob/1a11e2d8cf11afcfdb768f537c1a9bde294c24f9/README.md) | Clearly explains tolerant recovery and gives a concrete Neovim source-install path. | It is source/editor oriented and does not cover a native Node package, Rust crate, C install, or browser WASM. |
| [`singularity-ng/singularity-parser-mermaid`](https://github.com/singularity-ng/singularity-parser-mermaid/blob/f5ac2752fbf0f74f9c836014b87e511303d2abae/README.md) | Shows the discoverability value of putting install commands near the top. | It repeats mutable coverage/test claims, includes a large badge set and diagram gallery, and declares registry publication even though the official npm and crates.io endpoints returned `404` on the research date. This is the clearest pattern to avoid. |

## Scoped npm compatibility

### Node `require` and `import`

npm requires consumers to use the full scope for install and `require`, but otherwise treats the
package like any other module
([npm scope documentation](https://docs.npmjs.com/using-npm/scope.html),
[npm package usage](https://docs.npmjs.com/using-npm-packages-in-your-projects/)). Therefore the
public specifier becomes:

```text
@mermanjs/tree-sitter-mermaid
```

The current package is CommonJS, exports `module.exports = language`, and publishes explicit root,
WASM, query, source, and metadata subpaths
([`bindings/node/index.js`](../../distribution/tree-sitter-mermaid/bindings/node/index.js),
[`package.json`](../../distribution/tree-sitter-mermaid/package.json)). Changing the npm `name`
does not require changing the native binding or `tree_sitter_mermaid` C symbol. It does require
changing every consumer-facing package specifier.

The existing root export remains usable from both module systems:

- CommonJS uses `require('@mermanjs/tree-sitter-mermaid')`.
- Node ESM can use a default import because Node exposes a CommonJS module's `module.exports` as its
  reliable default export
  ([Node ESM interoperability](https://nodejs.org/api/esm.html#interoperability-with-commonjs)).
- The current `exports` fallback is available to `import`; Node documents `default` as the generic
  fallback condition and supports scoped package self-resolution
  ([Node package exports](https://nodejs.org/api/packages.html#conditional-exports)).

TypeScript's current declaration uses `export =`, so the lowest-assumption typed form remains
`import Mermaid = require('@mermanjs/tree-sitter-mermaid')`. A JavaScript ESM example may use a
default import. The README should not imply that named imports exist.

The optional `tree-sitter` peer means a browser-only dependency does not pull the native Tree-sitter
runtime, but it does not make npm installation free of native-package behavior: the current grammar
manifest still has a `node-gyp-build` install hook and Node binding dependencies. Consumers that
only need a remotely loaded language WASM can use an exact-version CDN URL without installing the
grammar package. Bundled applications should install the package and treat the prebuilt/source
native path as part of the package contract.

The first scoped publication must be public. npm documents that scoped packages default to private
visibility and require `--access public`; `publishConfig.access = "public"` makes that intent
manifest-local
([npm scoped public packages](https://docs.npmjs.com/creating-and-publishing-scoped-public-packages/)).
The maintained `tree-sitter-yaml` package uses exactly that scoped-name/public-access pattern
([package manifest](https://github.com/tree-sitter-grammars/tree-sitter-yaml/blob/a1c4812a73ec5e089de8e441fdea3a921e8d5079/package.json)).

### Browser WASM and jsDelivr

jsDelivr mirrors any public npm package file using
`/npm/package@version/file` and recommends pinning a production URL rather than using `latest`
([jsDelivr usage documentation](https://github.com/jsdelivr/jsdelivr#usage-documentation)). Its
public API has explicit scoped-package routes
([jsDelivr OpenAPI specification](https://github.com/jsdelivr/data.jsdelivr.com/blob/master/src/public/v1/spec.yaml)).

The existing Merman line already proves the namespace and WASM path combination:

- npm publishes [`@mermanjs/web`](https://registry.npmjs.org/%40mermanjs%2Fweb/latest) as a public
  package from the Merman repository;
- its [jsDelivr package page](https://www.jsdelivr.com/package/npm/@mermanjs/web) accepts the scoped
  name; and
- its exact-version
  [`merman_wasm_bg.wasm`](https://cdn.jsdelivr.net/npm/@mermanjs/web@0.7.0/pkg/merman_wasm_bg.wasm)
  is served with `application/wasm` and cross-origin access.

After `0.1.0` is published, the grammar asset URL should therefore be:

```text
https://cdn.jsdelivr.net/npm/@mermanjs/tree-sitter-mermaid@0.1.0/tree-sitter-mermaid.wasm
```

The scope appears literally in the URL; it must not be percent-encoded as `%40` in the CDN path.
The README must pin an exact version. It should also show the npm-installed asset subpath for
bundled applications so jsDelivr is an option, not a mandatory runtime dependency.

### Tree-sitter and editor discovery

The npm scope does not change Tree-sitter's language identity. Tree-sitter reads the grammar name,
`source.mermaid`, file types, injection regex, and query paths from `tree-sitter.json`
([Tree-sitter init reference](https://tree-sitter.github.io/tree-sitter/cli/init.html#structure-of-tree-sitterjson),
[current Mermaid metadata](../../distribution/tree-sitter-mermaid/tree-sitter.json)). Syntax
highlighting conventionally keeps queries in the grammar repository
([Tree-sitter highlighting guide](https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html#overview)).
The community parser index also discovers grammars by repository URL and generated-parser metadata,
not by requiring an unscoped npm package
([Tree-sitter parser list](https://github.com/tree-sitter/tree-sitter/wiki/List-of-parsers)).

Consequently:

- Node and browser consumers use the scoped npm coordinate.
- Rust consumers use the crates.io coordinate.
- Neovim, Helix, Zed, and parser indexes continue to pin the Git repository, revision, subdirectory,
  generated C parser, and their selected query profile.
- Discoverability should come from the repository name, `tree-sitter`/`mermaid` keywords, the parser
  index, release notes, and editor integrations. Publishing a second unscoped npm alias would add a
  second release identity without improving those editor paths.

## Artifact naming matrix

Names should be consistent at the language level, but they should follow each ecosystem's namespace
rules rather than being forced to identical strings.

| Surface | Recommended identity | Reason |
| --- | --- | --- |
| Product and README title | `tree-sitter-mermaid` | Matches the Tree-sitter repository convention and language ecosystem search term. |
| npm | `@mermanjs/tree-sitter-mermaid` | Reuses the established official Merman namespace and avoids a global unscoped ownership race. |
| npm import/require | `@mermanjs/tree-sitter-mermaid` | npm requires the full scope at every package reference. |
| npm tarball | `mermanjs-tree-sitter-mermaid-0.1.0.tgz` (npm-generated) | npm derives scoped tarball filenames; do not hand-rename registry artifacts. |
| crates.io | `tree-sitter-mermaid` | crates.io has no npm-style organization scope and this follows grammar convention. |
| Rust source path | `tree_sitter_mermaid` | Cargo's normal hyphen-to-underscore crate import mapping. |
| CMake/pkg-config/library | `tree-sitter-mermaid` / `libtree-sitter-mermaid` | Matches the current generated C build surface and Tree-sitter templates. |
| C header | `tree_sitter/tree-sitter-mermaid.h` | Matches the current public include path. |
| C language function | `tree_sitter_mermaid()` | Stable generated language symbol; registry branding must not affect ABI. |
| WASM asset | `tree-sitter-mermaid.wasm` | Runtime-neutral language asset and current package export. |
| GitHub tag | `tree-sitter-mermaid-v0.1.0` | Avoids collisions with Merman releases in a monorepo; already selected by the release contract. |
| GitHub release title | `tree-sitter-mermaid 0.1.0` | Human-readable product plus independent version. |
| Source archive | `tree-sitter-mermaid-0.1.0.tar.gz` | Runtime-neutral C/editor source artifact. |

The current C names are visible in
[`CMakeLists.txt`](../../distribution/tree-sitter-mermaid/CMakeLists.txt),
[`Makefile`](../../distribution/tree-sitter-mermaid/Makefile), and
[`tree-sitter-mermaid.pc.in`](../../distribution/tree-sitter-mermaid/bindings/c/tree-sitter-mermaid.pc.in).
The version-alignment and tag policy already live in the
[Tree-sitter Mermaid release contract](../release/TREE_SITTER_MERMAID.md#release-identity).

## Baseline README audit (`ebdd5164e`)

### Keep

The baseline
[`README.md`](https://github.com/Latias94/merman/blob/ebdd5164ee7998d3b7eb27197795b2ae873843c1/distribution/tree-sitter-mermaid/README.md)
already has several important strengths:

- It immediately distinguishes tolerant CST/editor ownership from Merman's strict semantics, IR,
  diagnostics, and rendering. This is the most important architectural expectation.
- It documents native Node and browser WASM as different consumption paths instead of inventing a
  grammar-specific JavaScript SDK.
- It shows Rust and C entry points and explains why the crate exposes `LanguageFn` rather than
  owning the Tree-sitter runtime.
- It identifies the Mermaid, ZenUML, Tree-sitter CLI, and ABI baselines.
- It explains portable versus editor-specific queries and ends with license/provenance links.

### Change before publication

| Priority | Gap | Recommended change |
| --- | --- | --- |
| P0 | Node install and `require` use the unscoped `tree-sitter-mermaid` name. | Replace every npm package reference with `@mermanjs/tree-sitter-mermaid`; keep Cargo/C names unscoped. |
| P0 | The browser example uses a hypothetical `/assets/...` URL, so it does not prove either package export or CDN consumption. | Show one exact-version jsDelivr URL and one installed-package asset subpath. State that the CDN URL exists only after npm publication. |
| P0 | No clean ESM example is shown. | Add a short Node ESM default-import example and retain CommonJS as the canonical native-binding example. Do not show named imports from the grammar package. |
| P0 | The scoped package's public visibility is not visible to readers. | State that it is a public npm package; implementation should set `publishConfig.access` rather than relying only on workflow flags. |
| P0 | “Optional peer” can be read as “browser installation never runs native package setup,” although the package has a `node-gyp-build` install hook. | Say narrowly that the optional peer avoids pulling the native Tree-sitter runtime. Recommend exact-version CDN loading to browser-only consumers who do not need an npm install. |
| P1 | The distribution inventory precedes first use and mixes product contract with release implementation. | Replace it with a compact “Choose a runtime” table, then put installable examples first. |
| P1 | The Node 24 `--liftoff-only` CI explanation occupies the browser consumer path. | Move it to a short troubleshooting/development note. The consumer rule is simply: native Node binding for Node, language WASM for browsers/workers. |
| P1 | The Rust snippet omits the two dependencies needed by a clean consumer. | Include `cargo add tree-sitter@0.26.12 tree-sitter-mermaid@0.1.0` or an equivalent `Cargo.toml` fragment. |
| P1 | Development commands assume the Merman repository root but the same README is rendered by npm/crates.io. | Label repository-root commands explicitly and link detailed maintenance/release docs instead of presenting them as package-user setup. |
| P1 | “All 35 public families” is a high-value claim but its meaning is not adjacent to the claim. | Define it as structured, tolerant CST support at the pinned Mermaid baseline; explicitly state that it is not strict semantic validation or Mermaid.js rendering equivalence. Link the conformance/migration evidence rather than duplicating a 35-row checklist. |
| P2 | Query test-policy details are longer than most consumers need. | Keep the canonical portable-query contract and editor-profile status; move detailed test philosophy to development docs. |

No badge is recommended for the initial rewrite. In a monorepo, a repository-wide CI badge does not
precisely state grammar health, and registry/version badges should not exist before the package
exists. Direct registry links in the installation table are enough after publication.

## Recommended README outline

The target should be a compact consumer document, approximately 150-220 lines, with this order:

1. **Title and two-sentence contract**
   - “Tolerant, incremental Tree-sitter grammar for Mermaid.”
   - Structured support baseline and the CST-versus-`merman-core` semantic boundary.
2. **Choose a runtime**
   - A four-row table for Node, browser/worker, Rust, and C/editor source.
   - Package coordinate, runtime dependency, and artifact used by each row.
3. **Node.js**
   - Scoped install command.
   - CommonJS parse example with `source_file`, no-error, and family-root assertions.
   - One short ESM default-import variant.
4. **Browser and workers**
   - External `web-tree-sitter@0.26.12` ownership.
   - Exact-version jsDelivr language-WASM example.
   - Installed asset export and portable highlight-query export.
   - One sentence about Merman Playground's syntax/semantic worker split.
5. **Rust**
   - Dependency fragment plus `LANGUAGE` example.
   - List `NODE_TYPES` and four portable query constants without implementation narrative.
6. **C and editors**
   - CMake install commands.
   - `tree-sitter.json`, portable query path, and links to Neovim/Helix/Zed adoption notes.
   - Do not imply that publishing to npm automatically updates editors.
7. **Compatibility and scope**
   - Grammar version, ABI 15, Tree-sitter CLI/Rust/web runtime 0.26.12, native Node runtime 0.25.x,
     Mermaid 11.16.1, and ZenUML Core 3.50.1.
   - Tolerant syntax support is not strict Mermaid validity or rendering parity.
8. **Development**
   - The smallest normal check set, explicitly marked “from the Merman repository root.”
   - Links to maintenance, migration, and release documents.
9. **License and provenance**
   - Retain the current exact provenance file links.

## Candidate installation examples

These examples should be copied into package smoke tests so documentation and the packed artifact
cannot drift.

### Native Node, CommonJS

```console
npm install tree-sitter @mermanjs/tree-sitter-mermaid
```

```js
const Parser = require('tree-sitter');
const Mermaid = require('@mermanjs/tree-sitter-mermaid');

const parser = new Parser();
parser.setLanguage(Mermaid);

const tree = parser.parse('flowchart TD\nA --> B\n');
if (tree.rootNode.type !== 'source_file' || tree.rootNode.hasError) {
  throw new Error(tree.rootNode.toString());
}
console.log(tree.rootNode.namedChildren[0].type); // flowchart_diagram
```

This matches the assertions already used by the current
[`binding_test.js`](../../distribution/tree-sitter-mermaid/bindings/node/binding_test.js).

### Native Node, ESM

```js
import Parser from 'tree-sitter';
import Mermaid from '@mermanjs/tree-sitter-mermaid';

const parser = new Parser();
parser.setLanguage(Mermaid);
const tree = parser.parse('sequenceDiagram\nAlice->>Bob: Hello\n');
```

This is JavaScript ESM interoperability with the CommonJS binding. TypeScript projects that do not
enable synthetic default imports should use `import Mermaid = require(...)`, matching the package's
`export =` declaration.

### Browser or worker

```console
npm install web-tree-sitter
```

```js
import { Language, Parser } from 'web-tree-sitter';

await Parser.init();
const language = await Language.load(
  'https://cdn.jsdelivr.net/npm/@mermanjs/tree-sitter-mermaid@0.1.0/tree-sitter-mermaid.wasm',
);

const parser = new Parser();
parser.setLanguage(language);
const tree = parser.parse('flowchart TD\nA --> B\n');

console.log(language.abiVersion); // 15
tree.delete();
parser.delete();
```

Bundled applications should resolve or copy the exported package asset
`@mermanjs/tree-sitter-mermaid/tree-sitter-mermaid.wasm` instead of depending on the CDN. The
portable highlight query is exported at
`@mermanjs/tree-sitter-mermaid/queries/portable/highlights.scm`.

### Rust

```console
cargo add tree-sitter@0.26.12 tree-sitter-mermaid@0.1.0
```

```rust
let language: tree_sitter::Language = tree_sitter_mermaid::LANGUAGE.into();
let mut parser = tree_sitter::Parser::new();
parser.set_language(&language)?;

let tree = parser
    .parse("flowchart TD\nA --> B\n", None)
    .expect("Tree-sitter always returns a tree for a configured language");
assert!(!tree.root_node().has_error());

# Ok::<(), Box<dyn std::error::Error>>(())
```

This matches the package's existing `LanguageFn` surface
([Rust binding](../../distribution/tree-sitter-mermaid/bindings/rust/lib.rs)).

### C source install

```console
cmake -S . -B build -DBUILD_SHARED_LIBS=OFF
cmake --build build
cmake --install build --prefix /usr/local
pkg-config --cflags --libs tree-sitter-mermaid
```

The README should say that these commands run from an extracted grammar source/npm package root.
The committed parser and scanner do not require the Tree-sitter generator for this build.

## Publication-readiness actions

The README rewrite and npm rename should be accepted only when all of these are true:

1. `package.json`, lockfile identity, release scripts/workflows, package smoke, documentation, and
   legal projections agree on `@mermanjs/tree-sitter-mermaid` for npm.
2. Cargo, C, language symbol, WASM filename, repository directory, and GitHub tag retain the
   identities in the naming matrix.
3. `npm pack` installed into a clean CommonJS consumer passes the documented native example.
4. A clean Node ESM consumer passes the documented default-import example.
5. The packed WASM and portable highlight query resolve through the declared npm `exports`.
6. After the first protected npm publication, the exact-version jsDelivr WASM URL returns the
   expected bytes, `application/wasm`, and a parseable ABI-15 language.
7. The Tree-sitter community parser-list and editor migration work happens from the immutable GitHub
   release revision; it does not wait for or infer an unscoped npm alias.

## Source ledger

All sources were accessed on 2026-08-18.

- [Tree-sitter: getting started](https://tree-sitter.github.io/tree-sitter/creating-parsers/1-getting-started.html)
- [Tree-sitter: publishing grammars](https://tree-sitter.github.io/tree-sitter/creating-parsers/6-publishing.html)
- [Tree-sitter: synchronized binding versions](https://tree-sitter.github.io/tree-sitter/cli/version.html)
- [Tree-sitter: `tree-sitter init` and metadata](https://tree-sitter.github.io/tree-sitter/cli/init.html)
- [Tree-sitter: syntax highlighting](https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html)
- [Web Tree-sitter README](https://github.com/tree-sitter/tree-sitter/blob/master/lib/binding_web/README.md)
- [Tree-sitter community parser list](https://github.com/tree-sitter/tree-sitter/wiki/List-of-parsers)
- [`tree-sitter-javascript` README and package](https://github.com/tree-sitter/tree-sitter-javascript/tree/58404d8cf191d69f2674a8fd507bd5776f46cb11)
- [`tree-sitter-typescript` README and package](https://github.com/tree-sitter/tree-sitter-typescript/tree/75b3874edb2dc714fb1fd77a32013d0f8699989f)
- [`tree-sitter-rust` README and package](https://github.com/tree-sitter/tree-sitter-rust/tree/77a3747266f4d621d0757825e6b11edcbf991ca5)
- [`tree-sitter-grammars/template`](https://github.com/tree-sitter-grammars/template/tree/9c46d09d688d27c7aef31c2b32f50260de4e7906)
- [`tree-sitter-yaml`](https://github.com/tree-sitter-grammars/tree-sitter-yaml/tree/a1c4812a73ec5e089de8e441fdea3a921e8d5079)
- [`tree-sitter-markdown`](https://github.com/tree-sitter-grammars/tree-sitter-markdown/tree/a0a00f817d02412bd92c54d316f164d827b57b5c)
- [`monaqa/tree-sitter-mermaid`](https://github.com/monaqa/tree-sitter-mermaid/tree/90ae195b31933ceb9d079abfa8a3ad0a36fee4cc)
- [`pappasam/tree-sitter-mermaid`](https://github.com/pappasam/tree-sitter-mermaid/tree/1a11e2d8cf11afcfdb768f537c1a9bde294c24f9)
- [`singularity-ng/singularity-parser-mermaid`](https://github.com/singularity-ng/singularity-parser-mermaid/tree/f5ac2752fbf0f74f9c836014b87e511303d2abae)
- [npm: scopes](https://docs.npmjs.com/using-npm/scope.html)
- [npm: using scoped packages](https://docs.npmjs.com/using-npm-packages-in-your-projects/)
- [npm: publishing public scoped packages](https://docs.npmjs.com/creating-and-publishing-scoped-public-packages/)
- [Node.js: package exports](https://nodejs.org/api/packages.html#package-entry-points)
- [Node.js: CommonJS/ESM interoperability](https://nodejs.org/api/esm.html#interoperability-with-commonjs)
- [jsDelivr npm usage](https://github.com/jsdelivr/jsdelivr#usage-documentation)
- [jsDelivr public API specification](https://github.com/jsdelivr/data.jsdelivr.com/blob/master/src/public/v1/spec.yaml)
- [Current Merman grammar README](../../distribution/tree-sitter-mermaid/README.md)
- [Current Merman grammar npm manifest](../../distribution/tree-sitter-mermaid/package.json)
- [Current Merman grammar Cargo manifest](../../distribution/tree-sitter-mermaid/Cargo.toml)
- [Current Merman grammar metadata](../../distribution/tree-sitter-mermaid/tree-sitter.json)
- [Current Merman grammar release contract](../release/TREE_SITTER_MERMAID.md)
