# HPD-080 - Flowchart Renderability Audit

Task: HPD-080 Flowchart visible/renderability triage.

## Question

Does current HEAD expose a fresh Flowchart user-visible renderability defect that needs a bounded,
source-backed production fix?

## Outcome

- No fresh Flowchart visible/renderability defect was found in current HEAD.
- No production code or tests were changed.
- The existing Flowchart dark-theme smoke still counts `themeVariables.strokeWidth` only through
  DOM-consumed visible edge signals: `.edge-thickness-normal` CSS plus the current visible edge path
  class tuple.
- Host-facing `resvg_safe` representative and boundary fixtures still render successfully.
- Flowchart structural parity remains green.
- Recommendation: return this slice's remaining budget to HPD-050 Architecture unless a new failing
  renderability gate, source-backed emitted-surface gap, or concrete consumer report appears.

## Verification

- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test theme_renderability_smoke` -
  passed, `12` tests run.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman --features render --test resvg_safe_fixture_smoke` -
  passed, `5` tests run and `1` skipped.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo nextest run -p merman-render --test flowchart_svg_test` -
  passed, `29` tests run.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3` -
  passed.
- `$env:RUSTFLAGS='-C linker=rust-lld'; cargo run -p xtask -- report-overrides --check-no-growth` -
  passed; override growth and root viewport usage checks were both ok.
- `cargo fmt --check` - passed.

## Notes

- Full ignored all-supported Flowchart raster audit was not rerun. The current representative
  `resvg_safe` gate, public theme smoke, full Flowchart renderer test suite, Flowchart structural
  compare, and override-growth check were sufficient for this fresh-defect triage slice.
- Known Flowchart root/max-width residuals are outside this HPD-080 renderability slice. This pass
  did not tune layout, root viewports, or override pins.
