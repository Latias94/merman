# Query migration and downstream compatibility

The `tree-sitter-mermaid` CST and query schemas are experimental before 1.0. They intentionally
prioritize one coherent, family-complete schema over compatibility with the older
`monaqa/tree-sitter-mermaid` node vocabulary. Downstream consumers should pin a package version,
compile their queries against that version, and review this document when upgrading.

## Fixed downstream matrix

`npm run test:downstream` exercises the following fixed consumers and contracts:

| Consumer | Fixed source or release | Blocking smoke |
| --- | --- | --- |
| Neovim | `v0.12.4` | Load the temporary native parser in real headless Neovim, load all nine declared query surfaces, and execute the 35-family applicability matrix. |
| Helix | `25.07.1` (`a05c151bb6e8e9c65ec390b0ae2afe7a5efd619b`) | Load the temporary native parser and exact runtime profile through `hx --health mermaid`, then compile and execute Helix's five supported query files against representative fixtures. |
| Zed | `1.16.0` source at `38ca9106c5306ef93e52c35643df015a27f15b72` | Validate the extension manifest and language configuration, the six query surfaces recognized by that Zed source, generated grammar ABI 14, committed WASM loading, and representative captures. |

The download URLs and SHA-256 digests for every supported Neovim and Helix host asset are recorded
in [`test/downstream/matrix.json`](../test/downstream/matrix.json). Downloads and extracted tools
are cached below the operating system's temporary directory. No editor binary is written to the
repository.

The fixed matrix is the blocking compatibility contract. A separate best-effort network probe
reports the current Neovim, Helix, and Zed release tags. Network failure or a newer release is a
warning, not a failure of the fixed matrix.

The checked-in Zed manifest must pin a Git revision whose generated parser matches the local
parser. During pre-commit grammar work, the smoke compares both parser SHA-256 values and reports
an explicit `pendingIntegration` warning when the worktree parser is dirty. After the grammar is
committed, update `grammars.mermaid.rev` to that grammar commit before committing the downstream
harness. A mismatch in a clean checkout is blocking.

### Host-specific query ownership

Portable captures live in `queries/portable`. Editor adapters live in `queries/neovim`,
`queries/helix`, and `queries/zed`; they are not interchangeable query ABIs.

- Neovim loads `portable/highlights.scm` before its highlights adapter. Its other eight query
  surfaces are complete Neovim-owned files.
- Helix 25.07.1 reads only highlights, injections, locals, indents, and textobjects. Folds, tags,
  brackets, and outline are intentionally absent rather than represented by empty files.
- Zed's fixed loader recognizes highlights, brackets, outline, indents, injections, and
  textobjects. Folds, locals, and tags are intentionally absent.

Helix 25.07.1 has no non-interactive command that opens a document and emits query captures.
`hx --health mermaid` is therefore the real-editor headless load check; the package-local
Tree-sitter CLI performs the additional compile-and-execute check over the exact files that were
installed into the temporary Helix runtime. Neovim's smoke executes the queries inside Neovim.
The Zed lane is a manifest/query/ABI configuration smoke and does not download or launch a Zed
application binary.

## Migrating from monaqa at `90ae195`

The downstream smoke downloads the historical highlights query from commit
`90ae195b31933ceb9d079abfa8a3ad0a36fee4cc` and verifies its SHA-256 digest
`095a2e34e4c1c170873bbb576e3d865369eacf21a6fe56036a53e7104c48704c` before replaying it.
Compilation against the current grammar must fail with `Invalid node type "sequenceDiagram"`.
This is an intentional schema break, not a compatibility regression. The current portable
highlights query must then compile and produce captures for the same representative Architecture
document.

Representative replacements include:

| Historical monaqa shape | Current query shape | Migration intent |
| --- | --- | --- |
| Literal `"sequenceDiagram"` | `(diagram_keyword) @keyword` inside `sequence_diagram` | Share a stable header token across public families while retaining a named family root. |
| `(pie_label)` / `(pie_value)` | `pie_section` fields containing `(langium_string)` and `(pie_number)` | Query the structured section and its typed value rather than grammar-specific leaves from the older eight-family grammar. |
| `(flow_vertex_id)` | `(flow_node_id)` | Use the current Flowchart declaration/reference vocabulary. |
| `er_cardinarity_*` nodes | `(er_cardinality)` | Replace the historical misspelling and cardinality-per-token schema with one coherent relationship operator node. |

Do not mechanically rename every old node. Start from the applicable portable or editor profile,
compile it against the pinned package, and add host-specific captures only where that editor owns
the behavior. The family root, fields, and current `node-types.json` are the authoritative CST
shape for that package version.

## Running the smoke

From `distribution/tree-sitter-mermaid`:

```text
npm run test:downstream
```

The command requires the package's installed Node dependencies, network access on the first run,
Git, and a system `tar` capable of extracting the fixed `.tar.gz`, `.tar.xz`, or `.zip` asset for
the host. Supported fixed assets cover macOS arm64/x64, Linux arm64/x64, and Windows x64. Neovim
also publishes a Windows arm64 asset; Helix 25.07.1 does not, so that host combination fails with
an explicit unsupported-asset error.
