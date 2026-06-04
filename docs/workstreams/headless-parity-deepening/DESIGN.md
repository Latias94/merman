# Headless Parity Deepening - DESIGN

Status: Active
Last updated: 2026-06-02

## Problem

`merman` has completed Mermaid `11.15.0` structural SVG DOM parity for the implemented diagram
matrix, but the remaining work is no longer “finish a few missing features”. The active debt is a
 mix of:

- root viewport / `max-width` residuals,
- browser-vs-headless text measurement drift,
- visible rendering defects that structural DOM parity does not catch, especially missing
  diagram-specific CSS/theme emission,
- duplicated baseline facts and generated provenance,
- shallow diagram registry and semantic/render seams,
- solver/input-model residuals in `manatee` / `dugong`-adjacent layout paths,
- and an unclear boundary between “worth fixing” parity gaps vs. honest headless residuals.

The next phase should not continue as ad hoc “11.15 patch-up” work. It needs a durable execution
lane that deepens the headless parity architecture, deletes stale abstractions, and makes residuals
classifiable, auditable, and fixable without drifting toward browser-dependent behavior.

## Target State

1. Baseline facts for the active Mermaid version are sourced from one manifest/seam rather than
   split across docs, generated names, registries, and report labels.
2. Residual parity work is governed by an explicit taxonomy that distinguishes source-backed
   behavior gaps from headless measurement approximations and browser-only lattice tails.
3. Text measurement and root-bounds policy live behind explicit seams instead of being re-derived
   inside multiple diagram renderers.
4. Functional renderability is treated as a first-class gate: blank output, hidden text, unreadable
   labels, missing semantic colors, and missing diagram theme CSS outrank numeric root residuals.
5. `Architecture`, `Sequence`, `Flowchart`, and `Class` residual work proceeds through shared
   measurement/layout seams rather than fixture-specific constants.
6. `manatee` / `dugong` alignment work is driven by source-backed input-model and bounds-feeding
   audits, not blanket solver rewrites or aimless numerical tweaking.
7. The repository is better prepared for new diagram-family adoption without implying that all
   Mermaid `11.15.0` families are already in scope.

## Scope

- Baseline registry/provenance deepening.
- Root residual taxonomy and evidence hygiene.
- Visible rendering defect triage for the implemented matrix, especially source-backed
  diagram-specific CSS/theme emission.
- Measurement and root-bounds seam extraction where justified.
- Source-backed `Architecture` layout engine/input-model audits.
- Source-backed `Sequence` / `Flowchart` / `Class` residual consolidation where it removes a real
  class of drift.
- Planning and rubric for follow-on diagram families that are not yet in the implemented matrix.

## Non-goals

- Do not force every remaining `parity-root` residual to zero in this lane.
- Do not add browser-runtime rendering dependencies.
- Do not replace headless layout/render behavior with fixture-keyed hacks or broad hard-coded text
  tables as the primary mechanism.
- Do not open implementation work for every unsupported Mermaid family in this lane.
- Do not keep stale 11.12-era naming/provenance just because generated file renames are annoying.

## Guardrails

1. Prefer Mermaid source-backed behavior and official fixtures before changing headless heuristics.
2. When a residual is measurement-driven, fix it by deepening a reusable seam or record it as a
   bounded headless residual; do not silently smear constants across diagram code.
3. Treat visible rendering defects as higher priority than `parity-root` tails. A structurally
   matching SVG is still broken if labels are invisible, semantic colors are missing, or diagram
   theme CSS is absent.
4. `parity-root` work must distinguish:
   - source-backed behavior gaps,
   - measurement approximation gaps,
   - browser bbox / lattice residuals,
   - solver / phase residuals,
   - stale baseline / stale override mistakes,
   - unsupported-family scope gaps.
5. `manatee` / `dugong` work audits input semantics first: parent assignment, constraints,
   component ordering, relocation centers, and bounds extras feeding.
6. Delete obsolete pathways when a deeper seam truly replaces them.

## Headless Parity Gate Tiers

