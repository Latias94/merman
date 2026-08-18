# U2 Terminal Cell Representation Prototype Results

Date: 2026-08-10
Revision: `98c7651ed` plus the primary-width working tree on `refactor/ascii-semantic-depth`
Host: macOS Darwin 25.6.0, Apple M4, 10 logical CPUs, `rustc 1.95.0`

## Scope

This is a private architecture gate for U2. It compares three representations on the same frozen
logical workloads:

1. `current_scalar`: a frozen copy of the pre-U2 scalar cell layout, with one scalar and a boolean
   continuation marker. It reuses the production `CanvasStyle` type but cannot drift with later
   `TerminalCell` refactors.
2. `compact_arena`: an 8-byte typed scalar/arena/continuation token, with complex graphemes copied
   into one append-only UTF-8 backing string and referenced by compact `(start, len)` entries.
3. `compact_interned`: the same typed token with a local `Arc<str>` arena and a HashMap interning
   index. The index is rebuildable and is intentionally omitted from `Clone` and consuming mirror
   results because it is an insertion aid, not glyph ownership.

The benchmark receives pre-segmented positive-width graphemes. Unicode segmentation and safe-text
normalization are deliberately outside this microbenchmark so the result attributes cost to cell
storage, ownership, and finalization. Width values are computed with the pinned `unicode-width
0.2.2` implementation during workload preparation.

Each workload contains 1,024 graphemes:

| Workload | Description | Scalar cells | Grapheme cells | Complex occurrences | Distinct complex |
|---|---|---:|---:|---:|---:|
| `ascii` | ASCII single-scalar text | 1,024 | 1,024 | 0 | 0 |
| `cjk` | CJK single-scalar wide text | 2,048 | 2,048 | 0 | 0 |
| `emoji_repeated` | repeated combining, ZWJ, modifier, VS, and flag graphemes | 3,073 | 1,843 | 1,024 | 5 |
| `complex_unique` | 1,024 distinct combining-mark graphemes | 3,072 | 1,024 | 1,024 | 1,024 |

The scalar mirror result is intentionally marked incorrect for the two complex workloads: it
reverses scalars instead of grapheme clusters. Its complex timing is diagnostic evidence only and
is not a correctness admission comparison.

## Layout evidence

The benchmark freezes the complete pre-U2 scalar cell shape and reuses the current production
`CanvasStyle`, whose size dominates both the old and selected cell layouts.

| Type | Size | Alignment |
|---|---:|---:|
| Frozen pre-U2 scalar cell | 40 B | 8 B |
| `CanvasStyle` | 32 B | 8 B |
| Typed glyph token | 8 B | 4 B |
| Candidate packed cell | 40 B | 8 B |
| `GlyphSlice` | 8 B | 4 B |
| `Arc<str>` | 16 B | 8 B |
| `HashMap<Arc<str>, u32>` header | 48 B | 8 B |

The important result is that reducing a token from 8 to 4 bytes does not reduce the complete cell:
`CanvasStyle` and alignment dominate. The first exploratory manually tagged-`u32` token was
therefore rejected; it made scalar finalization slower without changing cell size. The retained
candidate uses the clearer typed token.

## Allocation evidence

The existing native system allocator probe was reused through a benchmark-only `#[path]` include.
The operation result remains live when the measurement closes, so `peak_growth` captures temporary
and retained operation-owned memory. No custom allocator framework was added.

Selected paint measurements:

| Workload | Current scalar | Compact arena | Compact interned |
|---|---:|---:|---:|
| ASCII: allocations / bytes | 1 / 40,960 | 1 / 40,960 | 1 / 40,960 |
| CJK: allocations / bytes | 1 / 81,920 | 1 / 81,920 | 1 / 81,920 |
| Repeated emoji: allocations / bytes | 1 / 122,920 | 4 / 89,324 | 8 / 74,136 |
| Unique complex: allocations / bytes | 1 / 122,880 | 4 / 54,312 | 1,027 / 133,128 |

The scalar fast path creates no arena entries and no per-glyph allocations in both candidates. The
append-only arena avoids the per-occurrence allocation problem of a naive `Vec<Arc<str>>` arena. The
interner saves bytes for five repeated complex values, but its high-cardinality behavior is worse:
1,027 allocations and 133 KiB for 1,024 distinct values.

Selected clone and composition measurements:

| Workload / operation | Current scalar | Compact arena | Compact interned |
|---|---:|---:|---:|
| Repeated emoji: clone allocations / bytes | 1 / 122,920 | 2 / 81,912 | 2 / 73,800 |
| Unique complex: clone allocations / bytes | 1 / 122,880 | 2 / 49,152 | 2 / 57,344 |
| Repeated emoji: compose allocations / bytes | 1 / 122,920 | 3 / 86,016 | 4 / 74,028 |
| Unique complex: compose allocations / bytes | 1 / 122,880 | 3 / 53,256 | 4 / 112,648 |

The lazy intern index removes the pathological HashMap clone cost. It does not remove the cost of
cross-surface deduplication when the target is a different arena; that cost remains visible in the
unique-complex composition row.

## Criterion timing evidence

Values below are mean point estimates from the 20-sample exploratory run, in microseconds per
operation over 1,024 logical graphemes. The focused CJK `finalize` and CJK `compose` controls were
rerun with 50 samples; those reruns confirmed the conclusions and reduced the observed compose
noise.

Each cell is `current scalar / compact arena / compact interned`:

