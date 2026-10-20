# Theme performance integration after main — October 20, 2026

The performance branch merges upstream `d249bb2dc35bedebfd4212310b96447ad33a08f5` as
`9b4ff208f7d7d327648e63cb4b2462469a16c9ed`. The merge has no textual conflicts. Independent
source review checks compiled initialization, host authority and dynamic detector epochs against
upstream's new theme designs and role-paint behavior. Renderer ownership and benchmark-protocol
reviews cover the final three-dot diff. This checkpoint records integration correctness, not a
new allocation experiment or release-latency comparison.

## Integration validation

The clean merged revision uses the existing shared target directory. Native commands run serially;
nextest compilation and test execution use four jobs/threads, while the remaining native stages
use one Cargo job. Validation uses the repository's pinned Rust 1.95.0 on native aarch64 macOS.

| Owner check | Result |
| --- | --- |
| Core, facade and renderer, all-features release nextest | 7651 pass; seven skipped |
| Matching all-features release Clippy | Pass with `-D warnings` |
| Full SVG DOM matrix | 37 entries pass structure, parity and parity-root under the unchanged residual policy |
| Optimized full WASM | Pass, including `wasm-opt`; transaction `e93820d8194e` |
| Format, diff and indexed path lengths | Pass; 160-unit path budget |
| Performance baseline, memory, memory-driver and performance contracts | 16 + 51 + 29 + 149 = 245 pass |
| Theme acceptance, release PNG/all-diagrams/Cytoscape profile | 34 pass; none skipped |
| Package/public API acceptance boundary | Pass; production APIs compile, two private and six retired imports remain unavailable |
| Complete-feature compiled pipeline benchmark discovery | 459 benches / 459 receipts / seven lane groups pass |

The first 11-stage runner reaches its real exit 0 before the additional three-stage runner starts.
The additional runner also reaches its real exit 0 after all three planned stages. No native stages overlap. All 37 current DOM reports are
copied into the ignored integration directory with size, SHA-256 and freshness checks. Independent
WASM review rehashes all 1211 listed build inputs and all six artifacts. A WASM build does not establish
browser runtime smoke. Linux workspace tests, platform consumers, browser containment and remaining
CI-selected feature recipes retain their existing CI owners.

## Performance identity and limits

The adjacent allocation experiments retain their original revisions, input identities and limits:

- [Compiled initialization](compiled_theme_initialization_macos_2026-10-20.md) saves 2695 allocations
  and 602095 allocated bytes for warmed Kanban / Editor Light. Its bounded per-plan retention
  increases candidate-minus-base absolute process-live memory by 175285–284624 bytes; that primary's
  absolute process peak increases 35463 bytes. It is not a cold-start or total-memory reduction.
- [Literal XHTML](xhtml_plain_paragraph_macos_2026-10-20.md) saves 84 allocations and 13849 allocated
  bytes for the no-theme Kanban primary by proving one narrow paragraph shape before DOM parsing.
- [Empty explicit-owner queries](empty_explicit_owner_query_macos_2026-10-20.md) save 40 allocations
  and 1344 bytes for no-theme Flowchart; 371 of 418 stable catalog cells improve and 47 retain all
  four operation-growth metrics. Compatibility-owner predicates still execute.

The latest formal [v0.9.0 confirmation](theme_v090_post_initialization_confirmation_macos_2026-10-20.md)
measures clean `a6742779c`, before the empty-query guard and this merge. Its 19 same-SVG scenes
produce 15 confirmed non-regressions, three regressions and one inconclusive scene. The remaining
regressions are ZenUML (+17.18%, +3.443 us), Packet (+27.18%, +2.203 us) and Kanban (+18.09%,
+8.198 us); Class tiny's relative confidence bounds cross 10%. These are historical release-range
observations, not measurements of `9b4ff208f` or causal latency evidence for any individual repair.
Typed request API and output-changing scenes retain their separate evidence boundaries.

## Continuing the work

Future adjacent measurements must use current public theme inputs. The inherited private harness
imports the removed `compile_theme_definition_json`; its 22 authored documents and preset export
metadata also target the retired definition format. Source-only preparation preserves the old
artifacts and identifies the affected 276 site controls and 13 themed error controls. Migrate those
specific controls to the current design/complete-spec contract, record any retired/new-control
mapping, and freeze a new registration before capturing a baseline. Do not silently drop controls,
add a compatibility API or treat an old input's rejection as a performance regression.

The next scoped-appearance candidate only constructs a dotted path when its consumer exists.
It remains an unapplied source hypothesis until the migrated adjacent harness is admitted.
The all-family convergence goal remains incomplete. The PR is an early review draft while
cleanup and cross-platform qualification continue.

The ignored `target/bench/experiments/theme-main-integration-20261020/` retains exact commands,
logs, terminal receipts, archived DOM reports and independent reviews. Most earlier allocation
raw records also remain ignored; formal latency reports and plans have separate checked-in
compressed evidence. This checkpoint does not claim that all raw allocation artifacts ship in PR.
