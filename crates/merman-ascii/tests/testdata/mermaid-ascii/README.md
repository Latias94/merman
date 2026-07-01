# mermaid-ascii Golden Fixtures

These fixtures are copied from the MIT-licensed
[`AlexanderGrooff/mermaid-ascii`](https://github.com/AlexanderGrooff/mermaid-ascii) project.

- Source commit: `6fffb8e`
- Supplemental graph fixtures: `tight_arrow.txt` and `tight_arrow_mixed.txt` copied from local
  `repo-ref/mermaid-ascii` commit `876b5b4` after upstream renamed the no-whitespace edge cases.
- Source path: `cmd/testdata`
- License: MIT
- Local license copy: `crates/merman-ascii/LICENSES/mermaid-ascii-MIT.txt`

The files keep the upstream `input---expected-output` split format. They are tracked here because
`repo-ref/` is gitignored and must not be required by CI, crates.io packages, or downstream users.
Graph fixture parity is tracked by `tests/graph_fixture.rs`; known non-matching graph fixtures are
named in `GRAPH_FIXTURE_GAPS.md`. Sequence fixture parity is tracked by
`tests/sequence_model.rs`; copied upstream sequence fixture status is named in
`SEQUENCE_FIXTURE_GAPS.md`.

This directory is the copied-upstream oracle only. When a diagram is too dense, too family-specific,
or too semantically different for the upstream output to be a meaningful baseline, the tests should
use a local semantic fixture near the test file instead of stretching this inventory into a second
standard.

Three copied graph cases keep their rendered spacing via explicit test metadata in
`tests/graph_fixture.rs`; their Mermaid input stays directive-free.

Expected inventory:

| Directory | Fixture count | Purpose |
| --- | ---: | --- |
| `ascii` | 54 | Graph diagrams rendered with ASCII characters. |
| `extended-chars` | 25 | Graph diagrams rendered with Unicode box drawing characters. |
| `sequence` | 12 | Sequence diagrams rendered with Unicode box drawing characters. |
| `sequence-ascii` | 5 | Sequence diagrams rendered with ASCII characters. |

Upstream also includes `cmd/testdata/multibyte` examples for accented Latin, Greek, and Cyrillic
labels. They are treated as semantic coverage rather than copied exact-output fixtures because
`merman-ascii` preserves readable labels but does not copy `mermaid-ascii`'s current LR label
spacing byte-for-byte.
