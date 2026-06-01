# M15C-070 Flowchart FontAwesome Root Slice

Date: 2026-06-01

## Findings

- Mermaid 11.15 Flowchart root drift for FontAwesome labels is not a structural SVG issue:
  diagram-level `parity` remains green while `parity-root` failed only on root `style`/`viewBox`.
- Standard `fa:*` labels now require a `1.25em` inline icon box in deterministic text metrics.
- The documented `fab:fa-truck-bold` custom-pack example follows the same layout-time inline box
  rule in 11.15; treating it as zero width was stale 11.12-era behavior.
- Several old Flowchart icon root pins became harmful after the measurement fix because the
  unpinned renderer output already matched the 11.15 root envelope.

## Changes

- Updated Flowchart FontAwesome text measurement to use a `1.25em` icon advance for all supported
  `fa*` prefixes.
- Updated unit expectations for standard and custom-pack FontAwesome icon labels.
- Updated the remaining Flowchart icon root pins to 11.15 upstream roots and deleted obsolete icon
  pins whose unpinned output now passes.

## Evidence

- `cargo test -p merman-render flowchart_html_fontawesome -- --nocapture`: passed.
- `cargo nextest run -p merman-render fontawesome`: passed, 6 tests.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter icons --report-root-all`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --filter fontawesome --report-root-all`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`:
  passed.
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all`:
  failed with 205 Flowchart root-only mismatches, down from the earlier M15C-070 count of 229.
- `cargo run -p xtask -- report-overrides --check-no-growth`: passed; root viewport inventory is
  now 281 entries, with 38 Flowchart root entries.
- `cargo fmt --check`: passed.
- `git diff --check`: passed.
- `cargo run -p xtask -- check-alignment`: passed.

## Next

Continue M15C-070 by sampling the remaining non-icon Flowchart root buckets before deciding whether
to update fixture-scoped root pins or extract another shared Mermaid 11.15 root viewport rule.
