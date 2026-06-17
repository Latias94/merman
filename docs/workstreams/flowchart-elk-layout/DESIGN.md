# Flowchart ELK Layout - Design

Status: Active
Last updated: 2026-06-17

## Problem

`merman` now renders `flowchart-elk` and Flowchart `layout: elk` through the source-backed Mermaid
ELK adapter / Eclipse ELK layered port by default. The current upstream Mermaid ELK surface is
admitted to the Flowchart SVG parity matrix under the source-backed backend, while the dedicated
probe lane remains the focused regression gate for future upstream/user cases.

## Intent

Keep the default path source-backed and deterministic. Use Mermaid and Eclipse ELK source as the
specification, preserve the explicit compatibility fallback for alpha diagnostics, and keep ELK
admission centralized so `compat` is never mistaken for the mature parity path.

## Target State

- Public render entry points default to the source-backed Flowchart ELK backend.
- The dedicated ELK probe gate covers every upstream `flowchart-elk.spec.js` exact render call.
- Duplicate layout bodies are retained as exact-call fixtures for traceability.
- Admitted ELK fixtures participate in the default Flowchart parity matrix only under the
  source-backed backend.
- New ELK behavior is ported from Mermaid / Eclipse ELK source rather than fixture fitting.

## Scope

- `crates/merman-layout-elk`
- `crates/merman-render/src/flowchart/elk.rs`
- `crates/xtask/src/cmd/compare`
- `crates/xtask/src/cmd/import`
- `https://github.com/mermaid-js/mermaid/blob/develop/cypress/integration/rendering/flowchart/flowchart-elk.spec.js`

## Non-goals

- Do not fit ELK geometry from fixture output; port from source.
- Do not treat duplicate exact-call fixtures as evidence of additional unique layout semantics.
- Do not regress the non-ELK Flowchart lane while ELK is being expanded.

## ELK Fixture Tiers

| Tier | Upstream cases | Why it belongs here |
| --- | --- | --- |
| Tier A | `1-8`, `V2 elk - 16`, `1433`, `2388`, `2824`, `6647`, `7213` | Covered by admitted source-backed probes. |
| Tier B | `50-76`, `2050`, `58-65`, markdown string cases, `74` multi-edge labels, `6080-6088` | Covered by admitted source-backed probes. |
| Tier C | Future upstream/user cases outside the current Mermaid `flowchart-elk.spec.js` body set | Port from Mermaid / Eclipse ELK source and classify only after a targeted probe fails. |

## Fixture Admission Map

| Batch | Candidate fixtures | Expected work |
| --- | --- | --- |
| Default source-backed path | `LayoutOptions::default`, headless defaults, CLI, bindings | Source-backed backend is selected by default; `compat` remains explicit fallback. |
| Dedicated probe gate | 63 admitted fixtures from the ELK spec plus the HTML demo | `cargo run -p xtask -- check-flowchart-elk-source-backed-probes` must stay green. |
| Coverage audit | 63 upstream exact calls / 57 unique layout bodies | `cargo run -p xtask -- audit-flowchart-elk-source-backed-coverage` tracks exact-call and unique-body coverage. |
| Broad matrix policy | Flowchart `compare-all-svgs` default path | Source-backed ELK probes are admitted by default; explicit `compat` runs remain outside parity admission. |

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| The current Mermaid ELK exact call set is covered by source-backed probes. | High | `check-flowchart-elk-source-backed-probes` and `audit-flowchart-elk-source-backed-coverage`. | Treat failures as regressions or newly discovered source-port gaps. |
| Duplicate layout bodies are represented as exact-call fixtures. | High | Coverage audit reports 63 dedicated fixtures / 57 unique layout bodies. | Use the unique-body count for semantic progress, and the exact-call count for upstream traceability. |
| Future hardening must remain source-backed. | High | Project parity policy and the current ELK port history. | Reject heuristic tuning that only makes one fixture pass. |

## Architecture Direction

Prefer explicit, source-backed growth:

1. carry Flowchart direction and label data through the Mermaid adapter boundary;
2. keep the compatibility fallback explicit and out of the default path;
3. keep targeted probes as the focused regression gate after broad matrix admission;
4. port missing semantics from Mermaid / Eclipse ELK source.

## Closeout Condition

This lane can close when:

- the dedicated probe gate and coverage audit stay green,
- broad main-matrix admission stays source-backed and green,
- duplicate exact-call fixtures remain traceable without being mistaken for unique layout gaps,
- and future ELK regressions have source-backed diagnostics.
