# External Fixture References

This document tracks fixtures that are sourced from outside Mermaid's own repository.

These fixtures are treated as additional regression coverage and are parity-gated against upstream
Mermaid SVG baselines (Mermaid `@11.12.2`).

Pinned external repositories (commit hashes) live in `repo-ref/REPOS.lock.json`.

## mermaid-rs-renderer (mmdr) test fixtures

Source repo: `repo-ref/mermaid-rs-renderer` (see `repo-ref/REPOS.lock.json`).

### Flowchart

- `fixtures/flowchart/mmdr_tests_flowchart_flowchart_complex.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/flowchart/complex.mmd`
- `fixtures/flowchart/mmdr_tests_flowchart_flowchart_ports.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/flowchart/ports.mmd`
- `fixtures/flowchart/mmdr_tests_flowchart_flowchart_styles.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/flowchart/styles.mmd`
- `fixtures/flowchart/mmdr_tests_flowchart_flowchart_edges.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/flowchart/edges.mmd`
- `fixtures/flowchart/mmdr_tests_flowchart_flowchart_cycles.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/flowchart/cycles.mmd`
- `fixtures/flowchart/mmdr_tests_flowchart_flowchart_dense.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/flowchart/dense.mmd`
- `fixtures/flowchart/mmdr_tests_flowchart_flowchart_subgraph.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/flowchart/subgraph.mmd`
- `fixtures/flowchart/mmdr_tests_flowchart_flowchart_subgraph_direction.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/flowchart/subgraph_direction.mmd`

### Sequence

- `fixtures/sequence/mmdr_tests_sequence_sequence_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/sequence/basic.mmd`
- `fixtures/sequence/mmdr_tests_sequence_sequence_frames.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/sequence/frames.mmd`

### State

- `fixtures/state/mmdr_tests_state_state_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/state/basic.mmd`
- `fixtures/state/mmdr_tests_state_state_note.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/state/note.mmd`

### Mindmap

- `fixtures/mindmap/mmdr_tests_mindmap_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/mindmap/basic.mmd`

### Class

- `fixtures/class/mmdr_tests_class_class_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/class/basic.mmd`
- `fixtures/class/mmdr_tests_class_class_multiplicity.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/class/multiplicity.mmd`

### Gantt

- `fixtures/gantt/mmdr_tests_gantt_gantt_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/gantt/basic.mmd`

### Kanban

- `fixtures/kanban/mmdr_tests_kanban_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/kanban/basic.mmd`

## mermaid-rs-renderer issue reproductions

Some fixtures are derived from downstream issue reports and discussions.

### Flowchart

- `fixtures/flowchart/mmdr_issue_28_text_rendering.mmd`
- `fixtures/flowchart/mmdr_issue_29_edge_label_distance.mmd`

## mermaid-rs-renderer docs comparison sources

Source repo: `repo-ref/mermaid-rs-renderer` (see `repo-ref/REPOS.lock.json`).

These fixtures are used by mermaid-rs-renderer's own upstream-vs-downstream comparison docs and
are parity-gated against Mermaid SVG baselines:

### Sequence

- `fixtures/sequence/mmdr_docs_comparison_sources_sequence_autonumber.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/docs/comparison_sources/sequence_autonumber.mmd`
- `fixtures/sequence/mmdr_docs_comparison_sources_sequence_collab.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/docs/comparison_sources/sequence_collab.mmd`
- `fixtures/sequence/mmdr_docs_comparison_sources_sequence_loops.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/docs/comparison_sources/sequence_loops.mmd`
- `fixtures/sequence/mmdr_docs_comparison_sources_sequence_notes.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/docs/comparison_sources/sequence_notes.mmd`
- `fixtures/sequence/mmdr_docs_comparison_sources_sequence_oauth.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/docs/comparison_sources/sequence_oauth.mmd`
- `fixtures/sequence/mmdr_docs_comparison_sources_sequence_microservice.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/docs/comparison_sources/sequence_microservice.mmd`

## mermaid-rs-renderer bench fixtures

Source repo: `repo-ref/mermaid-rs-renderer` (see `repo-ref/REPOS.lock.json`).

These fixtures are sourced from mermaid-rs-renderer's benchmark corpus and are useful for
layout + rendering regressions at larger sizes.

### Sequence

- `fixtures/sequence/mmdr_benches_fixtures_sequence.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/benches/fixtures/sequence.mmd`
- `fixtures/sequence/mmdr_benches_fixtures_sequence_medium.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/benches/fixtures/sequence_medium.mmd`
- `fixtures/sequence/mmdr_benches_fixtures_expanded_sequence_long_labels.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/benches/fixtures/expanded/sequence_long_labels.mmd`
- `fixtures/sequence/mmdr_benches_fixtures_expanded_sequence_frames_notes.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/benches/fixtures/expanded/sequence_frames_notes.mmd`
