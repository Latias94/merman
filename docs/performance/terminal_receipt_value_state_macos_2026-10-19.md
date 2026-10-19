# Terminal receipt value state — macOS, October 19, 2026

Info and Error terminal receipts replace three copied observed CSS strings with an emission bit
and the comparison state they already accumulate. Treemap's title receipt borrows its expected
fill and keeps an emission bit instead of copying observed class/fill values. Each independently
required primary saves three allocations: Info and Error save 165 bytes each, and the active
Treemap title saves 48 bytes. This is allocation/work evidence, not a latency or release
convergence claim. All registered checks pass.

The clean execution baseline is `be2b6a1447f2a2668fc17dbdeedd0a691bc38c05`, on Apple M4 Pro,
48 GiB, macOS aarch64, Rust 1.95.0. It adds only the completed release-observation documentation
to the registered `0ce2aa0bdafd4988b22cf51f9650d1c653aa9d03`; all crates, Cargo manifests and
lockfile are byte-identical. The registration, execution preflight, frozen probes, patch and raw
results are in `target/bench/experiments/terminal-receipt-value-state-20261019/`. The preceding
[full release comparison](theme_v090_joint_confirmation_macos_2026-10-19.md) is complete and
archived before this build starts. Both sides use optimized release opt3 libraries with all
public features and the same existing public-operation probes, one warmup and five samples.
All Cargo, probes and checks run serially.

## Remove stored values after their last consumer

The old Info/Error receipt checks the emitted root, inherited and variable font CSS against its
expected value when the writer records them. It then allocates three copies and repeats the same
comparisons at completion. No subsequent consumer needs those observed strings. `css_emitted`
preserves the missing-event check; the existing uniqueness bit and sticky `terminal_matches`
preserve the accumulated comparisons. Expected font/geometry ownership is unchanged in this
candidate. It does not make no-theme receipt preparation lazy.

Info has an independent fill proof. With no CSS event, valid text can still prove fill while font
proof fails. A duplicate CSS event rejects font uniqueness without poisoning a valid fill proof;
a wrong first CSS event remains sticky even after a correct duplicate. Error retains exactly-one
message/version/detail counts and all text geometry checks. Focused negative contracts exercise
these state-machine boundaries.

Treemap's immutable title plan already owns the expected fill. The writer receipt can borrow it
for its local lifetime; it never enters the plan. The emitted stylesheet class/fill comparisons
become sticky facts recorded at the original stage. Missing CSS, duplicate CSS, missing or
duplicate title text, wrong classes/fill and a mismatched completion argument still reject. The
private empty expected fill remains accepted under its original matching observations. Only a
valid completed proof reaches the unchanged `OnceLock<()>` completion seal. No writer calls,
checkpoint, work charge, cancellation, unsupported-theme settlement or error order changes.

## Complete public-operation results

Each measured public render retains the same rich result at the memory checkpoint. Input loading,
theme compilation and result serialization stay outside the operation. Counts include allocation
and reallocation observations.

| Independent primary | Allocations before → after | Allocated bytes before → after | Peak growth before → after | Retained growth |
| --- | ---: | ---: | ---: | ---: |
| Info medium | 127 → 124 | 19,504 → 19,339 | 9,959 → 9,794 | 4,111 |
| Error basic | 135 → 132 | 27,197 → 27,032 | 13,844 → 13,679 | 7,855 |
| Treemap default with title | 2,589 → 2,586 | 501,686 → 501,638 | 213,024 → 213,024 | 19,008 |

All 632 successful controls preserve exact SVG/report identities and ordered custom-measurer
traces. The 26 public errors match exactly. Among 492 jointly stable controls, 64 improve,
428 stay unchanged and none increase any of the four memory metrics. Improvements comprise 27
Info, 25 Error and 12 active Treemap-title controls. Every improvement removes three allocations
and 26–177 bytes, according to the actual font/fill string lengths. Peak growth never increases;
retained growth is unchanged. The 140 variable controls remain inconclusive without retries.
Both sides have the same variable list; the only change from the preceding 141-row list is that
`treeview-fixture-02` is stable on both sides in this experiment.

All 418 theme-catalog cells are stable and retain exact warmup result metadata and all five
saved sample SVG/report identities, together with configuration, diagnostics, theme status and
family identity.
Twenty-two cells improve: eleven Info and eleven
Error, covering no theme and all ten presets. The two no-theme cells save three allocations and
165 bytes each and reduce peak by 165 bytes; twenty preset cells save three allocations and
177 bytes each, with peak unchanged. The other 396 cells retain all four metrics. Retained growth
is unchanged everywhere. The catalog Treemap scene has no active title receipt; its unchanged
result is expected, while the separate title controls establish that branch's saving. The catalog
continues to cover all 35 logical families and 38 fixture aliases.

## Validation and continuation

Independent final source and raw-evidence reviews find no blocker. The applied three-file patch
and each source hash match the measured candidate; the four baseline probes are frozen and
verified. Candidate renderer and catalog executables differ from the baseline and link the
recorded optimized libraries. Raw five-sample data reproduce the primary strict count/byte
reductions, all stable control metrics and every exact output/trace comparison.

Focused all-feature release nextest passes seven tests. The complete public/renderer release
nextest run passes 5,612 tests with seven skipped; strict matching release Clippy passes.
All 37 entries pass structure, parity and parity-root checks with the existing browser-text-layout
and parser-diagnostic residuals retained; parser-diagnostic residuals are not DOM equivalence.
The optimized full WASM package builds, including wasm-opt. WASM runtime smoke is not repeated.
Formatting, diff and changed-path checks pass. The terminal driver exits zero with all 13 steps
successful; its logs, source freeze, independent reviews and 37 final DOM reports remain in the
ignored experiment directory. The final source patch SHA-256 is
`acadd4faf3063935ce52333056bf2672d5fee72f2e5605e221aafdd9eba575c3`.

The all-family convergence goal stays active. The completed release comparison still reports
Zenuml, Packet and Kanban beyond budget and Class tiny, Sankey and Wardley inconclusive. This
local allocation result does not reclassify those measurements. Next independently evaluate the
reviewed Quadrant point-binding owner and remaining default/typed ownership candidates, preserving
the full success/error/catalog controls and resource policy.
