# Generated Default Config Parity - Evidence And Gates

Status: Active
Last updated: 2026-05-31

## Smallest Current Repro

```bash
cargo run -p xtask -- verify-default-config
```

Before GDC-020 this failed as an unknown command. After GDC-020 it should run independently of the
DOMPurify checkout and report only default-config verification state.

## Gate Set

### Targeted Iteration Gates

```bash
cargo nextest run -p xtask
cargo run -p xtask -- verify-default-config
cargo run -p xtask -- verify-dompurify-defaults
```

`cargo nextest run -p xtask` proves the Rust-side helper behavior. The two xtask commands prove the
CLI surface and artifact-specific source requirements.

### Package Gates

```bash
cargo nextest run -p merman-core config
cargo nextest run -p merman-render
```

These become required when a task changes generated default config content or renderer-visible
defaults.

### Formatting And Diff Gates

```bash
cargo fmt --check
git diff --check
```

### Broader Closeout Gate

```bash
$env:CARGO_PROFILE_TEST_DEBUG='0'; $env:CARGO_BUILD_JOBS='2'; cargo nextest run --workspace
```

Use the workspace gate at closeout or after default content changes. For command-surface-only work,
record why targeted gates were sufficient.

### Review Gate

Run `review-workstream` before accepting task or lane completion.

## Evidence Log

- 2026-05-31 GDC-020 red check:
  - Command: `cargo run -p xtask -- verify-default-config`
  - Result: failed with `UnknownCommand("verify-default-config")`, proving the split command does
    not exist before implementation.
- 2026-05-31 GDC-020 implementation checks:
  - Command: `cargo nextest run -p xtask`
  - Result: passed, 62 tests.
  - Command: `cargo run -p xtask -- verify-default-config`
  - Result: failed only with `default config mismatch`, proving the default-config gate is now
    independent from DOMPurify checkout state. This is the expected GDC-030 follow-up.
  - Command: `cargo run -p xtask -- verify-dompurify-defaults`
  - Result: failed only because `repo-ref/dompurify/dist/purify.cjs.js` is missing.
  - Command: `cargo run -p xtask -- verify-generated`
  - Result: failed with aggregated default-config mismatch and DOMPurify missing-checkout error.
  - Command: `cargo fmt --check`
  - Result: passed.
  - Command: `git diff --check`
  - Result: passed.
- 2026-05-31 GDC-030 implementation checks:
  - Command: `cargo nextest run -p xtask default_config`
  - Result: passed, 2 tests.
  - Command: `cargo run -p xtask -- verify-default-config`
  - Result: passed.
  - Command: `cargo run -p xtask -- gen-default-config --no-local-overrides --out target/xtask/default_config.schema_only.json`
  - Result: passed, proving schema-only diagnostics remain available.
  - Command: `cargo nextest run -p xtask`
  - Result: passed, 64 tests.
  - Command: `cargo nextest run -p merman-core config`
  - Result: passed, 11 tests.
  - Command: `cargo nextest run -p merman-render`
  - Result: passed, 248 tests.
  - Command: `cargo fmt --check`
  - Result: passed.
  - Command: `git diff --check`
  - Result: passed.
  - Command: `cargo run -p xtask -- verify-generated`
  - Result: failed only with the DOMPurify missing-checkout error for
    `repo-ref/dompurify/dist/purify.cjs.js`; default-config verification is no longer a blocker.

## Evidence Anchors

- `docs/workstreams/generated-default-config-parity/DESIGN.md`
- `docs/workstreams/generated-default-config-parity/TODO.md`
- `docs/workstreams/generated-default-config-parity/MILESTONES.md`
- `docs/adr/0019-generated-default-config.md`
- `docs/adr/0024-dompurify-default-allowlists-and-generation.md`
- `crates/xtask/default_config_overrides.json`
