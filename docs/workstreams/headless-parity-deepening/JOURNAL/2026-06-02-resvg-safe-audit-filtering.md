# HPD-080 Resvg-Safe Audit Filtering

Date: 2026-06-02

## Problem

The ignored all-supported `resvg_safe` audit is useful for finding real renderability failures, but
the raster version can exceed a normal Codex command timeout when run across every supported
fixture. A timeout gives no fixture-level signal and makes the audit hard to use while triaging
blank output, invalid SVG, or PNG conversion failures.

## Change

Added optional filters to `crates/merman/tests/resvg_safe_fixture_smoke.rs`:

- `MERMAN_RESVG_SAFE_AUDIT_FAMILY=journey`
- `MERMAN_RESVG_SAFE_AUDIT_FAMILY=journey,timeline`
- `MERMAN_RESVG_SAFE_AUDIT_FILTER=upstream_cypress`

Default behavior is unchanged. Without filters the ignored audit still expects the broad supported
fixture corpus. With filters, it expects a non-empty filtered audit and at least one rendered
fixture.

## Verification

- `cargo fmt --check -p merman`
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke`
- `$env:RUSTFLAGS='-C linker=rust-lld'; $env:MERMAN_RESVG_SAFE_AUDIT_FAMILY='journey'; cargo nextest run -p merman --features raster --test resvg_safe_fixture_smoke --run-ignored ignored-only all_supported_fixtures_render_headless_resvg_safe_audit`

## Residual Notes

The unfiltered raster all-supported audit remains expensive. Use family/name filters while triaging
actual renderability failures, then keep the representative raster smoke as the normal gate.
