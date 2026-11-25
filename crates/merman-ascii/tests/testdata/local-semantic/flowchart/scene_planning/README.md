# Scene planning acceptance corpus

These original local diagnostic inputs freeze authored facts, not reference artwork. Their
bytes, node identities, labels, edge order, endpoint markers, and strokes are checked by
[`scene_planning_corpus.rs`](../../../../scene_planning_corpus.rs). No seeds are regenerated.

| Input | Nodes / direct edges | Directed five-node cycles / forward links | SHA-256 |
| --- | --- | --- | --- |
| `dense-work-budget.mmd` | 100 / 195 | 20 / 95 | `c39460ae177007d7d0bab13d5fc9ce37a9178b876d1d0655ac7f1bf4675bbc0f` |
| `dense-minimized.mmd` | 20 / 33 | 4 / 13 | `51e61fbd87adea209d4314f64baa39a6030ed84475c22b47b7db92f180425ad7` |

The original was promoted from `target/trident-review/dense-work-budget.mmd`, preserving its
CRLF bytes and absent final newline through the local `.gitattributes`. The reduction came from
`target/bench/experiments/ascii-channel-indices/dense-diagnosis/dense-minimized.mmd` with LF and a
final newline. It retains `C0..C19` and their original edges except `C0 -> C5` and `C1 -> C6`.
Neither declarations nor retained edges were reordered; no labels or directions were changed.
CI uses only the promoted inputs, not those machine-local paths or a reference runtime.

## Spatial qualification

Both exact inputs must produce complete Canonical/Strict spatial primaries under the unchanged
Trusted Native resource profile, in ASCII and Unicode. At a 120-column Allow viewport they must
fit the width. The original must use at most 300 rows; the reduced source keeps its narrower
capacity-recovered layout within 120 rows. After a routing miss with no further measured capacity
growth, the planner may assign one main-axis level to each strongly connected component; every
member remains a separate node and every original relationship is routed and validated. Component
analysis and the new generation share the existing work ledger and finite rebuild allowances.

The private generated-scene owner additionally runs both exact sources in TD and LR, ASCII and
Unicode, through the full planner and its independent information oracle. It requires all 195 or
33 relationships, their actual endpoint contacts, markers, continuity, and final paint. These
constructive successes replace the earlier accepted routing-miss outcomes; no fixture bytes,
labels, directions, or edges are shortened or omitted. One-unit work controls still require typed
resource errors for both inputs.

`labelled-cycle-capacity.mmd` and `narrow-cycle-capacity.mmd` are eight-node controls for recovery
priority. Both must retain complete Strict spatial results at a 50-column Error viewport in ASCII
and Unicode. Endpoint/corridor and return-lane growth take precedence over the component candidate;
the labelled source must keep all six edge labels. The independent oracle additionally checks every
relationship and its final paint for both sources in TD and both character sets.

