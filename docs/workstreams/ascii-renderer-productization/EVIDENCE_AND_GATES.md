# ASCII Renderer Productization - Evidence And Gates

Status: Active
Last updated: 2026-05-28

## Planned Gates

### Scope Gate

```pwsh
git diff --check
```

Proves the planning docs and later copied fixtures have no whitespace errors.

### Crate Foundation Gate

```pwsh
cargo fmt --all --check
cargo check -p merman-ascii
cargo nextest run -p merman-ascii
```

Proves the ASCII crate builds, has test coverage, and follows workspace formatting.

### Public API Gate

```pwsh
cargo check -p merman --features ascii
cargo nextest run -p merman --features ascii
cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings
```

Proves the top-level library feature compiles, tests pass, and the public API avoids lint debt.

### CLI Gate

```pwsh
cargo check -p merman-cli --features ascii
cargo nextest run -p merman-cli
```

Proves CLI integration compiles and does not regress existing CLI behavior. This gate only becomes
required if ARP-080 ships CLI support inside this lane.

### Review And Verification Gate

Run `review-workstream` before accepting each implementation task. Run `verify-rust-workstream`
before marking the lane complete. Record fresh evidence here instead of relying on stale command
output.

## Evidence Anchors

- `docs/adr/0065-ascii-output-boundary.md`
- `docs/workstreams/ascii-renderer-productization/DESIGN.md`
- `docs/workstreams/ascii-renderer-productization/TODO.md`
- `docs/workstreams/ascii-renderer-productization/MILESTONES.md`
- Future `crates/merman-ascii/README.md`
- Future tracked third-party notice/license file for `mermaid-ascii`
- Future `crates/merman-ascii/tests/testdata`

## Evidence Log

- 2026-05-28: Workstream and ADR created. Implementation gates are pending because no ASCII crate
  exists yet.
- 2026-05-28: ARP-020 and ARP-030 foundation gates:
  - `cargo fmt --all --check` passed.
  - `cargo check -p merman-ascii` passed.
  - `cargo nextest run -p merman-ascii` passed: 5 tests.
  - `cargo nextest run -p merman-ascii fixture_inventory` passed: 2 tests.
  - `cargo clippy -p merman-ascii --all-targets -- -D warnings` passed.
  - `cargo test -p merman-ascii --doc` passed: 0 doctests.
  - `cargo package -p merman-ascii --list --allow-dirty` passed and listed the upstream MIT
    license copy plus copied golden fixtures in the crate package.
  - `git diff --check` passed.
  - `.gitattributes` excludes only the copied `mermaid-ascii` text goldens from `blank-at-eol`
    checks because their expected character-grid output intentionally preserves trailing cells.
  - Fixture inventory copied from `repo-ref/mermaid-ascii/cmd/testdata` at source commit `6fffb8e`:
    52 graph ASCII, 23 graph Unicode, 12 sequence Unicode, and 5 sequence ASCII fixtures.
- 2026-05-28: ARP-040 graph tracer-bullet gates:
  - `cargo nextest run -p merman-ascii graph::` passed: 6 tests.
  - `cargo nextest run -p merman-ascii graph_golden` passed: 6 tests.
  - `cargo check -p merman-ascii` passed.
  - `cargo nextest run -p merman-ascii` passed: 16 tests.
  - `cargo clippy -p merman-ascii --all-targets -- -D warnings` passed.
  - `cargo test -p merman-ascii --doc` passed: 0 doctests.
  - `cargo fmt --all --check` passed.
  - `git diff --check` passed.
  - Golden coverage now includes ASCII/Unicode single node, ASCII/Unicode left-to-right direct
    edge, ASCII long labels, and ASCII top-down linear chains.
  - Public flowchart tests cover the tracer-bullet render path, grid cell limit enforcement, and
    explicit unsupported-feature errors for edge labels, subgraphs, and non-LR/TD directions.
