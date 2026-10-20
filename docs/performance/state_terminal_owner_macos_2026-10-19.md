# State borrowed terminal paint (macOS, 2026-10-19)

State terminal paint now borrows the resolved compatibility scalars instead of materializing
another owned CSS value for every surface/property pair and cloning it again on each read.
The fixed selection covers all forty terminal surfaces and the same 104 valid combinations.
It retains sixteen missing combinations, their exact getter panic and their unowned status.
All classic, hand-drawn and Neo stylesheet surfaces remain available regardless of the current look.

The removed `BTreeMap` and owned selections were introduced by `6209fb12c` during typed theme
expansion. v0.9.0 used resolved scalar values directly. This source-backed ownership repair does
not identify State's entire release latency difference or establish equivalent release semantics.

The compatibility resolver still selects the same final values in the same order. Twenty-two
winner-source flags occupy a private `u32`; they come from the existing resolved winners, not
from CSS equality or a second configuration query. Two previously computed strings become plan
fields: the end-inner fill keeps its nullish `stateBorder` fallback, and Neo rough width keeps its
separate numeric conversion. Ordinary state borders remain truthy; StateNeo body width still uses
the classic width, composite Neo uses the nullish width and special/note Neo uses rough width.
Constants borrow static strings. Scoped gradients still format each diagram ID at the original
read point, while ordinary paint never formats an unused ID. Existing public consumers do not
mutate the scalars after construction; derived Clone retains independent values and source flags.

The experiment is registered under
`target/bench/experiments/state-terminal-owner-20261019/`, based on product `7faf5f9ae`.
Immutable native probes from the preceding Radar experiment preserve that baseline after builds
change the shared target directory. The original 262 controls are reused exactly. Another 140
State inputs were registered and captured before production edits, covering four looks, sixteen
configuration variants, absent/explicit typed themes, HTML labels and 0/1/16/64-state scales.
All 402 inputs pass the public preflight. Twenty-six earlier Radar parse-error controls remain
separate and also preserve their exact errors; they are not new State failure observations.

All 402 successful public controls preserve exact SVG/report bytes and ordered custom-measurer
traces. Among 262 jointly stable memory rows, ten improve and none regress in allocation count,
allocated bytes, peak growth or retained growth. Another 140 controls remain inconclusive for
memory: 134 new State controls and six existing Flowchart controls. They remain in the output/trace
comparison and were not rerun to select favorable samples.

| Complete public control | Allocations, before → after | Allocated bytes, before → after |
| --- | ---: | ---: |
| State tiny | 2,787 → 2,650 | 525,992 → 518,792 |
| State medium | 8,020 → 7,847 | 1,176,586 → 1,169,186 |
| Classic, one state | 3,572 → 3,429 | 761,827 → 754,595 |
| Classic, sixteen states | 11,679 → 11,491 | 1,833,380 → 1,825,893 |

Medium peak growth falls from 338,699 to 331,611 bytes; retained growth remains 60,272 bytes.
All 418 catalog cells across 35 logical families and ten presets plus no theme remain stable,
with exact SVG, reports, configuration, status and catalog metadata. The eleven State cells save
149–173 allocations and 7,376–8,088 allocated bytes each; the other 407 cells remain unchanged.
No stable peak or retained-growth metric regresses. Catalog and control probes use different
diagram IDs and retained reports, so compare absolute byte values only within the same lane.

Eight focused compatibility tests pass, including gradient Display/source ownership, missing
combinations, empty/whitespace borders and zero-width fallbacks. Renderer release nextest passes
5,206 tests with five skipped. Release all-feature Clippy, formatting and diff checks pass.
The full 37-entry DOM matrix passes structure, parity and parity-root. The descriptor-owned
optimized full browser WASM build, including wasm-opt, passes; package runtime smoke was not
repeated. Independent source review compared every mapping and source flag against the original
construction; independent raw-evidence review verified the measurements and build provenance.
Neither found a blocking regression. Commands and results remain in the ledger. This is exact
allocation/work removal; no latency improvement or v0.9.0 convergence claim is made.
