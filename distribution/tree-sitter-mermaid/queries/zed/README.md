# Zed query profile

This directory is a pre-1.0 Zed-specific adapter, not a portable Tree-sitter
query ABI. A downstream Zed extension must pin a released grammar revision and
validate these queries against the Zed version it supports.

Zed's loader at `crates/language_core/src/queries.rs` recognizes ten query
prefixes. Six of the nine project surfaces are native to that loader:

| Project surface | Zed query | Family coverage | Capture contract |
| --- | --- | ---: | --- |
| highlights | `highlights.scm` | 35/35 | Zed theme captures |
| brackets | `brackets.scm` | 25/35 | `open`, `close` |
| outline | `outline.scm` | 35/35 | required `item`, `name`; optional Zed outline captures |
| indents | `indents.scm` | 23/35 | required `indent`; optional `start`, `end`, `outdent`, `start.*` |
| injections | `injections.scm` | 35/35 | `injection.content` plus a fixed `injection.language` property |
| textobjects | `textobjects.scm` | 35/35 | Zed's function/class/comment Vim text objects |
| folds | none | 0/35 | Zed does not load `folds.scm` |
| locals | none | 0/35 | Zed does not load `locals.scm` |
| tags | none | 0/35 | Zed does not load `tags.scm` |

The highlight query preserves the portable query's semantic patterns while
mapping captures that are outside Zed's documented theme vocabulary:

| Portable capture | Zed capture |
| --- | --- |
| `comment.documentation` | `comment.doc` |
| `function.macro` | `function` |
| `keyword.operator` | `operator` |
| `namespace` | `type` |
| `variable.member` | `property` |

Bracket matching is not applicable to Cynefin, Git Graph, Info, Ishikawa,
Journey, Packet, Pie, Timeline, Tree View, or Treemap because their v1 CSTs do
not expose coherent source-owned delimiter pairs. Indentation is additionally
not applicable to Sankey or Venn. Ishikawa, Tree View, and Treemap already use
scanner-owned semantic indentation, while Venn only has inline label
delimiters.

All injection fixtures include Mermaid frontmatter, which Zed injects as YAML.
XY Chart also exposes delimiter-free Markdown payload nodes as
`markdown-inline`. Outline and text-object fixtures exercise every family;
bracket and indent fixtures exist only for applicable cells.

Merman's package tests compile every shipped query against the generated
language. Loader-specific behavior and representative captures belong in the
downstream Zed extension change rather than a duplicated package-owned 35 x 9
matrix.
