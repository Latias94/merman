# Alignment Status

This is the human-readable compatibility dashboard. It describes selected sources, admission, and
verification boundaries; it does not record implementation progress or certify an untested revision.
The [authority map](README.md#machine-authorities) identifies the structured owners.

## Selected Baseline

| Concern | Current boundary |
| --- | --- |
| Development reference | Mermaid `@12.3.0`, source `2086904e76c692331e34a45d7cc3bc30860d040e` |
| Published artifacts | Retain the source baseline and capabilities of their own release |
| Primary SVG matrix | 37 families in the executable compare facts, with selected 12.3 upstream manifests |
| External comparison | ZenUML uses its selected companion graph and a separate browser admission lane |
| Root viewport | Every primary family participates in root verification; deterministic contracts and browser diagnostics have distinct owners |
| Semantic edge labels | C4, Flowchart ELK, Architecture, Requirement, State, Class, and ER have fail-closed identity/geometry/presentation admission |
| Language/editor support | Family capabilities are independent of optional render backends; tolerant Tree-sitter syntax has its own selected baseline |
| Verification | A successful gate applies to the exact source, artifacts, environment, and revision that ran it |

The [12.3 migration](../migrations/mermaid-12-3.md) describes source behavior and dependency
selection. The source lock, reference bundle, and selection receipt bind that graph. Historical
12.0/12.2 test counts, capture environments, and release results do not qualify 12.3 or later
uncommitted changes.

## Reading Admission

Primary SVG admission means the family has the required semantic/layout evidence, selected-runtime
SVG provenance, and an executable comparison policy. It does not imply Chromium pixel identity,
every upstream syntax combination, or every output surface. Exact family membership, commands,
default DOM modes, tooling skips, and per-input exceptions live in
[compare facts](../../crates/xtask/src/cmd/compare/diagrams.rs),
[family capabilities](../../crates/merman-core/src/family.rs), and the per-family
[baseline manifests](../../fixtures/upstream-svgs/). The
[admission guide](ADMISSION_INVENTORY.md) explains how these owners are checked together.

Parser-only inputs, raw upstream corpora, deferred fixtures, and controlled family-local experiments
are evidence with their own scope. They are not automatically part of the primary matrix. In
particular, Swimlane's retained DDLT corpus exercises layout independently of ordinary snapshot
admission. Consult manifests and compare policy instead of deriving exclusions from file names.

## Primary SVG Matrix

The [release-facts collector](../../scripts/collect_release_facts.py) falls back to parsing this
section when its historical admission-inventory parser cannot find records. Preserve this heading,
the following `Non-Primary Families` heading, and the first-column family identifiers. The
[compare facts](../../crates/xtask/src/cmd/compare/diagrams.rs) remain authoritative.

Every row below is semantic-, layout-, and SVG-covered in the admission inventory. `N` means a
normalized fixture corpus; `N+D` means normalized fixtures plus an explicitly deferred
investigation corpus. The default DOM mode is the family command's configured comparison boundary,
not a quality ranking. Callers may select another supported mode explicitly.

| Family | Corpus | Compare command | Default DOM mode |
| --- | ---: | --- | --- |
| `agentflow` | N | `compare-agentflow-svgs` | `parity` |
| `usecase` | N | `compare-usecase-svgs` | `parity` |
| `er` | N+D | `compare-er-svgs` | `parity` |
| `flowchart` | N+D | `compare-flowchart-svgs` | `parity` |
| `state` | N+D | `compare-state-svgs` | `structure` |
| `class` | N+D | `compare-class-svgs` | `parity` |
| `sequence` | N+D | `compare-sequence-svgs` | `structure` |
| `info` | N+D | `compare-info-svgs` | `parity` |
| `pie` | N+D | `compare-pie-svgs` | `structure` |
| `sankey` | N+D | `compare-sankey-svgs` | `parity-root` |
| `packet` | N+D | `compare-packet-svgs` | `structure` |
| `timeline` | N | `compare-timeline-svgs` | `structure` |
| `journey` | N | `compare-journey-svgs` | `parity` |
| `kanban` | N | `compare-kanban-svgs` | `structure` |
| `gitgraph` | N+D | `compare-gitgraph-svgs` | `parity` |
| `gantt` | N | `compare-gantt-svgs` | `structure` |
| `c4` | N+D | `compare-c4-svgs` | `parity` |
| `block` | N+D | `compare-block-svgs` | `structure` |
| `radar` | N+D | `compare-radar-svgs` | `parity` |
| `requirement` | N+D | `compare-requirement-svgs` | `parity` |
| `mindmap` | N | `compare-mindmap-svgs` | `parity` |
| `architecture` | N+D | `compare-architecture-svgs` | `parity` |
| `quadrantchart` | N | `compare-quadrantchart-svgs` | `parity` |
| `treemap` | N+D | `compare-treemap-svgs` | `parity` |
| `xychart` | N+D | `compare-xychart-svgs` | `parity` |
| `treeView` | N | `compare-tree-view-svgs` | `parity` |
| `ishikawa` | N+D | `compare-ishikawa-svgs` | `parity` |
| `eventmodeling` | N | `compare-eventmodeling-svgs` | `parity` |
| `error` | N | `compare-error-svgs` | `parity` |
| `venn` | N | `compare-venn-svgs` | `parity` |
| `swimlane` | N | `compare-swimlane-svgs` | `parity` |
| `railroad` | N | `compare-railroad-svgs` | `parity` |
| `railroadEbnf` | N | `compare-railroad-ebnf-svgs` | `parity` |
| `railroadAbnf` | N | `compare-railroad-abnf-svgs` | `parity` |
| `railroadPeg` | N | `compare-railroad-peg-svgs` | `parity` |
| `wardley` | N | `compare-wardley-svgs` | `parity` |
| `cynefin` | N | `compare-cynefin-svgs` | `parity` |

## Non-Primary Families

| Family | State | Evidence |
| --- | --- | --- |
| `zenuml` | External comparison lane | [Family notes and browser admission owners](FAMILY_NOTES.md#zenuml) |

ZenUML has family-owned parsing, semantic/editor facts, typed layout, and native SVG. Its external
plugin graph is checked in an opaque browser realm; its admission does not create built-in Mermaid
upstream SVG baselines. See [ZenUML notes](FAMILY_NOTES.md#zenuml).

## Compatibility Guides

| Boundary | Maintained guide |
| --- | --- |
| Semantic/structural parity and accepted browser differences | [Parity policy](PARITY_POLICY.md) |
| Family-specific limitations and controlled evidence | [Family notes](FAMILY_NOTES.md) |
| Exact edge-label identity, presentation, and geometry | [Semantic-label contract](SEMANTIC_LABEL_PARITY.md) |
| Config acceptance, precedence, security, and rendered evidence | [Config and frontmatter](CONFIG_FRONTMATTER_SUPPORT.md) |
| `merman-cli mmdc` arguments and export behavior | [CLI compatibility](CLI_COMPATIBILITY.md) |
| Source provenance, external comparisons, and attribution | [External sources](EXTERNAL_FIXTURES.md) |
| Consumer SVG, HTML fallback, fonts, and encoded output | [SVG output pipeline](../rendering/SVG_OUTPUT_PIPELINE.md) and [raster output](../rendering/RASTER_OUTPUT.md) |

The command-line compatibility surface remains `mmdc@11.16.0`, independently of the selected
Mermaid reference runtime and reference CLI. Native rendering uses `render` and `batch`; the
repository does not install an `mmdc` executable alias.

## Verification Entry Points

Run the smallest check that proves a change, then widen according to its affected owners:

```sh
# Capability, fixture, golden, manifest, and compare-owner consistency.
cargo run -p xtask -- check-alignment

# A family, using its registered command and comparison policy.
cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-decimals 3

# Repository verification; strict adds release-strength checks.
cargo run -p xtask -- verify
cargo run -p xtask -- verify --strict
```

Use the [upstream baseline guide](../rendering/UPSTREAM_SVG_BASELINES.md) for regeneration and the
[upgrade playbook](../release/MERMAID_UPGRADE_PLAYBOOK.md) for source admission. Exact residual
catalogs reject changed or stale evidence; a documented browser boundary does not waive a failing
gate. Current gate output, rather than this dashboard, establishes the result for a reviewed
revision.
