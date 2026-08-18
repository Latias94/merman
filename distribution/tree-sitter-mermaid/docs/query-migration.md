# Query migration and downstream compatibility

The `tree-sitter-mermaid` CST and canonical query captures are experimental before 1.0. Consumers
must pin a released version or immutable repository commit and compile their queries against that
exact generated parser.

## Query ownership

- `queries/portable` is the canonical package query set referenced by `tree-sitter.json`.
- `queries/neovim`, `queries/helix`, and `queries/zed` are pre-1.0 adoption assets for those hosts.
- downstream editors own their installed query copies and may intentionally diverge from the
  portable capture vocabulary.

The repository compiles every shipped query and keeps one compact canonical highlight contract:
non-header capture classes for all 35 public families plus focused exact-span, recovery, and
incremental-parse cases. It does not maintain a fixed-editor download matrix, 35-by-surface
applicability table, or exact capture forest. Real downstream adoption is verified in the
downstream pull request against an immutable grammar release.

## Migrating from monaqa/tree-sitter-mermaid

This grammar intentionally does not preserve the older eight-family node vocabulary. Representative
changes include:

| Historical shape | Current shape | Migration intent |
| --- | --- | --- |
| literal `"sequenceDiagram"` | `(diagram_keyword) @keyword` inside `sequence_diagram` | Share a stable header token while retaining a named family root. |
| `(pie_label)` / `(pie_value)` | `pie_section` fields containing `langium_string` and `pie_number` | Query the structured section and typed value. |
| `(flow_vertex_id)` | `(flow_node_id)` | Use the current declaration/reference vocabulary. |
| misspelled `er_cardinarity_*` nodes | `(er_cardinality)` | Use one coherent relationship operator node. |

Do not mechanically rename every historical node. Start from the current portable query, inspect
`src/node-types.json`, and add host-specific captures only where the editor owns that behavior.

## Editor source pins

The grammar lives in a monorepo but all three target editors support a subdirectory:

- nvim-treesitter: repository URL plus `location = "distribution/tree-sitter-mermaid"`;
- Helix: repository URL, release revision, and `subpath = "distribution/tree-sitter-mermaid"`;
- Zed extension: repository URL, release revision, and `path = "distribution/tree-sitter-mermaid"`.

Publishing to npm or crates.io does not update those pins. Open downstream changes only after the
release commit contains the final generated parser and query files.

## Compatibility policy

Before 1.0, named-node/field removals, canonical capture removals, ABI changes, and Mermaid baseline
changes require a minor release. Compatible fixes and additive captures may use a patch release.
Every release notes its Tree-sitter ABI and selected Mermaid/ZenUML baselines.
