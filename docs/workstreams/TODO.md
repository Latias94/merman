# Workstreams TODO (Alignment / Parity Hardening)

This TODO list is a **work-in-progress execution plan** for closing the most common Mermaid parity
gaps. Each item includes a lightweight “gap check” so we don’t fix problems we don’t have.

Baseline upstream: Mermaid `@11.12.3`.

## How to use this file

For each item:

1. Prove the gap exists (or mark it as “Already covered” with evidence).
2. If the gap exists, add the smallest reproducer fixture.
3. Fix in the model (preferred) or via a scoped override (when justified).
4. Add regression coverage.

## Global gap checks (run first)

- [x] Confirm current parity gates are green:
  - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`
  - `cargo run -p xtask -- check-alignment`
- [x] Inventory override footprint (to avoid unbounded growth):
  - `cargo run -p xtask -- report-overrides`

## A) Text measurement & wrapping

- [x] URL / punctuation line breaking in HTML labels  
  Gap check:
  - Search for failing strict-XML cases in local stress fixtures (if any).
  - Add a minimal flowchart fixture with a URL in parentheses at `wrappingWidth=200`.
  Evidence:
  - Compare: `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-decimals 3`
  - Strict stress (optional): `compare-svg-xml --dom-mode strict --dom-decimals 3`
  Notes:
  - Fixture: `fixtures/flowchart/stress_flowchart_html_label_url_punct_wrap_067.mmd`

- [ ] Mixed CJK + ASCII + emoji measurement stability  
  Gap check:
  - Look for existing fixtures: `rg -n "中文|漢字|emoji|😀" fixtures/ docs/alignment -S`
  - If present, verify parity-root stability at `--dom-decimals 3`.

- [ ] Whitespace corner cases (`&nbsp;`, multiple spaces, trailing spaces)  
  Gap check:
  - Find tests: `rg -n "nbsp|multiple spaces|trailing" crates/merman-render/src/text -S`
  - Add a fixture that includes repeated spaces and verify DOM parity does not regress.

- [ ] Newline normalization (`\\n` literal vs newline vs `<br>` variants)  
  Gap check:
  - Locate existing coverage: `rg -n "replace_br_variants|<br" crates/merman-render/src/text.rs`
  - Ensure both SVG labels and HTML labels handle `<br/>` consistently.

## B) `htmlLabels` semantics and config precedence

- [ ] Flowchart label-mode precedence matrix  
  Gap check:
  - Build a small fixture matrix:
    - global `htmlLabels` (T/F)
    - `flowchart.htmlLabels` (unset/T/F)
    - node vs edge vs subgraph titles
  Evidence:
  - DOM parity compare passes and emitted label modes match upstream.

- [ ] `wrappingWidth` applies to the right label categories  
  Gap check:
  - Ensure node HTML label max-width tracks `flowchart.wrappingWidth`.
  - Ensure edge labels remain capped at 200px unless upstream says otherwise.

- [ ] Font-size precedence rules per diagram  
  Gap check:
  - Search docs: `docs/alignment/*MINIMUM.md` and per-diagram render modules for `fontSize`.
  - Confirm whether each diagram reads from `themeVariables.fontSize`, top-level `fontSize`, or a diagram override.
  Evidence:
  - A “fontSize smoke fixture” per diagram with `init` directives.

## C) Markdown subset parity

- [ ] `_` delimiter correctness (`a__b`, `_a_b_`, `_a__b_`)  
  Gap check:
  - Confirm existing tests cover underscore-heavy ids and labels:
    - `docs/alignment/FLOWCHART_*`, `crates/merman-render/src/text/tests.rs`
  Evidence:
  - Add a minimal fixture for each edge case if missing.

- [ ] Inline code suppresses emphasis parsing  
  Gap check:
  - Ensure `` `**not bold**` `` remains literal in both SVG-label and HTML-label modes.

- [ ] Escaped entities survive markdown→HTML→SVG pipeline  
  Gap check:
  - Add a fixture containing `&lt;`, `&amp;`, and unknown `&entity;` sequences.
  Evidence:
  - Rendered SVG must remain valid XML and match upstream escaping behavior.

## D) Theme & CSS selector drift

- [ ] Verify `theme=default` vs `base` does not cause implicit defaults  
  Gap check:
  - Add a small xychart + flowchart fixture with explicit colors and ensure they stay stable.

- [ ] Inline `classDef` / `style` overrides: font-family/font-size/opacity  
  Gap check:
  - Find existing fixtures with classDef style usage in `fixtures/upstream-svgs/*`.
  - Confirm HTML-label div style mirrors upstream (only for properties upstream emits).

## E) SVG DOM stability and IDs

- [ ] Stable element ordering (nodes/edges/clusters)  
  Gap check:
  - If strict diffs are noisy, verify parity mode is stable.
  - Only chase ordering issues when upstream ordering is deterministic and important.

- [ ] Marker/id/url references are stable and correctly escaped  
  Gap check:
  - Ensure `marker-end="url(#...)"` ids don’t change run-to-run.
  - Add a fixture with multiple diagrams on the same page if the renderer supports it.

## F) Subgraphs, clipping, edge geometry

- [ ] Boundary clipping for cluster edges  
  Gap check:
  - Use existing stress fixtures for external edges; confirm no DOM/viewport drift.

- [ ] Edge labels near cluster titles  
  Gap check:
  - Validate label placement does not overlap titles in parity-root mode.

## G) Diagram-specific hardening passes

- [ ] Sequence: note wrapping / activation stacking / message font precedence  
  Gap check:
  - Review `docs/alignment/SEQUENCE_*` and add a small “stacking matrix” fixture if missing.

- [ ] Gantt: date parsing/timezone + “today” determinism  
  Gap check:
  - Confirm no fixtures depend on real “now” unless frozen.
  - Add one deterministic “today marker” fixture if upstream has one.

- [ ] Class: generics + namespace/member layout  
  Gap check:
  - Ensure `<`/`>` escaping and multiline members match upstream.

- [ ] State: composite padding + classDef html label measurement  
  Gap check:
  - Confirm classDef-driven HTML label metrics don’t drift in parity-root mode.

- [ ] Mindmap: multiline CJK + deep nesting viewport drift  
  Gap check:
  - Ensure root viewport bounds include all labels; compare in parity-root mode.
