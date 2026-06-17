# Flowchart ELK Layout - Design

Status: Active
Last updated: 2026-06-17

## Problem

`merman` now renders `flowchart-elk` and Flowchart `layout: elk` through the source-backed Mermaid
ELK adapter / Eclipse ELK layered port by default. The upstream Mermaid ELK surface still needs a
separate lane because broad Flowchart main-matrix admission, exact duplicate-call fixture import,
and future upstream/user cases should be handled without weakening the non-ELK Flowchart parity
gate.

## Intent

Keep the default path source-backed and deterministic. Use Mermaid and Eclipse ELK source as the
specification, preserve the explicit compatibility fallback for alpha diagnostics, and treat broad
main-matrix admission as a separate policy decision from the dedicated ELK probe gate.

## Target State

- Public render entry points default to the source-backed Flowchart ELK backend.
- The dedicated ELK probe gate covers every unique upstream `flowchart-elk.spec.js` layout body.
- Exact duplicate-call fixture gaps are tracked separately from unique layout gaps.
- ELK fixture probes run explicitly without weakening the default non-ELK Flowchart parity matrix.
- New ELK behavior is ported from Mermaid / Eclipse ELK source rather than fixture fitting.

## Scope

- `crates/merman-layout-elk`
- `crates/merman-render/src/flowchart/elk.rs`
- `crates/xtask/src/cmd/compare`
- `crates/xtask/src/cmd/import`
- `https://github.com/mermaid-js/mermaid/blob/develop/cypress/integration/rendering/flowchart/flowchart-elk.spec.js`

## Non-goals

- Do not fit ELK geometry from fixture output; port from source.
- Do not treat duplicate exact-call fixture gaps as unique layout gaps.
- Do not regress the non-ELK Flowchart lane while ELK is being expanded.

## ELK Fixture Tiers

| Tier | Upstream cases | Why it belongs here |
| --- | --- | --- |
| Tier A | `1-8`, `V2 elk - 16`, `1433`, `2388`, `2824`, `6647`, `7213` | Covered by admitted source-backed probes or duplicate layout bodies. |
| Tier B | `50-76`, `2050`, `58-65`, markdown string cases, `74` multi-edge labels, `6080-6088` | Covered by admitted source-backed probes or duplicate layout bodies. |
| Tier C | Future upstream/user cases outside the current Mermaid `flowchart-elk.spec.js` body set | Port from Mermaid / Eclipse ELK source and classify only after a targeted probe fails. |

## Fixture Admission Map

| Batch | Candidate fixtures | Expected work |
| --- | --- | --- |
| Default source-backed path | `LayoutOptions::default`, headless defaults, CLI, bindings | Source-backed backend is selected by default; `compat` remains explicit fallback. |
| Dedicated probe gate | 57 admitted fixtures from the ELK spec plus the HTML demo | `cargo run -p xtask -- check-flowchart-elk-source-backed-probes` must stay green. |
| Coverage audit | 63 upstream exact calls / 57 unique layout bodies | `cargo run -p xtask -- audit-flowchart-elk-source-backed-coverage` tracks duplicate-body gaps. |
| Broad matrix policy | Flowchart `compare-all-svgs` default path | Decide separately when ELK probe fixtures should move into the broad main matrix. |

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| The current Mermaid ELK spec body set is covered by source-backed probes. | High | `check-flowchart-elk-source-backed-probes` and `audit-flowchart-elk-source-backed-coverage`. | Treat failures as regressions or newly discovered source-port gaps. |
| The six uncovered exact calls are duplicate layout bodies, not unique layout gaps. | High | Coverage audit maps each to an admitted representative. | Import duplicate fixtures only if exact-call traceability becomes worth the corpus noise. |
| Future hardening must remain source-backed. | High | Project parity policy and the current ELK port history. | Reject heuristic tuning that only makes one fixture pass. |

## Architecture Direction

Prefer explicit, source-backed growth:

1. carry Flowchart direction and label data through the Mermaid adapter boundary;
2. keep the compatibility fallback explicit and out of the default path;
3. use targeted probes before broad main-matrix admission;
4. port missing semantics from Mermaid / Eclipse ELK source.

## Closeout Condition

This lane can close when:

- the dedicated probe gate and coverage audit stay green,
- broad main-matrix admission policy is decided,
- duplicate exact-call fixture gaps are either intentionally left as duplicate-covered or imported,
- and future ELK regressions have source-backed diagnostics.
