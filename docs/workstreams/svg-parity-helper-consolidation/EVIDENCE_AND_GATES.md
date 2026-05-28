# SVG Parity Helper Consolidation - Evidence And Gates

Status: Complete
Last updated: 2026-05-28

## Gate Set

```bash
cargo nextest run -p merman-render fmt_points
cargo nextest run -p merman-render radar
cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3
cargo fmt -p merman-render -- --check
cargo nextest run -p merman-render
cargo clippy -p merman-render --all-targets -- -D warnings
```

## Evidence Anchors

- `crates/merman-render/src/svg/parity/util.rs`
- `crates/merman-render/src/svg/parity/radar.rs`

## Fresh Evidence

Recorded on 2026-05-28 in the local development workspace.

```text
cargo nextest run -p merman-render fmt_points
Result: pass, 1 test run, 1 passed, 205 skipped

cargo nextest run -p merman-render radar
Result: pass, 1 test run, 1 passed, 205 skipped

cargo run -p xtask -- compare-radar-svgs --check-dom --dom-mode parity --dom-decimals 3
Result: pass

cargo fmt -p merman-render -- --check
Result: pass

cargo nextest run -p merman-render
Result: pass, 206 tests run, 206 passed, 0 skipped

cargo clippy -p merman-render --all-targets -- -D warnings
Result: pass
```

## Notes

- Added shared point-list formatting helpers in `svg::parity::util`.
- Radar polygon graticule and curve emission now use the helper.
- Existing `escape_xml(&points)` call sites were preserved to avoid mixing helper extraction with
  escaping behavior changes.
- The broader P0 helper consolidation remains open for future adopters.
