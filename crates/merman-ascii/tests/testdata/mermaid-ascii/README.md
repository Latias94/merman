# mermaid-ascii Golden Fixtures

These fixtures are copied from the MIT-licensed
[`AlexanderGrooff/mermaid-ascii`](https://github.com/AlexanderGrooff/mermaid-ascii) project.

- Source commit: `6fffb8e`
- Source path: `cmd/testdata`
- License: MIT
- Local license copy: `crates/merman-ascii/LICENSES/mermaid-ascii-MIT.txt`

The files keep the upstream `input---expected-output` split format. They are tracked here because
`repo-ref/` is gitignored and must not be required by CI, crates.io packages, or downstream users.
Graph fixture parity is tracked by `tests/graph_fixture.rs`; known non-matching graph fixtures are
named in `GRAPH_FIXTURE_GAPS.md`.

Expected inventory:

| Directory | Fixture count | Purpose |
| --- | ---: | --- |
| `ascii` | 52 | Graph diagrams rendered with ASCII characters. |
| `extended-chars` | 23 | Graph diagrams rendered with Unicode box drawing characters. |
| `sequence` | 12 | Sequence diagrams rendered with Unicode box drawing characters. |
| `sequence-ascii` | 5 | Sequence diagrams rendered with ASCII characters. |