This lane treats Mermaid parity as a layered contract, not as a single pixel-equality target.
Mermaid JS renders through a browser; `merman` is a deterministic headless renderer. A browser
measurement artifact is useful evidence, but it is not automatically a production requirement.

Hard gates:

- parser, semantic model, and source-backed error behavior for implemented families;
- diagram-specific theme/CSS emission, readable labels, and semantic colors;
- structural SVG DOM parity for the implemented matrix;
- no blank output, hidden labels, black cards, root clipping, or unreadable fallback behavior.

Strong alignment targets:

- source-backed layout topology, group membership, edge endpoints, relative constraints, label
  wrapping, and reusable measurement/root-bounds seams;
- candidate source formulas that improve the family as a whole, not just one or two root rows.

Diagnostic sensors:

- `parity-root` `max-width` / `viewBox` numeric tails;
- small residuals from browser `getBBox()`, `getComputedTextLength()`, canvas `measureText()`,
  Cytoscape body/label bounds, and FCoSE solution decimals;
- residual counts as queue-shaping signals, not completion percentages.

Explicit non-goals:

- depending on a browser at runtime;
- fixture-keyed constants or hand-written text tables to clear individual rows;
- importing raw browser measurement artifacts when they damage the deterministic headless family
  model.

## Architecture Direction

This lane is intentionally split into five capability themes:

1. **Baseline registry**
   One authoritative Mermaid baseline registry/manifest that projects into docs, generation,
   compare/report labels, and generated override provenance.

2. **Residual governance**
   A workstream-local taxonomy and evidence ledger for residuals, including which residuals are
   aligned to fix vs. record.

3. **Visible rendering quality**
   Source-backed diagram CSS/theme and renderer checks for failures that produce blank,
   unreadable, or semantically misleading output even when DOM parity is green.

4. **Measurement / root-bounds platform**
   Shared seams for browser-like text measurement approximation and diagram root viewport policy.

5. **Layout engine audit**
   Source-backed audits of `manatee` / `dugong`-adjacent seams, initially centered on
   `Architecture`, then reused where profitable.

6. **New-family rubric**
   A disciplined gate for which Mermaid families are worth promoting into the headless support
   matrix and in what order.

## Priority Order

Completed foundation:

1. HPD-010 lane freeze and prioritization
2. HPD-020 baseline registry
3. HPD-030 residual taxonomy + evidence alignment
4. HPD-040 measurement / root-bounds platform
5. HPD-060 semantic/render unification pilot
6. HPD-070 new family rubric
7. HPD-090 baseline preparation before parity

Current execution priority:

1. HPD-080 visible rendering defect triage
2. HPD-050 focused Architecture / `manatee` / `dugong` audits when no higher-severity rendering
   defect is active

## Current Repository Reality

- Implemented-matrix structural Mermaid `11.15.0` SVG DOM `parity` is green.
- The active front is now visible rendering quality plus `parity-root`; DOM parity alone is not
  enough to declare output usable.
- Current honest `parity-root` buckets are led by:
  - Flowchart: `61`
  - Architecture: `24`
  - Sequence: `27`
  - Class: `12`
  - Timeline: `3`
  - Journey: `2`
- `Architecture` is the highest-value source-backed layout audit target because the remaining rows
  are a mix of FCoSE input parity, compound bounds feeding, Cytoscape-like measurement tails, and
  disconnected-component phase drift.
- `Sequence` is no longer dominated by central-connection semantics; the remaining rows are mostly
  note/wrap/participant measurement and root-bounds tails.
- `Flowchart` residuals are smaller-width root tails after the major 11.15 shape-source work; this
  lane should avoid reopening broad shape parity unless a shared measurement seam justifies it.
- `Class` still has tempting text-width residuals, but the workstream direction is to audit stale
  tables and generated evidence, not to grow hand-curated lookup data.

## Architecture Issue Mapping

This lane exists to turn the 2026-06-01 audit into bounded execution, not to duplicate the entire
issue ledger. The primary mapping is:

- HPD-020 baseline registry:
  - `ARCH-002` baseline facts are split and stale
  - `ARCH-034` documentation and workstream state can contradict active gates
