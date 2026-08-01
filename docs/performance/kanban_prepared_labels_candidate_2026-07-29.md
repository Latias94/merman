# Kanban Prepared Labels U5 Candidate Receipt

Date: 2026-07-29

Status: accepted. Kanban retains the private operation-scoped prepared-label artifact.

## Compared identities

- Adjacent base commit: `1d63259af3df9d3292fbfb38760baeaa9b0efb67`.
- Accepted candidate commit: `19da80418dd0ad47a87ff7f7a99b5d3a7ccf76af`.
- Host: macOS arm64, Apple M4 Pro.
- Public lane: existing Criterion `end_to_end/kanban_medium`.
- Workload: the committed three-section, four-card `kanban_medium.mmd` fixture.
- Base executable SHA-256: `b851980aab6f15ac8a26145e44f357ead437bafbf3edc992c557370717e1c4ff`.
- Candidate executable SHA-256: `d1902b22e76cff6cc7c50280b34aa1bb51e41f19437ec1e17600cbbc11e4876e`.

No benchmark lane, fixture, runner, script, test function, dependency, or public API was added.

## Design

Layout now owns one sanitized XHTML fragment and one compact geometry record for every section and
card title. SVG emission consumes those values instead of repeating Markdown conversion,
sanitization, raw measurement, and optional wrapped measurement. Ticket and assigned labels remain
SVG-local because they do not affect layout; each is measured once and the result is reused for
positioning and emission.

The public `KanbanDiagramLayout` projection is unchanged. There is no global cache, source-text
copy, cross-family abstraction, or retained ticket/assigned plan. Each retained geometry contains
only two `f64` values and one `bool`, vectors reserve their known cardinality, and retained XHTML is
linear in the already-admitted section/title text for the operation.

## Low-latency gate

The frozen base A/A estimate was 51,313.56 ns. Eight independent A/A pairs on each executable were
stable. The largest simultaneous 95% identity/order endpoints were 0.01361 log units and 685.64 ns,
below the stability limits of 0.05129 and 1,000 ns. The registered formulas therefore produced:

```text
T_r = 0.10536052 (10.00% reduction)
T_d = 5,131.36 ns
required pairs = 8
```

Eight fresh balanced AB/BA pairs were then collected with 20 Criterion samples, one-second warmup,
one-second measurement, and 10,000 resamples per observation:

| Pair | Order | Base ns | Candidate ns |
|---:|:---:|---:|---:|
| 1 | BH | 51,203 | 40,849 |
| 2 | HB | 51,026 | 40,166 |
| 3 | BH | 51,548 | 40,696 |
| 4 | HB | 50,900 | 40,413 |
| 5 | BH | 51,895 | 40,475 |
| 6 | HB | 51,980 | 40,973 |
| 7 | BH | 52,478 | 40,803 |
| 8 | HB | 52,209 | 41,306 |

| Base mean | Candidate mean | Mean delta | Relative delta | Simultaneous one-sided 95% upper bounds | Result |
|---:|---:|---:|---:|---:|---|
| 51,654.88 ns | 40,710.12 ns | -10,944.75 ns | -21.19% | -10,668.98 ns; -20.75% | Confirmed improvement |

Both upper bounds clear the registered absolute and relative thresholds. Diagnostic stage readings
on the final binary were about 10.17 us for prepare/layout, 23.73 us for SVG emission, and 41.40 us
end to end; the improvement was not created by moving detail measurement into layout.

## Semantics and complexity

The existing host-measurement test now proves that SVG emission performs exactly one retained Wrap
measurement for each non-empty ticket and assigned label. An existing DOM-id test also covers the
Mermaid-compatible zero-sized foreignObject for an empty card title. The other Kanban Markdown,
sanitizer, wrapping, Look, viewport, theme, and canonical layout/SVG tests were reused unchanged.

Verification:

```text
CARGO_BUILD_JOBS=1 cargo +1.95.0 check -p merman-render
CARGO_BUILD_JOBS=1 cargo +1.95.0 nextest run -p merman-render -E 'test(/kanban/)' --test-threads=1
22 passed; 0 failed
```

The accepted implementation removes the SVG-side Markdown/sanitizer path and duplicate label
measurement helper. The four-file change is `+298/-194`; most additions are the private artifact
and a test-only adapter for existing handcrafted-layout tests. The candidate benchmark executable
is 720 bytes larger than the base. The latency, bounded-state, semantic, host-callback, artifact,
and complexity gates pass.