- 2026-05-28: ARP-050 flowchart adapter gates:
  - `cargo nextest run -p merman-ascii flowchart` passed: 18 tests.
  - `cargo check -p merman-ascii` passed.
  - `cargo nextest run -p merman-ascii` passed: 28 tests.
  - `cargo clippy -p merman-ascii --all-targets -- -D warnings` passed.
  - `cargo test -p merman-ascii --doc` passed: 0 doctests.
  - `cargo fmt --all --check` passed.
  - `git diff --check` passed.
  - Parser/model-level tests now render simple `flowchart LR`, `graph LR`, and `flowchart TB`
    inputs through `merman_core::Engine::parse_diagram_for_render_model_sync` and compare against
    upstream `mermaid-ascii` golden output.
  - Parser/model-level tests explicitly reject edge labels, subgraphs, non-LR/TD directions,
    non-rectangular shapes, edge length modifiers, dotted strokes, and open/non-point edge arrows.
  - Hand-built model tests explicitly reject edges with missing endpoint nodes instead of silently
    dropping them.
- 2026-05-28: ARP-060 sequence vertical slice gates:
  - `cargo fmt --all --check` passed.
  - `cargo check -p merman-ascii` passed.
  - `cargo nextest run -p merman-ascii sequence` passed: 10 tests.
  - `cargo nextest run -p merman-ascii sequence_golden` passed: 2 tests.
  - `cargo nextest run -p merman-ascii` passed: 37 tests.
  - `cargo clippy -p merman-ascii --all-targets -- -D warnings` passed.
  - `cargo test -p merman-ascii --doc` passed: 0 doctests.
  - `git diff --check` passed.
  - `rg -n "[ \t]+$" crates/merman-ascii docs/adr/0065-ascii-output-boundary.md docs/workstreams/ascii-renderer-productization -g "!crates/merman-ascii/tests/testdata/mermaid-ascii/**/*.txt"`
    returned no matches, covering untracked source/docs while excluding upstream golden fixtures
    with intentional trailing cells.
  - Parser/model-level tests now render copied upstream sequence fixtures through
    `merman_core::Engine::parse_diagram_for_render_model_sync` and compare against upstream
    `mermaid-ascii` output: 12 Unicode sequence fixtures and 5 ASCII sequence fixtures.
  - Sequence rendering now covers participants, participant boxes, lifelines, solid messages,
    dotted messages, reverse messages, self messages, empty/multiword labels, and visible
    autonumber.
  - Sequence tests explicitly reject titles, notes, activations, actor-shaped participants,
    wrapped actor labels, actor links/properties, boxes, actor create/destroy, message placement,
    wrapped messages, control messages, unknown actors, and unsupported message types.
  - Golden comparison for copied sequence fixtures intentionally follows upstream normalized
    whitespace behavior: trailing spaces are not product-significant for those fixtures.
- 2026-05-28: ARP-070 public API and host integration gates:
  - `cargo run -p merman --features ascii --example ascii_output` passed and printed the default
    Unicode sequence example.
  - `cargo run -p merman --features ascii --example ascii_output -- --ascii` passed and printed
    the same example with pure ASCII characters.
  - `cargo check -p merman --features ascii` passed.
  - `cargo nextest run -p merman-ascii` passed: 37 tests.
  - `cargo nextest run -p merman --features ascii` passed: 3 tests.
  - `cargo clippy -p merman-ascii -p merman --features ascii --all-targets -- -D warnings`
    passed.
  - `cargo test -p merman --doc --features ascii` passed: 0 doctests.
  - `cargo fmt --all --check` passed.
  - `git diff --check` passed.
  - Top-level `merman` now exposes an opt-in `ascii` feature, `merman::ascii` module, synchronous
    and async render helpers, `HeadlessAsciiRenderer`, re-exported `merman-ascii` option/error
    types, API tests, and `ascii_output` example commands in the README.

## Notes

Do not run tests against `repo-ref/mermaid-ascii` as a release gate. `repo-ref/` is ignored and is
only a local research reference. All authoritative fixtures and license records must be copied into
tracked paths before they are used by CI or release validation.