- HPD-030 residual governance:
  - `ARCH-018` fixture parity inventory is fragmented
  - `ARCH-019` compare results are difficult to interpret
  - `ARCH-034` documentation/workstream contradiction risk
- HPD-040 measurement / root-bounds platform:
  - `ARCH-010` root viewport parity logic is scattered
  - `ARCH-011` emitted bounds logic is too entangled with family renderers
  - `ARCH-017` text and bbox approximation policy lacks a first-class seam
- HPD-050 layout engine audit:
  - `ARCH-014` Architecture layout adaptation weakens the reusable engine boundary
  - related `manatee` / `dugong` audit findings in the same ledger
- HPD-060 semantic/render unification pilot:
  - `ARCH-003` diagram detection and registry seam is too shallow
  - `ARCH-005` semantic model seam leaks into `Engine`
- HPD-070 unsupported-family rubric:
  - active alignment status and follow-on family promotion policy from `docs/alignment/STATUS.md`

## Execution Order

This lane should execute in three layers rather than as a flat backlog:

1. Truth and governance:
   baseline registry, residual taxonomy, evidence alignment.
2. Shared seams:
   measurement policy, root-bounds ownership, source-backed reusable helpers.
3. Deep audits and selective promotion:
   Architecture/solver-input work, semantic/render seam pilots, unsupported-family rubric.

The key rule is that deeper implementation slices should consume clearer truth and better seams.
If a candidate fix still depends on hidden fixture keys, old 11.12 naming, or ambiguous residual
classification, the lane should step back and fix the governing seam first.

Visible rendering defects are a priority override. If a supported diagram emits unreadable text,
blank output, black blocks, or loses Mermaid's semantic theme colors, fix that before spending more
time on small root-width residuals. The fix still needs the same evidence standard: use pinned
Mermaid source and fixtures, not browser-dependent runtime rendering or fixture-keyed cosmetics.

## Residual Taxonomy

This lane uses the following residual taxonomy. It is intentionally operational rather than
numerically precise: a row belongs to the first category that explains the evidence well enough to
drive the next action.

1. **Source-backed behavior gap**
   Mermaid source, upstream fixture output, or source-owned config semantics show that local logic
   is still wrong. Expected action: implement or refactor toward the source rule.

2. **Generated measurement gap**
   The residual is measurement-driven, but the right fix is a reusable generated or audited
   measurement seam rather than a diagram-local constant. Expected action: improve shared
   measurement inputs or generated evidence.

3. **Browser lattice tail**
   Source inputs and local rules already match, but Chromium/Cytoscape `getBBox()` /
   `getComputedTextLength()` / serialization lattice behavior still leaves a small residual.
   Expected action: keep as diagnostic unless broad generated evidence justifies a seam-level
   approximation.

4. **Stale baseline or stale override**
   The mismatch is caused by old upstream SVGs, old root pins, or old generated lookup data rather
   than a current renderer defect. Expected action: refresh or delete the stale artifact first.

5. **Solver / phase residual**
   Source inputs match, but the remaining drift is produced by layout engine solution phase,
   component ordering, relocation, or compound-bound behavior. Expected action: audit adapter or
   engine seam, not text constants.

6. **Scope boundary**
   The row belongs to an unsupported upstream family or an explicitly deferred capability. Expected
   action: track as scope, not as a hidden parity failure.

Classification rules:

- Prefer honest category assignment over forced bucket reduction.
- “Small” does not mean “browser lattice tail” by default; source-backed evidence must rule out a
  deeper defect first.
- A residual may move categories after new evidence. For example, what first looks like a solver
  issue may become a stale-baseline issue after upstream refresh.
- Counts are useful only as queue-shaping aids. The taxonomy is for deciding what kind of work is
  justified next, not for claiming completion percentages.

## Success Criteria

- Workstream docs and task ledger agree on the lane shape and first executable slice.
- Baseline registry seam exists or the repository is measurably closer to one.
- Residual taxonomy exists and at least the top active residual buckets are classified.
- Measurement / root-bounds seams are explicit enough that new parity fixes do not need new
  hidden constants in diagram code by default.
- `Architecture` residual work is traceably driven by source/input-model audits rather than
  ungrounded solver tuning.
