# Mermaid 12.2 Alignment

## Scope and authority

Align the Rust headless implementation and its selected browser reference graph from Mermaid
12.1.0 (`21f72f07ea22c0af48a3149c550654e80d8e40cb`) to 12.2.0
(`1fb7869866dd619168d840291c9c54b4f129758b`, tag `mermaid@12.2.0`).
The trusted Merman base is `3a4b8c393939b69372a8f05a2b971f7cdf6ae529`.
The initial working tree was clean. Implementation is on `feat/mermaid-12-2-alignment`.
No push, PR, publication, or release is requested. Historical release records and the independent
Tree-sitter package retain their original baselines.

## Source delta

- Mindmap gains `colorBy: depth` with an eleven-color non-root cycle; branch remains the default.
  SVG labels are centered and Neo HTML labels use the corresponding palette text color.
- Circle and double-circle ignore diagram minimum label width; Neo circle radius uses one 16px
  padding step.
- ELK terminal straightening rejects an increased count of edge/label overlaps as well as crossings.
- Swimlane clusters receive diagram-scoped DOM IDs; clipping follows the actual layout engine and
  respects skip-intersection. Disconnected nodes retain source order; terminal lanes are split
  after crossing cleanup; title bands account for inherited direction and border rails.
- KaTeX and Chevrotain dependency ranges change. Built-in registrations, parser companion version,
  and external companion versions do not change. The upstream E2E corpus is reorganized; historical
  fixture provenance must not be relabeled.

## Lane map

Mode: `execute_direct`. Workspace: repository root. Verification owner: root agent.
Cargo builds/tests use the existing target directory. The maintainer explicitly authorized full CPU
utilization on 2026-10-13; the root may use parallel build and test jobs. Workers do not commit.
All workers preserve other edits and may only write their assigned paths. AGENTS.md, skills,
reference descriptors, package locks, and historical records are forbidden to workers.

| Lane | Ownership | Expected output |
| --- | --- | --- |
| Root | Reference descriptors, dependency workspaces, generators, docs, shared configuration, baseline refresh and integration | Verified selected graph, accurate provenance, final checks |
| Mindmap | `crates/merman-core/src/diagrams/mindmap/`, `crates/merman-render/src/mindmap*`, `crates/merman-render/src/svg/parity/mindmap*`, dedicated mindmap tests | Structural depth colors, label positioning/CSS, source-backed regressions |
| Swimlane | `crates/merman-render/src/swimlane/`, `crates/merman-render/src/svg/parity/flowchart/swimlane/`, dedicated swimlane tests | Source order, geometry cleanup, scoped cluster IDs and clipping audit |
| Circle/ELK | `crates/merman-render/src/flowchart.rs`, `crates/merman-render/src/flowchart/`, `crates/merman-render/src/elk_terminal_jogs.rs`, dedicated circle/ELK tests | Label-derived circle sizing and label-aware terminal straightening |

Report shared-path needs before editing outside ownership. Stop a lane on ownership conflicts or
unexplained source behavior; root resolves dependencies before admission. No semantic comparator
relaxations or fixture-specific geometry overrides.

## Verification

Run focused source-backed Rust regressions first, then reference verification (including materialized
and trusted-base forms), generated configuration/theme/catalog checks, LALRPOP freshness, workspace
nextest, formatting, clippy, alignment, and strict release gates. Refresh reference SVGs by actual
rendering through the existing generator. Record exact diagnostic/artifact residuals only after review.
Use memory shards for execution evidence and handoff; this document records scope and decisions.

## Completion

Implementation and local verification completed on 2026-10-13. The selected graph, generated
configuration/theme/catalog, actual browser baselines, family behavior and host integration agree
on Mermaid 12.2.0. The full strict command passes; the final-source verifier also passes the
materialized/base-bound reference, generated and 37-family three-mode DOM checks.

The maintainer's fixture-sufficiency review adds eleven primary inputs in total for this transition
and two controlled family-local browser references. The primary matrix now has 3,755 inputs,
3,752 layout goldens and 3,731 actual upstream SVGs. The focused review, remaining combinations
and artifact boundaries are recorded in [the coverage review](../alignment/FIXTURE_COVERAGE_12_2.md)
and [ADR-0093](../adr/0093-mermaid-12-2-upstream-transition.md).
