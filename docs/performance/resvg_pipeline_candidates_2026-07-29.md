# Resvg Pipeline U8 Candidate Receipt

Date: 2026-07-29

Status: two adversarial-complexity fixes accepted; one-shot export fusion and single-reader terminal
validation rejected at discovery. No rejected production candidate remains.

## Scope and environment

- Host: macOS 26.5.1 arm64, Apple M4 Pro.
- Toolchain: Rust 1.95.0 (`59807616e1fa2540724bfbac14d7976d7e4a3860`).
- Lock SHA-256: `223444c1a2385f4807766f71d95de503dbcae0380ee05165760b1b437708e711`.
- Fixture: existing `kanban_medium.mmd`, SHA-256
  `43382cbc1379cf4c8ff7399915269340c350b406ff8cee53838d407392c95689`.
- Build: `cargo +1.95.0 bench -p merman --features png --bench pipeline --no-run`, default
  optimized bench profile, no ambient Rust flags, one Cargo job, shared `target`.

Two lanes were added to the existing Criterion executable: one complete public PNG operation and
one complete public ResvgSafe sealed-SVG operation. Both reuse the existing fixture. No runner,
script, fixture, dependency, memory harness, or benchmark crate was added.

## Accepted complexity repairs

### Duplicate parsed-ID references

Commit `cc54e72b405352dba96e4d3c85fdd4e88847ba07` replaces repeated materialization of
every duplicate parsed-ID candidate for every `feImage` or marker reference. Duplicate candidates
are represented by on-demand transparent group nodes. A reference adds one edge to its group; the
group owns each candidate edge once.

The parsed/marker portion of dependency construction changes from worst-case `O(D * R)` edges and
work to `O(D + R)`. Transparent groups have zero element weight and zero depth increment, but remain
in DFS cycle detection and occurrence propagation. The returned occurrence vector is truncated to
real source nodes, preserving encounter-order export accounting. Unique IDs stay on the original
direct-edge path and allocate no group.

One combined scale-and-semantics test covers 64 duplicate candidates and 64 parsed/marker
references, exact linear edge count, multiplicity, expanded elements, depth, and raw occurrences.
The existing cycle test was extended with a duplicate parsed-ID cycle; no separate cycle test was
added.

### Duplicate expanded attributes

Commit `3f5f92058b6626d0407db3eceb52cf320a0dec92` retains the zero-allocation first
namespaced attribute and replaces subsequent linear `Vec::contains` checks with a `HashSet`.
Expanded-name duplicate detection changes from worst-case `O(A^2)` membership work to expected
`O(A)` without iteration-order semantics. Existing malformed-XML coverage proves the same duplicate
expanded-name rejection; no new test was added.

These are adversarial input-complexity and resource-boundary repairs. Their acceptance rests on the
strict asymptotic reduction plus semantic/security tests, not on ordinary-fixture microseconds.

Verification completed during the two repairs:

```text
CARGO_BUILD_JOBS=1 cargo +1.95.0 check -p merman-render
CARGO_BUILD_JOBS=1 cargo +1.95.0 nextest run -p merman-render --test-threads=1
1216 passed; 1 skipped

CARGO_BUILD_JOBS=1 cargo +1.95.0 nextest run -p merman-render \
  -E 'test(rejects_dtd_processing_instructions_and_malformed_xml)' --test-threads=1
1 passed
```

## Rejected one-shot export fusion

The temporary PNG candidate consumed the sealed SVG, moved its source/reference plan into the
exporter, and performed raster preparation plus PNG encoding inside one 8 MiB backend worker. The
baseline borrowed the artifact, cloned source/plan, and used one worker for preparation followed by
one worker for encoding. The public benchmark source was identical on both sides.

- Base commit: `bae8c36cd54c2f641bbd0881589ed8c1341fc1ea`.
- Base executable: `1563d042c35939f36e65e4a872ca36e25845c153d656c25d8c324b07c9360807`,
  28,611,664 bytes.
- Candidate executable: `a1e7119016ee1f62cb6f54f4b4dff199fd5074afc4e196f494415fb831d94929`,
  28,592,608 bytes.
- Lane: `png_end_to_end/kanban_medium`.
- Diagnostic schedule: four alternating BH/HB pairs, 20 samples, one-second warmup, two-second
  measurement, and 10,000 resamples per observation.

| Pair | Order | Base ms | Candidate ms | Delta ms |
|---:|:---:|---:|---:|---:|
| 1 | BH | 8.0742 | 8.0042 | -0.0700 |
| 2 | HB | 8.0239 | 8.1163 | +0.0924 |
| 3 | BH | 8.2936 | 7.9960 | -0.2976 |
| 4 | HB | 8.0319 | 8.0531 | +0.0212 |

The means were 8.1059 ms and 8.0424 ms: -0.0635 ms and -0.78%. This complete operation uses the
ordinary gate, whose relative threshold requires about 0.8106 ms. The observed effect is an order
of magnitude smaller and changes direction across pairs. Formal confirmation cannot rescue that
effect-size mismatch. The consuming API, worker-internal encoder, and facade switch were removed.
JPEG/PDF variants were not built because they eliminate the same fixed spawn/clone term behind
larger encoders.

## Rejected single-reader terminal validation

The temporary validator candidate drove independent general and Resvg observers from one strict
`NsReader`. General validation ran first for every event. The first Resvg error was buffered and its
state dropped while general validation continued to EOF, preserving the baseline rule that any
general XML/resource error overrides every Resvg-contract error. Attribute semantics remained in
two independent observers; downstream sanitizers and `usvg` parsing were unchanged.

- Base commit: `7be6739a0232ba936d944d7e8939dc9614f6f74a`.
- Base executable: `fd77bb2e3a795232249922c14ec8fd30f9d8458beba9e551d86f68bcc59dfbbf`,
  28,627,968 bytes.
- Candidate executable: `0ed9e8fb736979baa3121ee1e1f96324cd0f9f52dd452390bde186cd375b7bb9`,
  28,641,536 bytes.
- Lane: `resvg_end_to_end/kanban_medium`.
- Diagnostic schedule: four alternating BH/HB pairs, 20 samples, one-second warmup, one-second
  measurement, and 10,000 resamples per observation.

| Pair | Order | Base ms | Candidate ms | Delta ms |
|---:|:---:|---:|---:|---:|
| 1 | BH | 4.8544 | 4.8632 | +0.0088 |
| 2 | HB | 4.8205 | 4.8757 | +0.0552 |
| 3 | BH | 4.7811 | 4.8282 | +0.0471 |
| 4 | HB | 4.8237 | 4.8219 | -0.0018 |

The means were 4.819925 ms and 4.847250 ms: +0.027325 ms and +0.57%. Removing one tokenizer pass
does not offset observer dispatch/error buffering and is negligible beside the ResvgSafe preset's
other sanitization passes. The candidate, including its temporary three-row dual-error test, was
removed. The reviewed two-pass fail-closed validator remains authoritative.

## Decision

- Retain transparent parsed-ID groups and hash-based expanded-attribute membership because they
  remove attacker-amplifiable superlinear work while preserving bounded semantics.
- Retain the two narrow public benchmark lanes because raster export and ResvgSafe finalization had
  no direct coverage before this unit.
- Reject one-shot export fusion and single-reader validation. Do not add API aliases, JPEG/PDF
  copies, event-by-event test matrices, differential oracles, or new benchmark scripts.
- Revisit ordinary latency only when a profile identifies a larger owner in preset sanitization or
  a real workload registers a different throughput/memory contract before measurement.
