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

### Sequence

- `fixtures/sequence/mmdr_tests_sequence_sequence_frames.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/sequence/frames.mmd`

### State

- `fixtures/state/mmdr_tests_state_state_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/state/basic.mmd`

### Class

- `fixtures/class/mmdr_tests_class_class_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/class/basic.mmd`

### Gantt

- `fixtures/gantt/mmdr_tests_gantt_gantt_basic.mmd`
  - Source: `repo-ref/mermaid-rs-renderer/tests/fixtures/gantt/basic.mmd`

## mermaid-rs-renderer issue reproductions

Some fixtures are derived from downstream issue reports and discussions.

### Flowchart

- `fixtures/flowchart/mmdr_issue_28_text_rendering.mmd`
- `fixtures/flowchart/mmdr_issue_29_edge_label_distance.mmd`
