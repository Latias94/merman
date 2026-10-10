# XY Chart optional paint receipt storage

This is a structural memory repair against `2fa9063d5`, not a latency benchmark.
`XyChartPaintPlan::resolve` previously copied every recognized Text, Path, and enabled
rectangle DataLabel into a `BTreeMap` before accounting could determine whether any
paint rule needed a terminal receipt. Source-owned or entirely inapplicable requests
therefore retained payloads that `begin_terminal_receipt` would never use.

The paint and accounting pass now finishes first. A pending receipt triggers one
read-only capture pass over the final layout. Without pending paint, the plan retains
no terminal entries and the XML escape callback is never invoked. Source ownership,
layout paint values, resource charges, and final evidence dispositions remain unchanged.

Let N be recognized terminals and B their total content and paint string bytes. For
requests without pending paint, terminal storage changes from O(N + B) to O(1), with
no additional retained index. Requests with pending paint retain O(N + B) and perform
an additional layout traversal; the existing escaped receipt copy remains. This change
does not claim lower latency or lower peak memory for those pending requests.

The checked-in scale regression uses 1, 64, and 1024 long text terminals. Each contains
6144 text bytes and the five-byte source-owned paint `black`. The removed payload size
below is derived from the old exact clone operations and excludes BTreeMap overhead:

| Terminals | Previous copied content/paint bytes | Current retained terminals |
| --- | --- | --- |
| 1 | 6149 | 0 |
| 64 | 393536 | 0 |
| 1024 | 6296576 | 0 |

The same inputs with typed paint must still retain N terminal expectations. Existing
writer mutation tests cover Text, Path, and DataLabel, including escaped content,
missing declarations, duplicate terminals, and foreign receipt owners. The zero-size
DataLabel case also retains its NotApplicable result without storing an unused payload.

Validation on macOS ARM64:

- Four plan and writer receipt tests passed, including the three-scale storage check.
- All 28 XY Chart SVG integration tests passed.
- `cargo clippy --locked -p merman-render --lib --no-default-features --no-deps` and
  `cargo fmt --all --check` passed. Clippy reported existing crate warnings; none referenced
  the changed XY Chart paint file.

The experiment registration is under the ignored
`target/bench/experiments/xychart-optional-receipt/experiment.yaml` directory.
This does not retire further legacy routes or change public support declarations.
