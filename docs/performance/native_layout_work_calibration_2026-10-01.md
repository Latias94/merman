# Native layout-work calibration — 2026-10-01

## Decision

Status: **accepted-structural** for the finite work policy. This is not a latency, throughput,
portable memory, or host-SLO claim.

Set `trusted-native.max_layout_work_units` to **15,000,000** and select `trusted-native` for
rustdoc's offline build. The CLI already selects that profile. Keep `interactive` at 800,000,
`constrained` at 125,000, and the general library/Web default interactive. The other source,
model, nesting, SVG, backend, and CLI scheduling limits remain unchanged.

This applies to the source checkout after alpha.7, not the published alpha.7 macro. The macro's
resource-profile and positive finite override options were added in the preceding fix. Ordinary
nested Class documentation now renders without selecting a budget. Each diagram and theme gets
an independent budget; explicit tighter profiles and local limits still reject oversized work.

## Workload and margin

Mermaid 12's ELK default makes the former interactive documentation budget unsuitable for some
ordinary Class diagrams. The issue reproducer is synthetic: the reporter's private source was
not available. No layout algorithm, accounting coefficient, namespace topology, or fallback was
changed to make the reproducer fit.

The closed corpus contains 73 registered fixtures: the prior 71 plus two fixed nested-Class
controls. The controls have 20/160 classes, eight nested namespaces, and 20/160 relations.
Both `default` and `dark` site themes are tested, preserving source-level theme precedence.
Every successful bounded SVG and consumed-work count equals its unbounded control.

The prior alpha.7 exploratory sweep suggested the candidate. The existing absolute/relative
margin rule and a native million-unit quantum were fixed before implementation and this
confirmation. This is a closed-corpus engineering policy, not a blind holdout or a statistical
production-coverage estimate. The rule is: add at least 100,000 units or 10% of the measured
maximum, whichever is larger, then round upward to a 1,000,000-unit quantum for native work:

```text
W = 12,759,734
required = W + max(100,000, ceil(W / 10)) = 14,035,708
ceiling = round_up(required, 1,000,000) = 15,000,000
headroom = 2,240,266 units = 17.557310% of W
```

These are deterministic admission units, not milliseconds, bytes, source lines, or a node limit.
The margin covers this closed workload; it is not a promise for every graph.

| Registered fixture | Consumed work units |
| --- | ---: |
| `class_nested_namespaces` | 1,193,772 |
| `flowchart_large` | 5,254,117 |
| `flowchart_svg_label_reuse` | 938,215 |
| `class_nested_namespaces_large` | 12,759,734 |

## Exact admission and amplification boundaries

The largest Class fixture succeeds at **12,759,734** and emits the same SVG hash. At **12,759,733**
it returns a typed `ceiling` rejection in `layout_model`, with `actual=12759734`, the selected
`trusted-native` profile, and the exact explicit override.

For the registered linear Flowchart curve, every size from one through 968 nodes succeeds.
The full accepted prefix is hashed; no unproved monotonicity shortcut is used. Size 969 rejects
at `actual=15000002`, and size 970 also rejects. This boundary is specific to that topology.

The configuration control holds 32 Architecture services and 31 edges fixed and changes only
`numIter`: 59,045 succeeds, while 59,046 rejects at `actual=15000001` and 59,047 also rejects.
The accepted graph consumes only 48,589 units because the iteration admission check is a
non-consuming upper-bound preflight. Consequently, final consumed work and a rejected charge
must not be treated as interchangeable for arbitrary inputs. An error's `actual` is not a
promise of the budget required to finish rendering.

## Host and provenance

The evidence uses clean source `b22bc723a10373356f5b1051057cc9ce98388778`, Rust/Cargo 1.95.0, a release build with
`complete-svg-elk`, and Windows 11 on an Intel Core i9-13900KF (24 cores / 32 logical processors,
approximately 64 GiB physical memory).

Each theme has five fresh, byte-identical full reports plus isolated semantic, layout, SVG,
end-to-end, exact-failure, and cardinality acceptance/rejection probes: **24 processes total**,
all successful within a 300-second outer timeout. The Windows runner gates process launch until
the helper is assigned to its Job Object, terminates descendants on timeout or normal cleanup,
and reads the target's `PeakWorkingSetSize` before releasing its process handle.

Observed target-process peak working sets reach 53.87 MiB for `default` and 53.13 MiB for `dark`.
Full process times span 27.94–34.65 s and 25.52–42.64 s respectively. These include provenance
checks and launcher overhead and are supporting observations only. They exclude descendant
memory and are not interchangeable with Linux/macOS RSS or a process-tree memory ceiling.

The [evidence manifest](evidence/native_layout_work_calibration_2026-10-01.json) binds source, lockfile, executable, corpus members, reports, and timing
observations by SHA-256. Raw artifacts remain ignored under
`target/bench/experiments/issue158-native-default/{default,dark}`. Historical interactive
calibration from 2026-08-07 remains a receipt for its old backend/corpus, not proof that the
current Mermaid 12 corpus fits 800,000 units.

