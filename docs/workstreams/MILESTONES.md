# Workstreams Milestones (Alignment / Parity Hardening)

This document defines **pragmatic milestones** for Mermaid parity work. The intent is to ship
incrementally while keeping the main parity gates green.

Baseline upstream: Mermaid `@11.12.3`.

## Parity gates (definition of “green”)

- DOM parity baseline:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`
- Alignment lint/health checks:
  - `cargo run -p xtask -- check-alignment`

Optional (stress / not always required):

- Root viewport stress at higher precision:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 6`
- Strict XML stress for targeted fixtures:
  - `cargo run -p xtask -- compare-svg-xml --dom-mode strict --dom-decimals 3 ...`

## Milestone M0: Keep current gates green

Exit criteria:

- DOM parity (`--dom-mode parity`) stays green across the full corpus.
- No new unexplained root viewport drift at the current enforced precision (3dp where applicable).

## Milestone M1: Text measurement hardening (highest ROI)

Scope:

- HTML label wrapping boundaries (URLs, punctuation, long tokens).
- Whitespace/newline normalization and markdown token boundaries.
- Flowchart `parity-root` stabilization for text-driven node height changes (avoid viewport drift).

Exit criteria:

- Add a small, curated set of “text edge case” fixtures per sensitive diagram (flowchart, sequence, state).
- Any new mismatches are either fixed via model changes or explicitly tracked with justification.

## Milestone M2: `htmlLabels` semantic matrix coverage

Scope:

- Diagram-specific precedence rules:
  - global `htmlLabels` vs diagram overrides (flowchart in particular)
  - `wrappingWidth` applicability and edge-label defaults

Exit criteria:

- A toggle-matrix fixture set exists and is part of the committed corpus.
- Output label mode (SVG text vs `<foreignObject>`) matches upstream for each label category.

## Milestone M3: Theme/config precedence smoke suite

Scope:

- `themeVariables` vs config precedence for font and colors.
- Selector differences under `htmlLabels` toggles.
- `theme=default` behavior does not implicitly apply `base` defaults.

Exit criteria:

- A minimal “theme smoke” fixture set exists across 3–5 key diagrams.
- No unexpected structure drift; only expected style deltas appear.

## Milestone M4: Geometry and clipping hardening

Scope:

- Cluster boundary clipping, external edges, edge-label placement.
- Path data stability where upstream is deterministic.

Exit criteria:

- Add/confirm fixtures for cluster boundary cases.
- Triage workflow documented for geometry deltas (`debug-svg-data-points`, bbox helpers).

## Milestone M5: Diagram-specific sweeps (batch-driven)

Scope:

- Sequence, Gantt, Class, State, Mindmap “repeat offenders” from `docs/workstreams/TODO.md`.

Exit criteria:

- Each sweep ends with:
  - a small fixture batch import or promotion
  - green parity gates
  - a short note in `docs/alignment/STATUS.md` (optional but recommended)
