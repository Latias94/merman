# CLI Resource Policy

Merman CLI resolves one resource policy before it acquires input, opens a network
connection, creates an output directory, or starts rendering. The selected
profile covers the complete workflow:

- Mermaid source, semantic-model, layout, and SVG budgets come from the
  canonical `merman` resource contract.
- CLI-owned budgets cover Markdown documents, auxiliary files, icon packs,
  staged output, scheduler admission, concurrency, redirects, and timeouts.
- A named `--resource-limit ID=VALUE` override changes one budget. It does not
  disable the other budgets selected by the profile.

These values are operational policy, not Mermaid syntax, compatibility, or ABI
limits. They may change between releases when measured evidence supports the
change.

## Profiles

| Profile | Intended use |
| --- | --- |
| `constrained` | Untrusted submissions and tightly budgeted services |
| `interactive` | Editor previews and other latency-sensitive local work |
| `trusted-native` | Default CLI use on trusted local files and batch jobs |
| `unbounded-for-trusted-input` | Explicitly trusted exceptional workloads |

The unbounded profile removes policy ceilings, but it does not remove integer
overflow checks, backend hard capabilities, job and redirect guards, or finite
network timeouts.

## CLI-Owned Defaults

Byte values use binary units. The job default also depends on available CPU
parallelism.

| Limit ID | Constrained | Interactive | Trusted native | Trusted unbounded |
| --- | ---: | ---: | ---: | ---: |
| `max_markdown_document_bytes` | 4 MiB | 8 MiB | 64 MiB | Unlimited |
| `max_config_bytes` | 256 KiB | 512 KiB | 4 MiB | Unlimited |
| `max_css_bytes` | 512 KiB | 1 MiB | 8 MiB | Unlimited |
| `max_puppeteer_config_bytes` | 256 KiB | 512 KiB | 2 MiB | Unlimited |
| `max_local_icon_body_bytes` | 8 MiB | 16 MiB | 64 MiB | Unlimited |
| `max_remote_icon_body_bytes` | 8 MiB | 16 MiB | 64 MiB | Unlimited |
| `max_aggregate_icon_bytes` | 16 MiB | 32 MiB | 256 MiB | Unlimited |
| `max_icon_packs` | 8 | 16 | 64 | 256 hard guard |
| `max_markdown_charts` | 256 | 1,024 | 8,192 | Unlimited |
| `max_staged_bytes` | 512 MiB | 1 GiB | 8 GiB | Unlimited |
| `max_scheduling_weight_bytes` | 576 MiB | 640 MiB | 2 GiB | Unlimited |
| `max_jobs` | 2 | 4 | 32 | 64 hard guard |
| default jobs | 1 | `min(CPU, 2)` | `min(max(CPU / 2, 1), 8)` | `min(max(CPU / 2, 1), 32)` |
| `max_redirects` | 3 | 5 | 10 | 20 hard guard |
| `connect_timeout_seconds` | 5 | 5 | 10 | 30 hard guard |
| `per_hop_timeout_seconds` | 15 | 30 | 60 | 300 hard guard |
| `workflow_timeout_seconds` | 30 | 60 | 300 | 900 hard guard |

`max_scheduling_weight_bytes` is a conservative scheduler admission weight. It
is not measured RSS and does not promise an operating-system memory ceiling.
Deployments that need a hard memory boundary must also use process or container
isolation.

## Evidence And Margins

The initial policy was reviewed on 2026-07-27 against the checked-in corpus:

| Workload | Recorded high-water mark | Smallest relevant ceiling | Margin |
| --- | ---: | ---: | ---: |
| Mermaid fixture source | 9,970 bytes | 1 MiB constrained source | 105x |
| Upstream SVG fixture | 370,289 bytes | 12 MiB constrained SVG | 34x |
| Repository Markdown document | 552,874 bytes | 4 MiB constrained Markdown | 7.5x |
| Mermaid fences in one checked-in Markdown document | 4 | 256 constrained charts | 64x |

The measurements are reproducible with:

```text
rg --files fixtures -g '*.mmd' -g '*.mermaid' -g '*.txt' -g '*.md' -g '*.html'
rg --files fixtures -g '*.svg'
rg --files -g '*.md'
```

For each set, measure file bytes and select the maximum without splitting paths
on whitespace. Fence counts must use the CLI's Markdown scanner rather than a
regular-expression approximation.

The CSS, Puppeteer compatibility, icon-body, staged-output, and scheduling
ceilings are initial engineering ceilings. The repository does not yet contain
a representative production corpus for those workload classes, so they must
not be presented as statistically calibrated limits. A future reduction
requires a checked-in measurement receipt, a representative near-high-water
test, and an explicit review of the remaining margin.

## Network Boundary

Network icon acquisition remains disabled unless explicitly authorized.
Public authorization is separate from private-network authorization. Every URL
and redirect hop must:

1. reject credentials and non-HTTP(S) schemes;
2. resolve and classify every returned address;
3. reject the entire hop when any address is outside the authorized classes;
4. pin the approved addresses while preserving the original host for HTTP and
   TLS verification;
5. enforce connect, per-hop, workflow, body-size, and redirect limits; and
6. omit credentials, paths, queries, and fragments from diagnostics.

The production DNS implementation must have a real cancellable deadline. A
post-return elapsed-time check around a blocking system resolver is not a
timeout and is not sufficient for the constrained profiles.

## Changing A Limit

A policy change is complete only when it includes:

1. the workload and threat model affected by the change;
2. a reproducible high-water measurement and its provenance;
3. the selected margin and why it is appropriate;
4. exact-limit and limit-plus-one tests;
5. profile monotonicity and hard-guard tests; and
6. dependency-closure and artifact-size evidence when the implementation adds
   a runtime, resolver, codec, or other non-trivial dependency.