## CLI scheduling

Keep the 2 GiB native scheduler pool and the 64 scheduling-byte coefficient per allowed layout
work unit. They are conservative admission weights, not measured RSS bounds. For encoded Mermaid
output, reserve the shared prefix plus the larger transient phase:

```text
S = 2 * source_limit + 256 * model_items_limit + 2 * model_text_limit
G = 2 * svg_limit
L = 64 * layout_work_limit + 1 MiB
weight = S + G + max(L, encoding_weight)
```

The semantic/layout artifact is consumed before PNG/JPEG/PDF encoding begins. Actual encoding
checks still include the fixed prefix; exclusive modes retain their finite floor and pool checks.
Raw-SVG imports and SVG-only accounting are unchanged. This removes the false addition of two
non-overlapping workspaces; it does not remove the layout weight or change the pool.

At 15m, default Mermaid SVG/PNG/JPEG/PDF reserve **1,347,792,896 scheduling bytes**, admitting one
backend at a time. At the former 1m native limit, SVG could admit four. `--jobs` is an upper bound.
At an explicit 20m, PDF now fits at 1,667,792,896 bytes instead of failing the pool during
preparation. Tests retain exact/plus-one, overflow, cancellation, actual-weight, and permit-release
checks. Decoupling CPU allowance from a calibrated layout working-set estimate remains separate
work; the current corpus does not justify deleting or truncating the coefficient.

## Reproduce

```text
cargo build --locked --release -p merman --example layout_work_calibration --features complete-svg-elk --jobs 2
python tools/bench/run_layout_work_calibration.py --authoritative-date 2026-10-01 --resource-profile trusted-native --theme default --full-repeats 5 --out-dir target/bench/native-calibration-default
python tools/bench/run_layout_work_calibration.py --authoritative-date 2026-10-01 --resource-profile trusted-native --theme dark --full-repeats 5 --out-dir target/bench/native-calibration-dark
python -m unittest tools.bench.test_layout_work_calibration
```

Output directories must be empty. Native pipeline layout/render/end-to-end benchmarks use the
same finite offline policy. Formal latency comparisons with older revisions must align both
harness and corpus; the existing same-harness confirmation gate is intentionally unchanged.

## Batch controls

A 69-diagram Markdown control selects the existing corpus members that fit the original 800k
exploratory ceiling, so both a 1m and a 15m layout policy can complete the same input. It preserves
frontmatter indentation when wrapping sources into fences. Both use the same release build of the
current-source CLI, `trusted-native`, eight requested jobs, the default theme, a 30-second cooperative deadline, and
an outer 60-second managed-process timeout. Each writes into a fresh directory.

All 34 runs succeeded: two warmups, eight A/A pairs, and eight alternating A/B or B/A pairs.
Every run wrote 69 SVGs with the same complete output-set digest. The A/B medians were 20.639 s
at 1m and 19.163 s at 15m. These are descriptive observations, **not an admitted speedup or proof
that pure rendering throughput is unchanged**. The whole operation includes synchronized
transaction publication; host/I/O timing also shifted between the A/A and A/B portions.

Source inspection confirms that the operation deadline has no sleeping watchdog. Publication
crosses a final cooperative cancellation boundary before committing the transaction, so successful
completion may occur after the requested deadline. The outer Job timeout remains the actual
process-tree cleanup boundary for this experiment.

The release build of the current-source CLI also rendered the 20-class reproducer in SVG, PNG,
JPEG, and PDF with its default resource policy and default output settings. An explicit 20m PDF run succeeded as well.
The corresponding raw output hashes, encoded sizes, and peak-working-set observations are bound
by the evidence manifest; they are correctness/output controls, not latency benchmarks.

## Verification

The complete SVG structure gate passed with the repository's existing, exact browser-text residual
allowlist; neither the comparator nor upstream baselines changed. The first attempt lacked the
Node KaTeX provider for six math fixtures. Installing the pinned `tools/mermaid-cli` dependencies
with `PUPPETEER_SKIP_DOWNLOAD=true npm ci` resolved that local prerequisite before the passing run.

Focused checks passed: rustdoc (41 Class/Flowchart/ELK and 22 SVG-only tests, plus eight doctests
per closure), CLI (300 related tests and two final native-default smoke checks), renderer resource
contracts (22), binding catalog (one), Rust calibration contracts (10), and Python calibration,
corpus, and recipe contracts (14/10/22). These are selected test invocations, not disjoint totals.
The generated resource contract, formatting, and Clippy with warnings denied also passed. Windows
is the local validation host; other platform matrices remain owned by CI.

Both added Class fixtures also pass the native `layout`, `render`, and `end_to_end` benchmark
smoke paths (six selections), including preflight/postflight SVG identity where applicable.
These `--test` runs establish harness execution, not measured Criterion latency.
