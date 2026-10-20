# C4 consumer-owned color preparation (macOS, 2026-10-19)

C4 now prepares element colors when binding actual layout shapes. The existing per-type style
entry retains the three resolved colors for reuse within that family artifact. Typography still
prepares every built-in type and observed custom type because the stylesheet consumes their fonts.
Unused types no longer format color keys, read configuration, or allocate color values. No new
collection, global cache, resource-policy exception, or validation bypass is introduced.

This removes work introduced by theme preparation: v0.9.0 selected fallback colors inside the
actual-shape emission loop, while the pre-change typography plan prepared colors for all types.
The change preserves the current source-binding contract: the last source metadata wins for an
alias, but defaults come from the actual layout type. Explicit empty colors, non-string fallback,
custom/empty types, missing-source-before-missing-style errors, fonts, measurement calls, work
accounting and cancellation checkpoints retain their owners and order.

The allocation experiment is registered under
`target/bench/experiments/c4-terminal-palette-20261019/`, based on `458268867`. Synchronizing
`origin/main` at `17d1773f2` added only the PR #212 merge commit; its tree equals the previous
integration revision. Frozen pre-edit probes remain the comparison baseline. Optimized library
receipts and linked library/executable digests identify both measured products.

All 206 public controls preserve exact SVG/report bytes and ordered custom-measurer traces.
Across 200 jointly stable rows, seven improve and none regress in allocation count, allocated
bytes, peak growth or retained growth. Six variable Flowchart rows remain inconclusive for memory.
The controls include zero, one, 16 and 64 repeated C4 shapes; all 20 built-in types; configured
colors; and explicit empty colors. All-type controls retain their original memory metrics.

| Public control | Allocations, before → after | Allocated bytes, before → after |
| --- | ---: | ---: |
| C4 medium | 2,129 → 1,985 | 245,002 → 242,227 |
| Zero shapes, title only | 664 → 484 | 130,623 → 127,438 |
| One repeated type, 64 shapes | 15,096 → 14,925 | 1,236,649 → 1,233,561 |

C4 medium peak growth falls from 96,224 to 95,888 bytes; retained growth stays at 38,995 bytes.
All 418 catalog cells across 35 logical families and ten presets plus no theme preserve exact
SVG, reports, configuration, status and catalog metadata. Every cell is stable. The eleven C4
cells each save 144 allocations and 2,775 allocated bytes; other cells are unchanged. No peak or
retained-growth metric regresses. Absolute byte metrics differ between the catalog and control
probes because they use different diagram IDs and retained reports; compare each with its own base.

The first candidate used a temporary map of borrowed per-type colors. It improved most C4 cases
but added four allocations for the all-types/empty-colors control, violating the registered
non-regression gate. Its patch and measurements remain recorded as rejected. The accepted
candidate reuses existing entries and passes that control without adding allocations.

Renderer nextest passes 5,201 tests with five skipped; four public C4 tests pass. Release
all-feature Clippy, formatting and diff checks pass. The full 37-entry DOM matrix passes structure,
parity and parity-root modes. The descriptor-owned full browser WASM build, including wasm-opt,
passes from this worktree; package runtime smoke was not repeated for this candidate. Independent
source review found no reproducible semantic regression. Commands and logs remain in the ledger.
This is allocation/work-removal evidence; neither a calibrated latency improvement nor convergence
to v0.9.0 is claimed.
