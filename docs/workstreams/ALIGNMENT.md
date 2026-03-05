# Alignment Workstream (Mermaid Parity)

This folder tracks the **ongoing alignment workstream**: what to align next, how to prove we have
a real gap, and how we decide between *model changes* vs *fixture-derived overrides*.

Baseline target (pinned upstream): Mermaid `@11.12.3`.

Related documentation:

- Diagram-specific coverage and gap notes: `docs/alignment/`
- SVG compare tooling: `docs/rendering/COMPARE_ALL_SVGS.md`, `docs/rendering/SVG_CANONICAL_XML.md`
- Current status / long-term plan: `docs/alignment/STATUS.md`, `docs/alignment/PARITY_HARDENING_PLAN.md`

## What “aligned” means

We use multiple parity levels depending on risk and cost:

1. **DOM parity** (`--dom-mode parity`)  
   Diagram subtree structure matches upstream (classes, node ordering, attributes in scope).

2. **Root viewport parity** (`--dom-mode parity-root`)  
   `viewBox` and root `style="max-width: ...px"` match upstream at a stable precision target.

3. **Strict canonical XML parity** (`--dom-mode strict`)  
   A stress test that surfaces 1/64px lattice drift and serialization differences. This is
   useful for tightening layout math but is *not* automatically “must-fix” for all fixtures.

## How to check whether we have a real gap

Before “fixing”, validate the gap exists and is in-scope:

