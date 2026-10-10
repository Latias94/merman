# System-only export font database ownership — 2026-09-11

Base: `cc3039fbc`. Scope: native `merman-export`, `png,jpeg,pdf`, macOS arm64.

`ExportFontEnvironment::from_resources` previously cloned the system database before resolving
embedded resources, then discarded that clone when the effective source policy was SystemOnly.
The returned database was already the original shared `Arc`.

The candidate retains that `Arc` and calls `Arc::make_mut` only in the effective embedded-source
branch. Mixed modes still copy before merging; EmbeddedOnly still avoids system discovery.
Source evidence maps, catalog asset evidence, policy intersection, and generic-family setup remain.
No font files or dependencies are added.

This is a structural copy-elimination result, not a measured latency or RSS result. With `S`
backing font-database slots and `M` bytes of copied metadata, the old path performed an unnecessary
`O(S + M)` transient copy; the new path performs an `O(1)` Arc clone at that point. Font bytes
behind shared source handles were never copied. Evidence preparation still traverses the database
and stores one record per live face, so total preparation is not constant-time.

Evidence is the ownership path and fontdb 0.23.0's derived database clone, plus behavioral tests.
Pointer-identity assertions alone do not detect a transient clone. Existing tests now check that
both merge orders preserve the caller's database and system face IDs, and that SystemOnly retains
system-source evidence through actual usvg text resolution with a deterministic font fixture.

Validation:

- Baseline font environment tests: 19/19 passed.
- Candidate: `cargo nextest run --locked -p merman-export --features png,jpeg,pdf --build-jobs 2
  --test-threads 2`: 96/96 passed, including export/resource/error contracts.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked -p merman-export --features png,jpeg,pdf --lib --jobs 2`: passed with
  warnings in unchanged code; this is not a warning-free result.

SVG generation and layout are unchanged. Experiment registration and command logs are local under
`target/bench/experiments/system-only-fontdb-2026-09-11/`. This checkpoint closes the redundant
SystemOnly database-copy issue; it does not qualify presets or close C7a/C7b.
