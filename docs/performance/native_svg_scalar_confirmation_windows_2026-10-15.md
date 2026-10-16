# Native SVG scalar confirmation on Windows, 2026-10-15

The committed UTF-8 character candidate remains **inconclusive for public SVG latency**. The full registered comparison confirms no material regression for Kanban, but does not admit an improvement. Six other fixtures fail A/A calibration and have no confirmation A/B samples. This does not close the v0.9.0 SVG regression.

## Revisions and evidence contract

The authorized commits are `21671959c` (`refactor(svg): validate XML characters in bounded UTF-8 blocks`) and `ad3d5f07b` (`docs(perf): record SVG scalar and terminal experiments`). The clean comparison uses base `a56aa0792f6d97e976517887cc86cdfe58e46d74` and head `ad3d5f07bcb5dd6ee6d1cf244727835341193c68`; the head product change is `21671959c`, with only documentation added afterward. No additional production change or push is made during this checkpoint.

The canonical `tools/bench/compare_self.py` independently builds both locked recipes with one Cargo job and separate targets. Features, benchmark source, corpus, seven fixture bytes, lifecycle and output identities match. The host is Windows 11, Intel Core i7-11700, Rust 1.95.0, x86_64-pc-windows-msvc, optimized bench profile, default features plus SVG, deterministic measurement, trusted-native resources and reused renderer. Pre/post-sampling Git, executable and fixture checks pass.

| Executable | Bytes | SHA-256 |
| --- | ---: | --- |
| Clean base | 40,279,040 | `6288b6ba3d3635bb85e57cae0d5ef1e866057d6c55a9c541b2ee9a96e4a0f8d2` |
| Clean head | 40,271,872 | `59153d05d2ae9a59e610580361681fb215f59e0f68efd9ef12647a33fdfe0a1c` |

The new base executable is independently built and has a different digest from the earlier frozen diagnostic base. Earlier source/binary receipts retain their original precommit evidence class. They are not rewritten into clean confirmation receipts. The preceding [scalar experiments](native_svg_scalar_experiments_windows_2026-10-15.md) contain the 3,317 passing renderer tests, two existing ignored tests, 152 exact SVG identities and full renderer Clippy/format evidence; these are not repeated or counted as new tests here.

## Registered measurements and result

Three alternating diagnostic rounds first show lower mean estimates for all seven fixtures. They are advisory and are not included in confirmation. The fixed confirmation contract retains all seven fixtures, 30 Criterion samples, two-second warmup, three-second measurement, eight balanced A/A pairs per executable, a power-derived even A/B count capped at 32, and the joint 10% and 50 us improvement requirement. Suite confidence remains 95% simultaneous over 14 components, with Bonferroni adjustment and 10,000 bootstrap resamples.

The first confirmation attempt is rejected before the first measurement because a separate Cargo process is observed; it has zero complete A/A pairs. A single preregistered fresh retry discards all of that attempt and completes both eight-pair A/A schedules: 224 Criterion calibration measurements. No rows are removed and no samples are spliced.

| Fixture | Calibration result | Confirmation pairs |
| --- | --- | ---: |
| Class medium | Unstable; derived requirement also exceeds cap | 0 |
| Journey medium | Base unstable | 0 |
| Class tiny | Unstable | 0 |
| XY Chart medium | Unstable | 0 |
| Flowchart medium | Unstable; base derived requirement exceeds cap | 0 |
| Sequence medium | Unstable; base derived requirement exceeds cap | 0 |
| Kanban medium | Stable on both executables | 32 |

Kanban itself requires eight pairs by its dispersion estimate. The helper schedules 32 using its suite-wide capped plan, not because Kanban independently requires 32. Its completed comparison is:

