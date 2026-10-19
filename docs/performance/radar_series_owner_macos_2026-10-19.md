# Radar single series-palette owner (macOS, 2026-10-19)

Radar now constructs its twelve series colors directly in the final series-paint plan. The CSS
binding no longer owns another twelve-color array whose sole production consumer cloned it into
that plan. Every CSS slot remains present, including on diagrams with zero curves. No global
cache, input classifier, sanitizer change or theme-only bypass is introduced.

This removes duplicated ownership from theme preparation: v0.9.0 emitted the single palette owned
by `RadarTheme`, whereas `6ff15c63d` introduced the later binding-to-plan clone. It does not prove
that this duplication explains Radar's entire release latency difference. The same configuration
string projection and fallback now run in the series plan, after typography and before layout.
Typed ordinal selection, explicit source ownership, the fixed twelve-slot residual, terminal
receipts, work accounting, measurement, cancellation and output limits keep their existing paths.

The registered allocation experiment is under
`target/bench/experiments/radar-series-owner-20261019/`, based on product `c7f64db2b`. Its native
production tree equals `2c029f124`; only documentation and Dependabot configuration differ.
The immutable TreeView candidate probes provide the original 231 controls and 418-cell catalog
baseline. Additional Radar inputs were registered and captured from those same frozen executables
before production edits. The candidate renderer library and linked probe executables differ from their baselines;
all runtime library hashes are recorded. The allocator and probe sources remain unchanged.

The input preflight retains 26 public `theme_color` errors separately: values such as `false`, an
empty string and `var(--series)` can be rejected during configuration theme preprocessing before
Radar preparation. They are not rendered samples. The initial supplementary allocation attempt
stopped at the first such error; its correction and observations remain recorded before candidate
measurement. A direct plan test separately preserves raw strings, empty strings and string-only
fallback below that public preprocessing boundary.

All 262 successful controls preserve exact SVG/report bytes and ordered custom-measurer traces.
All 26 public error controls retain identical errors. Among 256 jointly stable memory rows,
32 improve and none regress in allocation count, allocated bytes, peak growth or retained growth.
Six variable Flowchart rows remain inconclusive for memory; their outputs and traces still match.
Controls include zero, one, twelve and thirteen curves; hidden legends; empty requested recipes;
typed palettes; explicit default-valued and scoped configuration; and `THEME_COLOR_LIMIT` variants.

| Complete public control | Allocations, before → after | Allocated bytes, before → after |
| --- | ---: | ---: |
| Radar medium | 301 → 289 | 39,524 → 38,882 |
| Zero curves | 174 → 162 | 30,606 → 29,964 |
| Twelve curves, no theme | 902 → 890 | 97,129 → 96,487 |
| Thirteen curves, no theme | 961 → 949 | 99,736 → 99,094 |

Medium peak growth falls from 18,911 to 18,269 bytes; retained growth remains 7,599 bytes.
All 418 catalog cells across 35 logical families and ten presets plus no theme remain stable and
preserve exact SVG, reports, configuration, status and catalog metadata. The eleven Radar cells
each save twelve allocations and 372–766 allocated bytes, depending on their palette strings;
other cells remain unchanged. Absolute
byte values differ between catalog and control probes because they use different diagram IDs and
retained reports; compare each lane only with its own baseline.

Focused Radar tests pass 65 cases; renderer release nextest passes 5,203 with five skipped.
Release all-feature Clippy, formatting and diff checks pass. The full 37-entry DOM matrix passes
structure, parity and parity-root. The descriptor-owned optimized full browser WASM build,
including wasm-opt, passes; package runtime smoke was not repeated. Independent source and
raw-evidence reviews found no blocking regression or provenance gap. Commands and results remain
in the ledger. This is exact allocation/work-removal evidence, not a latency improvement or
v0.9.0 convergence claim.
