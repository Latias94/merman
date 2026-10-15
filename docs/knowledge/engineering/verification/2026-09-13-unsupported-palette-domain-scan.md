---
type: Verification Evidence
title: Bound unsupported-terminal palette discovery by prepared domain membership
timestamp: 2026-09-13
related_plan: docs/plans/2026-08-09-001-portable-theme-architecture-convergence-addendum.md
git_branch: refactor/presentation-theme-model
tags: theme,performance,resource-accounting,verification
---

# Finding and bound

The C5 closure audit at `f577318f866a41245760905812b4201007ee9af7` found a repeated
mechanism-map scan in `family/evidence_support.rs::reconcile_unsupported_terminal_domains`.
For each terminal with unspecified, non-source-owned fill, palette discovery filtered all
mechanism keys, including static rules. A domain containing only static stroke rules therefore
rescanned those rules for every terminal even though no palette existed. The work meter charged
routes once and terminals once; this extra scan was not charged.

Let N be terminal occurrences, M domain mechanism keys, and P domain palette keys. Palette
candidate discovery previously took worst-case O(N*M) key visits. The domain now prepares all
palette-key references once, then each eligible terminal checks only those references:
O(M+N*P) discovery work and O(P) additional borrowed entries. P is bounded by the admitted
semantic targets. This bound concerns palette discovery, not the entire reconciliation function;
style resolution, winner lookups, evidence writes, and their existing accounting remain.
A domain whose fills never need fallback now pays one additional O(M) pass, of the same order as
its existing observation/outcome preparation. Empty owned outcomes still return before preparing
palette references.

The source ownership and Unspecified-fill gate, Text/NodeLabel dual-palette semantics,
per-target `series_color` query, output order, and work-meter charges are unchanged. No public
support claim, resource limit, dependency, or artifact profile changes.

# Validation

The private Release owner suite passed **895/895**: the prior 882-test core/binding catalog,
family-program, primary-family SVG, C6 runtime, route-cutover and retirement scope plus all 13
shared unsupported-terminal tests. The ignored benchmark is excluded from that count.
Independent correctness review found no remaining findings in the final production diff.
`cargo fmt --all --check`, scoped production-library Clippy, `git diff --check`, and the full
SVG structure comparison passed. Clippy emitted existing repository warnings; it was not a
warning-free `-D warnings` gate. The structure gate used the documented exact browser-text-layout
residual policy, with no comparator changes.

The private regression uses 511 static NodeLabel stroke rules and 1,000 terminals. It exercises
no palette, both Text and NodeLabel palettes, source-owned fill, and an explicit Clear rule.
It checks residual/NotApplicable membership, exact work use, success at the exact work ceiling,
and ResourceLimitExceeded one unit below it. The first baseline run passed the 12 existing tests
but the new Clear fixture attempted 513 rules, exceeding the default 512-rule limit; the final
fixture reserves one slot for Clear. This was a fixture error, not a renderer failure.

The ignored private diagnostic runs 1, 128, and 512 static rules with 1,000 terminals, two warmups
and seven samples per scale. Compilation and fixture preparation are outside the timed region;
each sample checks evidence counts and work use. Results are local stage diagnostics, not an
end-to-end latency claim or a Performance CI gate. The acceptance basis is the reviewed bound
and unchanged semantics/resource thresholds.

| Static rules | Baseline stage median | Candidate stage median |
| --- | --- | --- |
| 1 | 403.708 us | 411.708 us |
| 128 | 736.833 us | 454.750 us |
| 512 | 1,605.833 us | 516.625 us |

These were sequential baseline/candidate diagnostics, without balanced A/A or AB/BA latency
confirmation. Do not infer an ordinary-render speedup or significance at the one-rule scale.
The baseline used the old production loop and the final benchmark workload; correcting the
separate Clear regression fixture did not alter this workload.

The experiment ledger, exact owner command, and raw logs are under
`target/bench/experiments/unsupported-palette-domain-scan/`. These ignored local artifacts are
not portable release evidence. The host is macOS ARM64, Rust 1.95.0, Release, two Cargo jobs,
with the private acceptance cfg and `merman-render/layout-elk` enabled.

# Current audit disposition and remaining gates

The earlier `1c2067f5` audit's Block CI gap was fixed by `2fa9063d5`: both
`.github/workflows/ci.yml` and `release-preflight.yml` explicitly select
`block_title_legacy_projection` with PNG and the private acceptance wrapper. All 33 release
workflow security tests passed in this audit, including the guard for both workflows.
XY Chart changed in `477c09505` to call `capture_terminals` only after accounting yields nonempty pending receipts;
that earlier unconditional-cache finding no longer describes the current implementation.
This slice does not claim a new XY Chart measurement.

The parallel read-only C5 catalog audit found a single core ID/alias descriptor, core-derived
registry/discovery projections, core-backed theme scope parsing, exhaustive renderer dispatch,
and a runtime family identity check. Existing tests provide catalog and detection coverage;
this source review alone does not close every cross-crate executed-witness requirement.

C5 still needs its complete current-source closure audit. C7a artifact-bound catalog promotion,
public rollout and contract freeze, and C7b bridge retirement remain open. The full workspace,
all-platform builds, installed consumers, and complete browser suite are outside this slice.
Accordingly, the aggregate `xtask verify --strict` gate was not run: it includes those materialized
release and browser/platform checks, and this host had approximately 5.5 GiB free disk. Scoped
success here does not certify that aggregate gate.

The owner command is the State classification record's 882-test command with
`test(family::evidence_support::tests)` added to its renderer filter. The diagnostic uses that
same package/feature recipe and selects `test(unsupported_palette_discovery_workload_benchmark)`
with `--run-ignored only --success-output immediate`. Additional commands were:

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py clippy --release --locked -p merman-core -p merman-render -p merman-bindings-core -p merman-theme-acceptance --features merman-render/layout-elk --lib
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
cargo fmt --all --check
git diff --check
python3 -m unittest discover -s scripts -p test_release_workflow_security.py
```

# Clean-checkout confirmation

The committed implementation `05ef33898b7bf77f8d7f39a66aa09d85ea086c44` passed the same
**895/895** owner tests from `/tmp/merman-palette-domain-05ef33898`. Git status was empty
before and after. The checkout reused the active worktree's target directory through
`CARGO_TARGET_DIR`, with two Cargo jobs. Fresh checkout timestamps triggered recompilation;
the log identifies core, renderer, and the other workspace dependencies under this checkout.
This was clean-source verification with a shared dependency cache, not an empty-target build.

The resulting `state_svg_test-f66e607ba44a9308` binary has SHA-256
`4f84260007a10be32fa3780599f8b2ff109c361659ade597378589d291371044`.
It contains eight core source-path references to the clean checkout and zero references to the
stale pre-Class baseline used in the earlier cache investigation. The experiment directory
contains `clean-owner.log`, `clean-state.json`, and `clean-linkage.json`. The SVG structure,
Clippy, and diagnostic curve checks were run on this same implementation before commit in the
active worktree; they were not repeated in the clean checkout.

A subsequent read-only primary-family audit also found that Flowchart/Swimlane candidate and
residual helpers query `rule_facet_disposition`, while Sequence typography and evidence consume
the same route disposition. No duplicate narrow support table or blanket Typed classification
was found in that scope. This is source-review evidence, not an additional executed test count.
