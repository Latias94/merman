---
type: "Work Progress"
title: "Mermaid 12 editor syntax and remaining layout dispatch checkpoint"
description: "Both new families reach editor syntax and public catalogs; three existing-family layout adapters are in progress before promotion."
timestamp: 2026-09-20T21:16:50Z
record_id: "9ec2c204-55e2-4bba-a655-e8591c4959e1"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "464b4d1cf"
supersedes: "46ce1a02-cf9e-4e33-ae42-8be866f4da6b"
---

# Status

GOAL remains active. Target alpha.7 includes unpublished main plus this alignment; package versions
remain alpha.6 and production Mermaid reference remains 11.17.2 until U15. Do not treat these partial
integration results as completion. The unrelated core tests/flowchart.rs formatting diff remains
untouched and unstaged. No push, release, PR or merge occurred.

# Implemented

- `7a8a5e342`: Agentflow first edge endpoints are entities, later occurrences are references; exact
  declaration/lexer spans support container IDs, repeated endpoints and presentation references.
  Flowchart interaction failures retain payload evidence, preventing unrelated completion snippets.
- `529a78b75`: Usecase joins the public catalog (37 families), bindings metadata, Web catalog,
  Playground baseline/variant examples and Merman semantic/layout regression fixtures.
- `464b4d1cf`: Agentflow and Usecase Tree-sitter roots, structured declarations/edges, queries,
  native parser and language WASM; source provenance distinguishes original 11.16.1 derivations
  from new 12.0.0 translations. Legal materials were generated through existing owners.
- Independent syntax review found and fixed keyword-prefix IDs in Usecase and Agentflow's incomplete
  YAML grammar. Agentflow preserves scoped YAML payload for Core instead of duplicating YAML parsing.
- Windows WASI SDK builds omit the Clang suffix. Generation now verifies SDK VERSION and exact
  supported Clang version spellings; ABI and size limits remain unchanged.

# Evidence

- Editor completion/structure/semantic facts: 85 passed (1a3beb52-3be2-41df-9f9c-fff99374bf18).
- Agentflow and appearance tests: 40 passed (7dacd64c-3263-48e9-807a-9d8d9df186fc), including
  authored Mindmap layout precedence for the next adapter change.
- Tree-sitter incremental/query/conformance: 14 passed (59358cc0-ad35-450a-99e0-75b451cf21bf).
  All strict-valid Merman fixtures parse cleanly; final corpus command passed with 13 new cases.
- Built Tree-sitter WASM: 3 passed (both new family roots and ABI15). WASM size 3,672,533 bytes,
  below the unchanged 5MiB limit; previous asset was 3,450,333 bytes.
- Generation/C-smoke script tests: 9 passed. Web TypeScript contracts/build passed.
- Bindings-core/WASM native tests: 127 passed in preceding work; Usecase focused SVG: 7 passed.
- License verifier passed; generated legal material check covered 382 projections.
- Earlier strict feature matrix passed all 117 combinations and 34 artifact profiles. Do not
  rerun it just for metadata changes.
- Independent review of editor fact changes reported no findings after the focused fixes.

The current Web WASM package set still predates Usecase catalog admission. Rebuild once after
remaining core/layout work settles. New Merman fixtures are regression snapshots, not Mermaid12
oracle evidence. Current generated Usecase layout=dagre reflects the still-selected old default;
Usecase rendering in Mermaid12 follows the global registered layout.

# Remaining Work and Ownership

U10 source audit found State, Requirement and Mindmap still bypassing registered layout selection.
Workers are implementing those adapters with existing ELK kernels, operation controls and separate
paint fallback for algorithms returning no edge sections. Root owns shared family.rs/model.rs and
Mindmap authored-layout precedence in core config/appearance.rs. Shared minNodeWidth work is in
progress for source-reachable families (Flowchart/Agentflow/State/Usecase, not Class/ER).

Continue source-backed U9/U10 appearance/geometry and remaining Usecase DOM/shape/style evidence.
Agentflow typed diagnostic position projection still needs an exact upstream Unicode/frontmatter
contract check; warning-fact editor spans already use the common source map.

U15 must add Agentflow/Usecase to xtask UPSTREAM_SVG_DIAGRAMS, construct a valid base-bound selection
receipt, regenerate authentic target config/theme/SVG and then Merman goldens, admit current Web
WASM/Playground/FFI and finally project alpha.7 plus changelog. Historical Cypress and original
syntax provenance must retain their true identities. Version owner requires a clean linked release
worktree; do not discard current work to satisfy it.
