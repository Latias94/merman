# Local Semantic Fixtures

This directory stores self-authored ASCII fixtures that are meant to validate semantic behavior
rather than copied upstream output.

- Use it when the copied `mermaid-ascii` corpus is a poor oracle for the shape you want to test.
- Keep the files small and focused on the behavior under review.
- Do not add copied parity fixtures here; those belong under `tests/testdata/mermaid-ascii/`.

Current examples:

- `class/dense_relations.mmd`
- `er/dense_relations.mmd`
- `state/composite_boundary.mmd`
- `xychart/mixed_small.mmd`
