# Post-merge documentation and preflight audit fixes

Date: 2026-09-15. Reviewed baseline: `b7246bbe4192fc8c1a14c10f8103d7050327dfd1`,
which contains the audited `a6b9f9ae5` runtime changes. The runtime repairs are
`92bcdf026` (shared Markdown) and `ff0cd48e6` (rustdoc attributes/helpers). This record describes local repair
verification, not a full workspace/platform run or C7a freeze.

## Findings and disposition

1. **Host npm preflight:** Node setup did not install the npm 12 metadata owner. All six host
   Node setup steps now immediately install pinned npm 12.0.2; matrix conditions match setup.
   GNU/musl baseline containers keep their explicit npm 11 adapter. Commit `19ada22e1` contains
   the workflow and ordering/condition regression check.
2. **Retained Markdown prefixes:** parent containers now retain source offsets. Only actual
   Mermaid/include replacement blocks construct the innermost continuation prefix. Ordinary
   prose and non-Mermaid code blocks do not allocate those replacement prefixes.
3. **Repeated negative include scans:** candidate lines are remembered after a negative match,
   and the literal directive prefix is checked before parsing trailing whitespace. Source
   ranges, lazy-container rejection and parser-owned block recognition remain intact.
4. **Conditional doc order:** literal groups stop at `cfg_attr` as well as dynamic `doc`
   attributes. Rust still evaluates all conditions, including nested conditions; no configuration
   evaluator was added. A real rustdoc regression checks active, inactive and nested conditions.
5. **Facade re-exports:** the new string option `crate_path = "::facade"` locates a facade's
   re-exported `__render_doc` helper. The facade exposes both macros, and the application does not
   need a direct proc-macro dependency. The option is invocation-local. A real rustc-built facade
   and rustdoc application test exercises plain docs and a descendant Mermaid diagram; renamed
   direct dependencies retain their existing regression coverage.

The shared Markdown ADR is now ADR-0093 (renumbered after main assigned ADR-0089 to ELK defaults); its predecessor is the existing ADR-0087 dual-product
boundary. The related ADR-0076 references were corrected. The headless plan now names
`ThemeDefinitionV1` / `CompiledDiagramTheme` and `custom_diagram_theme.rs`. It does not authorize
restoring HostTheme or the removed presentation compatibility layer.

## Resource measurements

This is a structural resource repair, not a general latency optimization claim. Let D be quote
nesting depth, I the number of inline text events in a line, and T its trailing whitespace length.
The repaired adapter previously retained O(D²) prefix bytes and rescanned an O(T) suffix across
O(I) negative candidates. Parent state is now O(D); replacement prefixes are materialized only
when needed, and each eligible line is considered once before suffix parsing. These bounds cover
the adapter terms, not an independent bound on every pulldown-cmark operation.

A local Release probe compiled immutable baseline and candidate module copies into one executable
with Rust 1.95.0, pulldown-cmark 0.13.4 and serde_json 1.0.150 on macOS ARM64. It used one warmup and
four alternating baseline/candidate repetitions per scale. Seven plain/include/container controls
had identical complete block debug representations. Host timing remains diagnostic; the primary
measurement is total allocated bytes, including reallocations, rather than RSS or peak live heap.

| Quote depth | Source bytes | Baseline allocated bytes | Candidate allocated bytes |
| --- | ---: | ---: | ---: |
| 1,000 | 2,012 | 1,234,268 | 216,904 |
| 2,000 | 4,012 | 4,464,644 | 429,896 |
| 4,000 | 8,012 | 17,294,324 | 1,224,808 |

| Inline prose input bytes | Baseline median ms | Candidate median ms |
| ---: | ---: | ---: |
| 14,003 | 5.587 | 0.093 |
| 28,003 | 21.899 | 0.181 |
| 56,003 | 88.249 | 0.360 |

All adversarial inputs returned zero diagram blocks. The permanent allocation regression checks
three depths, a generous per-input cap and less than 3x allocation growth per doubling. Timing is
printed only as a diagnostic and never used as a flaky CI threshold. The experiment's immutable
sources, hashes, toolchain, manifest, lockfile, raw CSV and run details remain under
`target/bench/experiments/markdown-resource-amplification/`; the probe is not a new product script
or performance CI lane.

## Validation

- `merman-doc` and `merman-rustdoc`: 51/51 tests, including the new allocation, conditional-order
  and facade-consumer cases, existing inheritance, security, renamed-dependency and SVG cases.
- CLI `markdown_scanner`, `markdown_cli` and `rustdoc_cli`: 44/44 tests.
- Workflow contracts: 88/88 tests across CI planning, release workflow security, Android owner
  and fuzz configuration; actionlint passed for the changed preflight workflow.
- Actual npm 11.18.0 packaging reproduced two metadata failures. With npm 12.0.2 installed only
  in a temporary directory, package/command contracts passed 10/10 using the same host Node
  26.6.0. This is an npm-format repair witness, not a rebuild of the pinned host Node 24.21.0 lane.
- Changelog contracts: 9/9; release preparation verification passed. Published alpha.6-and-earlier
  history still exactly matches the merged main record. New macro options and fixes are Unreleased.
- ADR links and unique ADR identities passed a local check.
- Formatting and `git diff --check` passed. Scoped Clippy completed successfully for both doc
  crates and all their targets. No diagnostic points into the changed doc crates; dependencies
  still emit existing feature-dependent dead-code warnings (80 in `merman-render`, 1 in `merman`).
  Those warnings are not a claim that the workspace has been cleaned or passed with warnings denied.

No platform artifact was rebuilt for these source fixes. The separate Linux Node installed-package
record identifies its older source explicitly. A final candidate still needs the C7a owner
preflight/profile matrix and formal version/contract freeze.
