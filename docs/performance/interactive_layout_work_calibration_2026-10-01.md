# Interactive layout-work policy calibration — 2026-10-01

## Decision

Status: **accepted-structural** for the general library and Web default layout-work ceiling.
Set `interactive.max_layout_work_units` to **14,100,000**. Keep `trusted-native` at
15,000,000 and `constrained` at 125,000. The general input/model/SVG values remain profile-owned;
this change only raises the interactive layout-work dimension. Work units are deterministic
admission work, not milliseconds, bytes, RSS, or a universal node limit.

The change is for the current source after alpha.7. Published alpha.7 still uses the former
800,000-unit contract. A native or WebAssembly host must still select its own deadline,
cancellation, concurrency and memory policy. The profile name does not infer trust from the
platform.

## Calibration result

The closed corpus has 73 fixtures and was run under both `default` and `dark` themes. Mermaid 12
ELK nested Class layouts remain the maximum at **12,759,734** units. The pre-registered rule is
`W + max(100,000, ceil(W / 10))`, rounded upward to the profile's 100,000-unit quantum:

```text
W = 12,759,734
required = 14,035,708
ceiling = round_up(required, 100,000) = 14,100,000
headroom = 1,340,266 units = 10.503871% of W
```

Each theme had five byte-identical full reports plus 7 isolated probes, all within the 300-second
outer timeout. At the exact `W` limit the maximum fixture succeeds with the same SVG; at `W - 1`
it returns the typed `layout_model` ceiling with `actual=12,759,734`. The full accepted linear
Flowchart prefix reaches 938 nodes; 939 rejects at the exact profile boundary. The Architecture
configuration control accepts 55,502 iterations and rejects at 55,503; these boundaries are
specific to the registered topologies and do not define a general node or iteration limit.

The two themes have the same work maximum. Within each theme, bounded SVG output matches its
unbounded control. Observed full process times and
Windows target peak working sets are supporting observations only; they are not latency or memory
SLOs and exclude other host processes.

## Why the default is use-case based

Zed's local alpha.5 snapshot and several public alpha.6 consumers render synchronously on a
background worker while inheriting library defaults. Offline document adapters also use the library
facade directly, so a CLI-only default would not fix them. The host integration guide records these
pinned source observations and shows one-policy SVG configuration plus cooperative cancellation.

The larger interactive work envelope prevents ordinary current-backend diagrams from being
rejected solely because the old alpha.7 work ceiling was inherited. It does not make a preview
responsive by itself. Hosts should cancel superseded controls, start deadlines when workers begin,
limit concurrent jobs, and preserve typed resource errors instead of silently retrying unbounded.

## Scheduling and memory boundary

A 112-process allocator probe covered the registered corpus plus targeted nested, cross-scope,
long-label, Architecture, and long-rank Dagre controls. A simple `items/text_bytes` layout-memory
estimate was rejected: a 1,024-node top-level long-rank Dagre input had only 2,388 model items and
108,138 model-text bytes but reached **898,171,041 bytes** of allocator live growth before the
15-million work rejection. This is allocator live bytes, not RSS; it demonstrates derived graph
amplification, including failed operations.

The CLI therefore retains the existing 64-byte-per-work-unit conservative scheduling allowance
and phase-peak accounting. The interactive scheduler pool is raised from 640 MiB to **1 GiB**, so
its default graphical estimate of 970,360,832 bytes fits. Trusted native remains 2 GiB with
1,347,792,896 bytes per default estimate; constrained remains 576 MiB. These pools belong only to
CLI admission and do not constrain direct library, Zed, mobile, or browser hosts. No throughput,
portable memory-bound, or operating-system safety claim is made.

## Verification and provenance

Source revision: `740755137eff8c7db54550151663072bed7a0ffa`.

The complete SVG structure gate passed without comparator or baseline changes. Focused operation,
Zed fixture, CLI, resource/rustdoc, catalog, Clippy, formatting, minimal feature, and preview
example checks passed. The broad `verify --strict` command was intentionally not reported: it
expands into unrelated workspace, release/legal, Web/browser, and VS Code gates owned by CI.

The compact [evidence manifest](evidence/interactive_layout_work_calibration_2026-10-01.json)
binds raw summaries, reports, source corpus and exploratory controls by SHA-256. Raw files remain
ignored under `target/bench/experiments/integration-resource-policy/`.
