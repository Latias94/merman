# Default Font Configuration Fast Path (macOS, 2026-10-19)

This change borrows configured font strings and returns the canonical Mermaid default directly for
the two exact built-in raw spellings. Other nonblank configured spellings still use CSS
tokenization, normalization and safety validation; existing missing-value and explicitly blank
root-font behavior is unchanged. The evidence supports reduced short-lived allocation work; it is
not a calibrated latency claim.

## Cause and change

The Packet medium profile attributed repeated work to
`config_font_family_css -> normalize_css_font_family -> is_safe_css_font_family_value`, including
CSS parsing of Mermaid's fixed default font stack. The configuration helpers also cloned JSON
strings before normalization. The fast path recognizes only the built-in theme spelling
(`"trebuchet ms", verdana, arial, sans-serif`) and generated root spelling (the same value with a
trailing semicolon). Missing values return the canonical constant directly, and an explicitly blank
root font still emits no declaration. Custom, malformed and all other nonblank values retain their
previous precedence and full parsing/security path.

## Evidence

The preregistered experiment and raw outputs are under the ignored directory
`target/bench/experiments/theme-default-font-fastpath-20261019/`. Base library artifacts were built
from `d46ff33e9093b836cd181d65b2c0038cbff0efd9`; the candidate uses the same release profile,
features, toolchain, corpus and five-observation allocation harness.

All 178 public complete-render controls preserve SVG bytes, report bytes and ordered custom
measurement traces. Of 171 jointly stable memory rows, none regresses allocation count, allocated
bytes, peak growth or retained growth; 84 reduce allocation count and bytes, totaling 770 fewer
allocations and 32,433 fewer allocated bytes across those control rows. Seven variable Flowchart
rows remain inconclusive for memory.

All 418 catalog cells across 38 scenes, 35 logical families and 10 presets plus no-theme preserve
SVG bytes, reports, theme metadata and outcomes. All cells are stable; 122 reduce allocation count
and bytes, totaling 328 fewer allocations and 16,923 fewer allocated bytes across the catalog
rows. Peak growth and retained growth are unchanged throughout.

No calibrated latency run was performed. The improvement is limited to measured allocation count
and bytes plus the directly removed parser call for exact built-in defaults. It does not establish
an end-to-end speedup or close the v0.9.0 performance gap.

## Verification

- `cargo nextest run --locked -p merman-render --all-features`: 5,201 passed, 5 skipped.
- `cargo clippy --locked --release -p merman-render --all-features -- -D warnings`: passed.
- Release all-feature library build, all 178 public controls, all 418 catalog cells, formatting and
  diff checks: passed.
- Full strict verification was not rerun for this candidate. The preceding checkpoint's strict run
  completed the Rust feature matrix and artifact checks, then stopped because the local Mermaid CLI
  dependency `tools/mermaid-cli/node_modules/esbuild/package.json` is absent.