| Metric | Result |
| --- | --- |
| Base / head arithmetic mean | 333.402 / 293.590 us |
| Mean absolute delta, head minus base | -39.812 us |
| Paired log-ratio transformed to relative movement | -11.7002% |
| Simultaneous relative bounds | -15.3774% to -8.1993% |
| Simultaneous absolute bounds | -55.457 to -26.852 us |
| Non-regression outcome | `confirmed_non_regression` |
| Improvement outcome | `inconclusive` |

The bounds do not establish both registered improvement thresholds. The transformed paired log ratio differs from the ratio of arithmetic means; the reciprocal improvement statistic must not be reported as the candidate's percentage latency reduction. There are zero confirmed improvements, zero confirmed regressions and zero contract errors in the retry. The overall result is `inconclusive`, exit 3. Non-regression is established only for Kanban; it cannot be generalized to the six unconfirmed controls. No further sampling is appended to this frozen experiment.

## Host observation limits

A read-only `Win32_ProcessStartTrace` subscription is denied by Windows. No permission, system policy or user process is changed. The retained comparison wrapper observes compiler/Cargo process snapshots only before and after each measurement, makes observed contamination sticky and delegates all measurement, scheduling, statistics and result construction to the canonical owner. The first failure is preserved separately. The retry records no boundary hits.

Boundary snapshots are not continuous process observation or complete host isolation. They cannot exclude a process that starts and exits inside one measurement, CPU/thermal changes or unrelated Unity/editor load. Calibration instability is observed; this experiment does not attribute it to a particular background process. A review also identifies that unexpected snapshot-command or log-write exceptions would not latch the wrapper's guard state; canonical contract errors would still prevent admission. No such exception occurs in this retry. The frozen launcher is retained unchanged with its source hash rather than silently corrected after measurement.

## Next source-backed candidate

Read-only analysis excludes unconditional success-path cloning of large error values. The actual checkpoint callback inlines the execution/session/meter wrappers; payload moves and terminal/provenance conversion are on the error branch. Core cancellation code is unchanged from v0.9.0. This does not justify changing public error representation or repeating the rejected generic-dispatch specialization.

A separate fresh-baseline code-generation inspection confirms a concrete ownership cost: the dynamic raw-structure callback copies 488 bytes of immutable `RenderResourcePolicy` before checking the element ceiling. The callback symbol ends in `c8775c2644646882E.llvm.14399439668349826554`: offset +0x24 selects length 0x1e8, the call at +0x2d has a `memcpy` relocation at +0x2e, and the first limit check follows at +0x32. Both complete XML entry points reference this callback. The walker calls it once for each Start/Empty element. Both limit checker methods already borrow `&self`.

The next independent experiment should obtain the immutable policy once per validation and let the callback borrow it, or expose a private borrowed policy accessor while preserving the public getter. Its code-generation exit condition is no whole-policy copy in the element callback; entry hoisting may copy at most once. Preserve all element checks, first-error prefix ordering, exact limit actual, Postprocess phase, cancellation and sticky terminal. This checkpoint does not implement or admit that candidate.

The inspected rlib and extracted object have SHA-256 `29660b2105f11d54dca53843b5a023d68b7d35176a76ca36eb9311e65341c8cc` and `b1d60327bc5a757032293edd1f74bc2900eae6eb3531b9e1ff1f9a1d56702d32`. They come from the isolated clean-base target. The benchmark receipt does not retain an rlib Cargo-artifact/link-map association, so this is local code-generation evidence, not an exclusive CPU percentage or a measured speedup.

## Local evidence

All raw artifacts remain under `target/bench/experiments/svg-scalar-confirmation-20261015/`: experiment.yaml; commits.txt; diagnostic-receipt/summary and end_to_end-3pairs.json; canonical-common-args.json; clean-discovery.json; compiler-rejected-attempt/; confirmation-retry.json/.md and command/progress; boundary snapshots and launcher hash; guard/smoke-001/; decision-summary.json; policy-copy/base-static-receipt.json and local assembly. No full shared DOM/strict release qualification is claimed for this pending candidate.