1. **Reproduce with the existing gates**
+   - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`
   - For a single diagram: `cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-decimals 3`

2. **Locate upstream coverage**
   - Check `docs/alignment/*_UPSTREAM_TEST_COVERAGE.md` for relevant upstream sources/fixtures.
   - If it is not in the committed fixture corpus, decide whether to:
     - import it (preferred for long-term confidence), or
     - keep it as a local “extra” stress fixture (fast iteration, not gated).

3. **Classify the delta**
   - **Structure / selector drift**: wrong DOM structure, wrong CSS selector path, wrong class.
   - **Measurement drift**: widths/heights/translate differ; usually text measurement or bbox.
   - **Serialization drift**: attribute formatting/precision; fix in canonicalization or quantize.
   - **Known upstream float noise**: consider fixture-derived overrides (viewport / text/bbox).

4. **Minimize**
   Reduce to the smallest `.mmd` that reproduces the mismatch. Prefer one diagram + 1–2 nodes.

## Decision guide: model change vs override

Prefer **model changes** when:

- The delta appears across many fixtures or diagram types.
- Behavior is driven by spec/config semantics (e.g. `htmlLabels` precedence, theme variable rules).
- Fix improves determinism and reduces future override growth.

Prefer **fixture-derived overrides** when:

- The delta is 1/64px-level and tied to browser `getBBox()` / float serialization quirks.
- The string set is huge and the mismatch is not practically modelable without a browser engine.
- The fix would risk destabilizing many diagrams for a marginal strict-XML win.

Useful inventory command:

- `cargo run -p xtask -- report-overrides`

## High-ROI pitfall catalog (what to align next)

This list is ordered by “impact × frequency ÷ cost”.

### 1) Text measurement & wrapping boundaries

Common triggers:

- Long tokens (especially URLs), punctuation-heavy strings, parentheses/brackets.
- Mixed CJK + ASCII, emoji, combining marks.
- `&nbsp;`, multiple spaces, trailing whitespace, `\r\n`, trailing blank lines.
- `\\n` literal vs newline vs `<br>` variants.

Notes:

- `dom-mode parity` is primarily **structural**. It intentionally does not try to prove geometry
  parity for all numeric attributes (e.g. `translate(x,y)` payloads). For geometry-sensitive
  issues (especially text measurement), use `dom-mode parity-root` and/or layout goldens.
- Flowchart HTML labels are the highest churn area because small changes in measured line count
  cascade into Dagre node sizes → edge routes → root viewport (`viewBox`/`max-width`) deltas.

How to validate coverage:

- Search existing tests: `rg -n "wrap|measure|markdown|htmlLabels" crates/merman-render/src/text.rs`
- Scan flowchart stress fixtures: `fixtures/upstream-svgs/flowchart/` + `docs/alignment/FLOWCHART_*`

Suggested fixture matrix:

- `htmlLabels: true|false`
- `wrappingWidth: 80|120|200`
- Token types: URL, `a__b`, `` `code` ``, `_italic_`, `**bold**`, `~~strike~~`
- Inputs with `\r\n`, trailing newline, multiple spaces

#### Flowchart-specific: quoted-string whitespace height parity

Mermaid FlowDB preserves whitespace for `labelType=string` labels (quoted strings), but upstream
SVG baselines do **not** consistently allocate extra line height for *trailing-only* whitespace.
This is easy to over-model in headless measurement and causes large `parity-root` deltas.

Gap check:

- Use `parity-root` to surface the symptom:  
  `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 6 --filter whitespace_068`
- Inspect whether node heights inflate (e.g. a 1-line label becomes 2 lines worth of height) and
  whether root `viewBox` height grows as a result.

Evidence fixture:

- `fixtures/flowchart/stress_flowchart_html_label_whitespace_068.mmd`

### 2) `htmlLabels` semantics (diagram-specific precedence)

Flowchart gotchas:

- Global `htmlLabels` vs `flowchart.htmlLabels` (node/subgraph/edge labels may differ).
- Node label `wrappingWidth` vs edge label default 200px.

How to validate coverage:

- Use a “toggle matrix” fixture set and ensure each label category follows upstream.
- Confirm CSS selectors match the correct label mode (SVG text vs `<foreignObject>`).

### 3) Markdown subset parity (tokenization, escaping, delimiters)

Common triggers:

- `_` delimiter open/close rules (`a__node`, `_a_b_`, `_a__b_`).
- Inline code suppressing emphasis parsing.
- Escaping and entity decoding in the markdown→HTML pipeline.

How to validate coverage:

- Flowchart markdown + SVG-label markdown must agree on token boundaries.
- Confirm both layout measurement and emitted SVG use the same “plain text” model.

### 4) Theme/config precedence and CSS selector drift

Common triggers:

- `themeVariables` vs top-level config (`fontSize`, `fontFamily`, `fontWeight`, colors).
- `theme=default` special cases (avoid implicitly applying `base` defaults).
- Inline `classDef` / `style` overriding label font properties.

How to validate coverage:

- Run the same fixture under multiple `init` configs and compare:
  - DOM structure unchanged (unless upstream does)
  - only expected `style` attributes change

### 5) SVG DOM stability (ordering, IDs, URL refs)

Common triggers:

- Node/edge emission order changes causing unstable diffs.
- `marker-end="url(#...)"` id generation, escaping, and re-use.
- Link wrappers (`<a>`) affecting bbox/root viewport.

How to validate coverage:

- Use `--dom-mode parity` for structure and `--dom-mode parity-root` for viewport impact.

### 6) Subgraphs, boundary clipping, and edge geometry

Common triggers:

- Edges that enter/exit clusters; clipping points must match upstream.
- Edge labels positioned on post-clipped paths.

How to validate coverage:

- Prefer fixtures with external edges + cluster titles near label positions.
- Use `xtask debug-svg-data-points` for path point drift triage.

### 7) Diagram-specific “repeat offenders”

- **Sequence**: note wrapping, activation stacking, message font precedence, title bbox.
- **Gantt**: date parsing/timezone, “today”, axis formats, label wrapping.
- **Class**: generics, escaped `<`/`>`, namespaces, multiline member blocks.
- **State**: composite padding, classDef affecting HTML labels, link sanitization.
- **Mindmap**: multiline CJK, indentation depth, root viewport drift.

## Standard workflow (repeatable loop)

1. Reproduce mismatch in a small fixture.
2. Confirm in-scope and upstream-covered.
3. Fix (model change preferred; override only when justified).
4. Add regression (fixture or unit test).
5. Run:
   - `cargo run -p xtask -- compare-all-svgs --check-dom --dom-decimals 3`
   - `cargo nextest run -p merman-render`