| Operation | ASCII | CJK | Repeated emoji | Unique complex |
|---|---:|---:|---:|---:|
| Paint | 1.703 / 0.946 / 0.919 | 2.418 / 2.011 / 2.038 | 3.143 / 4.063 / 10.863 | 3.313 / 3.417 / 41.021 |
| Clone | 0.650 / 0.662 / 0.641 | 1.308 / 1.301 / 1.342 | 1.922 / 1.191 / 1.195 | 1.885 / 0.706 / 2.139 |
| Finalize | 0.842 / 0.853 / 0.845 | 1.423 / 1.683 / 1.658 | 2.796 / 3.576 / 2.831 | 3.031 / 3.260 / 3.391 |
| Mirror | 1.983 / 1.887 / 1.861 | 3.595 / 3.251 / 3.350 | 5.822 / 3.095 / 3.074 | 6.355 / 1.975 / 1.870 |
| Compose | 0.671 / 0.679 / 0.652 | 1.310 / 1.305 / 1.293 | 1.900 / 6.897 / 1.432 | 1.884 / 5.259 / 21.680 |

Interpretation:

- On ASCII and CJK paint, both correct candidates are at least as fast as the current scalar
  baseline. The append-only arena has no scalar-path arena work.
- On repeated complex text, the interner is substantially faster than the append-only arena for
  paint and composition because five values are reused. Its clone and mirror are also cheap after
  making the index lazy.
- On high-cardinality complex text, the append-only arena is the stable choice: it is much faster
  to paint, clone, and compose, and uses less memory. The interner's HashMap work is not free.
- Correct grapheme mirror is much cheaper than scalar mirror on complex workloads because the
  candidate surface owns 1,843/1,024 cells instead of 3,073/3,072 scalar cells. The scalar result
  is not semantically admissible there.
- The exploratory cell-by-cell run previously showed an approximately 18% CJK finalize penalty.
  The production-shaped primary-run encoder was rerun below and removes that penalty.

## Primary-run rerun

The preregistered primary gate was rerun on 2026-08-10 with 50 samples, one second of warm-up,
and two seconds of measurement on the same host. Mean times are microseconds per operation over
1,024 logical graphemes:

| Workload | Current scalar | Compact arena | Compact interned | Compact arena vs scalar |
|---|---:|---:|---:|---:|
| ASCII | 0.797 | 0.801 | 0.785 | -0.5% |
| CJK | 1.340 | 1.245 | 1.197 | -7.1% |
| Repeated emoji | 2.625 | 2.948 | 2.600 | +12.3% / -0.9% interned |
| Unique complex | 2.924 | 3.117 | 2.575 | +6.6% / -11.9% interned |

The repeated-emoji row is a diagnostic comparison: the selected append-only arena is intentionally
optimized for bounded high-cardinality ownership, while the optional lazy interner wins when five
complex values repeat. The mandatory scalar/CJK primary gate passes: ASCII improves and CJK remains
7.1% faster than the frozen scalar baseline, below the preregistered 10% regression ceiling.

## Gate decision

The full U2 representation and primary finalize timing gates are **passed**. Cross-surface compose
and workload-specific interning remain diagnostic evidence, not a blanket public latency claim.

### Selected production direction

Use the typed scalar-or-arena token with an append-only local UTF-8 arena as the default ownership
model:

- scalar and single-scalar CJK remain allocation-free per glyph and have no arena entries;
- complex text is stored once in a bounded backing arena with compact ranges;
- cells remain 40 bytes in the current style layout, so no artificial bit-packing is required;
- clone and mirror share/move the backing arena instead of cloning each complex string;
- composition uses an explicit remap table when surfaces have different arena ownership;
- an optional lazy interning index may be added behind the arena builder only after a workload-based
  decision. It must not be part of the mandatory cell ownership or clone contract.

The always-on `compact_interned` alternative is rejected as the default because its high-cardinality
paint and cross-surface composition costs are disproportionate. It remains a useful optional
optimization for repeated complex labels, subject to U3 grapheme/arena budgets and a measured
duplicate-rate policy.

### Threshold evidence

| Gate | Result |
|---|---|
| Complete cell size no larger than current | Pass: 40 B vs 40 B |
| Scalar ASCII/CJK arena entries | Pass: zero |
| Scalar paint/clone/mirror/empty composition | Pass or within noise after exact no-arena fast path |
| Scalar finalization provisional 10% relative gate | Pass: ASCII improves; CJK is 7.1% faster |
| Complex grapheme correctness | Pass for both candidates; current scalar rejected for mirror |
| Complex memory boundedness | Append arena passes the practical comparison; always-on interner loses on unique values |
| Worst-case cross-surface composition | Diagnostic residual; shared-arena optimization remains future work |

The CJK finalize miss is a private-stage attribution result, not a public renderer regression claim.
The absolute difference is small, but U2 should retain a primary-grapheme/run encoder seam so the
production implementation can remove the continuation scan and rerun this gate.

## Historical evidence

This report is a frozen U2 decision record from revision `98c7651ed` plus the working-tree state
identified above. The plan-owned benchmark and its rejected candidate implementations were removed
after the representation decision; they are not executable from the current tree and must not be
restored as standing test infrastructure.

The measured source formerly lived at
`crates/merman-ascii/benches/terminal_cell_representation.rs` and its `prototype.rs` module. Raw
Criterion and allocator logs were local diagnostic artifacts rather than durable release evidence.
The tables in this document preserve the accepted architectural observation only; current public
renderer performance claims require the canonical pipeline benchmark and a current closeout
receipt.