`issue_370_terminal_height.mmd` reproduces the terminal height regression.
With Fixed routing space it keeps all ten relationships, seven edge labels, and the cylinder lid
within 33 rows at a two-cell rank gap. The independent oracle checks the endpoint contacts,
markers, and painted routes in both character sets. [#370](https://github.com/Latias94/merman/issues/370)

## Reading worksheet and existing owners

Review the complete emitted document before consulting the source. An adaptive Plain request
at 80 columns must preserve every fact under the existing Trusted Native work ceiling. Strict
results remain separate. The frozen scenes now have a spatial acceptance bound; no dense reference
output is qualified and arbitrary larger graphs are not promised the same reading floor.

| Question | Required answer |
| --- | --- |
| Who connects directly to whom? | Exactly the ordered 195 or 33 authored endpoint pairs; reachability and a cycle-summary node do not replace direct relationships. |
| Which direction and multiplicity? | Authored TD, one directed relationship per pair, all 20 or 4 five-node cycles; no self-loops, reciprocal duplicates, or invented reverse edges. |
| Which identities and complete labels? | Every `Ci` remains individually identifiable, with its complete identical label: `C0..C99` or `C0..C19`. Preserve distinguishing numeric suffixes. |
| Which conditions, markers, and membership? | No edge conditions or groups; each edge has an unmarked source, pointed target, and normal visible stroke. Do not invent conditions or containment. |
| How much reading work? | Measure complete width/rows including every detail, longest tracing span, endpoint pairing, and detail lookups after correctness. A missing document or truncated label cannot set a smaller floor. |

The thirteen existing sources, qualified per-case dimensions, 60/80/100/120-cell outcomes, both
encodings, and both width profiles stay with `tests/presentation_floor.rs` and
`tests/testdata/local-semantic/flowchart/presentation/README.md`. `tests/graph_layout_quality.rs`
owns LR/RL twelve-way fans (at most 48 rows), private conditions, and complete suffixes.
`tests/graph_family_viewports.rs` retains its admitted State/Swimlane subsets. The maintained
24-input reading-review scope and individual reference disqualifications remain in
`ASCII_REFERENCE_COMPARISON.md`; a failed or ambiguous reference result remains unqualified.
Grok is presentation research only, under Apache-2.0; no implementation is copied here.

The independent oracle in `src/graph/routing/tests/generated.rs` still owns all 486 attempts and
3,042 visible edge contributions, including failures: nine families, fixed seed/size pairs
`(0x19a47c21, 2)`, `(0x73b9025d, 5)`, `(0xd061e8af, 8)`, two directions, three modes, and three
edge orders. Keep the shuffled-order salt `0xa81b2047`, spacing variants, independent continuity
checks, and every prior success. These fixtures neither duplicate nor resample that harness.
Separate recovery tests add eight exact dense-source scenes and four capacity-priority controls
through the full planner and the same independent oracle; they do not alter the original attempts
or success floor.

## Constructive private-test proposals

These fixed geometries extend the existing private repair/capacity tests without public hooks.
Coordinates are terminal cells; node rectangles are `(x, y, width, height)`, path endpoints are
berths immediately outside contacts. They require admission through the private owners before
being called renderer successes; neither proposal is inferred from dense-input failure.

**Four necessary contributors.** Use rectangular wall strips covering inclusive `x=20..100`
at `y=40..42` and `y=48..50`, capped at `x=20..22` and `x=98..100`, `y=43..47`. Split each strip
into nonoverlapping filler rectangles and four endpoint rectangles centered at
`x_i = 42 + 10*i`, `i=0..3`: top `(x_i-1,40,3,3)`, bottom `(x_i-1,48,3,3)`.
Put `s=(28,44,3,3)` and `t=(90,44,3,3)` inside. Each incumbent top-to-bottom edge follows
`(x_i,43)..(x_i,47)` and has a distinct three-line condition (`a<br>a<br>a`, then `b`, `c`, `d`
likewise) at `(x_i-2,44)`. The condition reservation seals all three usable corridor rows
`44..46`; the walls' reading margins seal rows 43 and 47. Any retained contributor still cuts
the entire corridor, so removing at most three cannot open `s -> t`.

An independent complete alternative routes `s -> t` along `(31,45)..(89,45)`. Replace each
incumbent by the polyline `(x_i,39)`, `(x_i,38-i)`, `(16-4*i,38-i)`, `(16-4*i,52+i)`,
`(x_i,52+i)`, `(x_i,51)`, with its unchanged condition at `(14-4*i,44)`. The four exterior
detours are nested and disjoint, contact the same endpoints from above/below, and leave one
blank column between adjacent label reservations and routes. Independent cell checks found
all 15 proper removal subsets blocked, but all-four removal open, and the five final routes
disjoint from unrelated node margins and complete label reservations. This is geometry
evidence, not private renderer admission. All coordinates fit `x=0..106`,
`y=0..56`, including the current six-cell search margin. Admission must check labels, markers,
each proper subset, the final complete scene, and atomic rollback; no budget increase is proof.

**Capacity-driven reflow.** Use 97 parallel edges with independent source-circle markers and
open targets, no labels or shared allocation. A `ForkJoinHorizontal` source at `(8,8,7,3)` has
12 admissible contacts and an upper bound of `12 * 8 = 96` markers, so the existing preflight
must report `PortCapacityExceeded { required: 97, available: 96 }`. Preserve the fork/join shape
and endpoints while widening it to `(8,8,293,3)` and a rectangular target to `(8,24,293,3)`.
For `i=0..96`, use the independent vertical route `(10+3*i,11)..(10+3*i,23)`, with its source
circle at the first berth. The 97 distinct contacts and three-column spacing need no shared
marker or trunk. Independent cell checks confirm disjoint paths and 97 distinct source berths.
A larger capacity upper bound alone does not establish success: admit every
route/marker, preserve multiplicity and labels, and independently validate the complete scene.
