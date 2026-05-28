# ASCII Renderer Productization - Milestones

Status: Active
Last updated: 2026-05-28

## M0 - Scope And Evidence Freeze

Exit criteria:

- ASCII output boundary is captured in an ADR.
- Workstream docs agree on crate boundary, non-goals, and validation gates.
- Third-party provenance policy for `mermaid-ascii` is explicit.
- First executable implementation task is identified.

Primary evidence:

- `docs/adr/0065-ascii-output-boundary.md`
- `docs/workstreams/ascii-renderer-productization/DESIGN.md`
- `docs/workstreams/ascii-renderer-productization/TODO.md`

## M1 - Crate And Provenance Foundation

Exit criteria:

- `crates/merman-ascii` exists as a workspace member.
- Public option/error types are sketched with tests.
- README and tracked third-party notice/license files cite upstream source, commit, and MIT license.
- Copied upstream fixture inventory is available to CI.

Primary gates:

- `cargo fmt --all --check`
- `cargo check -p merman-ascii`
- `cargo nextest run -p merman-ascii`

## M2 - Flowchart Vertical Slice

Exit criteria:

- Text width, canvas, charset, graph layout, routing, and drawing primitives exist.
- Basic flowcharts render from `FlowchartV2Model`.
- ASCII and Unicode graph golden tests cover the first supported subset.
- Unsupported flowchart features are documented or reported.

Primary gates:

- `cargo nextest run -p merman-ascii graph::`
- `cargo nextest run -p merman-ascii flowchart`

## M3 - Sequence Vertical Slice

Status: Met on 2026-05-28 by ARP-060.

Exit criteria:

- Participants and basic sequence messages render from `SequenceDiagramRenderModel`.
- ASCII and Unicode sequence golden tests cover the first supported subset.
- Unsupported sequence constructs have documented degradation behavior.

Primary gates:

- `cargo nextest run -p merman-ascii sequence`
- `cargo nextest run -p merman-ascii sequence_golden`

## M4 - Public API And Host Integration

Status: Library API met on 2026-05-28 by ARP-070; CLI decision remains in ARP-080.

Exit criteria:

- Top-level `merman` exposes ASCII output behind an opt-in feature.
- API examples compile.
- CLI integration is either shipped or explicitly split into a follow-on.
- README and CHANGELOG describe the new capability and limitations.

Primary gates:

- `cargo check -p merman --features ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo nextest run -p merman-cli`

## M5 - Verification And Closeout

Exit criteria:

- Final focused gates have fresh evidence.
- Workstream status and handoff reflect shipped behavior.
- Remaining unsupported Mermaid families or features are either deferred in TODO or split into new
  workstreams.
- License/provenance files are complete.

Primary gates:

- `cargo fmt --all --check`
- `cargo nextest run -p merman-ascii`
- `cargo nextest run -p merman --features ascii`
- `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
- `git diff --check`
