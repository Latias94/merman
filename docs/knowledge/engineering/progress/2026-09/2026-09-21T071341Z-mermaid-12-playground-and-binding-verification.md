---
type: "Work Progress"
title: "Mermaid 12 Playground and binding verification"
description: "Production Compare verification, binding coverage and source-backed renderer test migration."
timestamp: 2026-09-21T07:13:41Z
producer_id: "codex-mermaid12"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "512b55f82"
---

# Status

The Mermaid 12 alignment goal remains active; release target is v0.8.0-alpha.7.
No release, push, merge or commit was made in this continuation. Package versions remain alpha.6.
Preserve the existing large working diff, including target-generated SVGs and reference projections.

# Verified changes

- Playground Compare's installed dependency tree contains Mermaid 12.0.0, with built-in ELK;
  only Tidy and ZenUML remain external. The opaque runtime artifact was rebuilt and verified.
- Added automatic diagram theme selection, distinct from explicit classic `default`. Historical
  share links with omitted/default theme retain no-override semantics. Independent review found
  no configuration-precedence or share migration regression. Worker checks: 72 targeted tests,
  both TypeScript projects, license check, production build and distribution graph verification.
- Rebuilt full production WASM (`7096c4af12f0` input digest), assembled packages, then ran the
  normal Playground build including WASM freshness, opaque runtime and distribution checks.
  Eight Chromium smoke cases passed across the initial run and targeted rerun, covering
  production WASM, Compare lifecycle/version, built-in ELK, Agentflow/Usecase and auto/default.
  The theme test initially used visible button text instead of its accessible name; fixed to Theme.
  Later Class/CSS production edits make this WASM artifact stale; rebuild once at final integration.
- Full-feature native WASM tests: 39/39. Corrected stale default-layout and ASCII-count assertions.
  Enabled C consumer smoke passed new-family SVG/error/allocation cleanup and same-engine reuse;
  enabled UniFFI new-family error/reuse test passed. These are distinct from earlier thin-feature tests.
- ELK endpoint geometry now follows shared source intersection handling for ellipses and removes
  duplicate endpoints even in a two-point route. Seven focused tests passed; independent source review passed.
- Web smoke defaults now expect ELK and retain explicit Dagre coverage. Actual full WASM smoke
  passed 37 diagram families before the later Class changes. Two Web script test path assertions now
  normalize Windows separators; eight targeted Node tests passed.
- Flowchart tests separate source-defined classic measurement or Dagre behavior from Mermaid 12
  defaults. Source defaults are 14px text, 120px node wrapping/minimum label width; ordinary edge
  labels retain createText's 200px default. Image positioning now verifies the source geometry formula.
- Sequence default Neo tests preserve the new 3px message endpoint inset. Exact classic bounds,
  line wrapping and filter-none regressions explicitly select classic instead of changing production.
- Class renderer now emits the upstream palette CSS and class color slots in declaration order;
  notes/interfaces do not consume slots. Shared info-like CSS root font uses top-level configuration
  independently of the theme font. Palette CSS trims declaration whitespace as upstream Stylis does.
  Independent review found no source mismatch. Full Class CSS equality with genuine target SVG passed.

# Renderer verification

- Lean-family baseline run: 252 tests, 220 passed / 32 failed before migration.
- Enabled-ELK diagnostic run: 265 tests, 218 passed / 47 failed before subsequent fixes.
- Flowchart and Sequence after fixes, with ELK enabled: 178/178 passed, one pre-existing skip.
  Log: `target/mermaid12-flowchart-sequence-tests.log`.
- Class/State plus shared CSS after fixes: 95/97 passed; remaining failures were palette whitespace
  and an outdated Sankey root-font assertion. Both fixed. The focused Class strict CSS, shared CSS
  and palette regression suite with ELK/Cytoscape enabled then passed 13/13.
  Logs: `target/mermaid12-class-state-css-tests.log`, `target/mermaid12-class-css-final-tests.log`.
- Root-owned modified Rust files were formatted and scoped git diff checks passed.

# Source-backed distinctions

- Mermaid 12 ELK preserves empty subgraphs as groups. Three duplicate-ID oracle cases produce two
  same-ID wrappers (one has invalid upstream geometry); first-definition-only tests belong to Dagre.
  No production deduplication was added and no NaN output was copied. Existing ELK empty-group tests remain.
- Rectangle Packing intentionally delegates to Box for unstackable triples. A seven-line tall node
  now distinguishes the providers in the existing smoke corpus. Local elkjs 0.9.3 oracle confirms the
  distinction; no algorithm-uniqueness assertion was removed.

# Remaining work

1. Continue U15 semantic/layout golden refresh and reviewed target SVG/residual admission; do not
   relabel old fixture identities or mechanically update residual signatures.
2. Run remaining renderer library regression coverage and address real source-backed failures.
   Prior complete-render run had additional library/golden failures outside the seven family binaries.
3. Review other hand-built family CSS root-variable emitters: only the common info-like path was
   corrected here; Architecture/Flowchart/State/Sequence and other standalone emitters still have
   their own root-font plumbing. Use pinned mermaidAPI.createCssStyles as authority.
4. Finish feature/dependency/license/size checks, strict reference checks and final coherent WASM
   rebuild/browser integration after production code settles. Earlier browser success is batch evidence.
5. Update version projections/changelog to alpha.7 only at the final admission boundary; prepare
   focused reviewed commits without including unrelated edits. Do not publish or merge.
