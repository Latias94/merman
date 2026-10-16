# Reserved prepared-token search candidate — Windows, 2026-10-16

## Decision

Do not retain this performance candidate. Its ordinary latency benefit is not established by the
complete diagnostic, and controls remain mixed. The patch, sources, frozen artifacts and raw
observations remain available. This is not evidence that the algorithm has no benefit, nor a
confirmed regression. No threshold is changed and no samples are appended to this experiment.

The adjacent product baseline is `e56dde946`. The candidate changes only the mandatory reserved
prepared-token prefix search and the visibility of its shared checkpoint-batch constant. A borrowed
memmem Finder searches 64-byte logical blocks with 15-byte lookbehind, preserving exact first-match,
empty/short/miss and checkpoint behavior. The original token/tag parser and general KMP finder
remain unchanged. It is still O(N); the candidate removes a fixed prefix-table allocation and the
per-byte KMP loop in this one observer. It does not bypass token checks or XML validation.

## Complete controlled diagnostic

Windows 11 / Intel Core i7-11700, Rust 1.95.0, locked optimized native pipeline, default features
plus SVG. There are 42 measurements across seven fixtures, three complete AB/BA/AB pairs, sample
size 30, warmup 2 seconds and measurement 3 seconds. All 84 native process boundary snapshots are
clear; they do not establish continuous host isolation. SVG pre/postflight identity gates pass.
These are frozen dirty-patch diagnostics, without A/A calibration or clean-ref confirmation.

| Fixture | Base mean us | Candidate mean us | Mean movement | Paired ratio median movement |
| --- | ---: | ---: | ---: | ---: |
| class_medium | 2653.967 | 2575.700 | -2.95% | -4.77% |
| journey_medium | 222.103 | 204.480 | -7.93% | -7.81% |
| class_tiny | 635.340 | 571.393 | -10.06% | -3.05% |
| xychart_medium | 401.543 | 369.873 | -7.89% | -8.80% |
| flowchart_medium | 3451.000 | 3618.967 | +4.87% | +3.84% |
| sequence_medium | 1105.267 | 1174.133 | +6.23% | -5.24% |
| kanban_medium | 277.263 | 257.943 | -6.97% | -8.54% |

The Class-tiny arithmetic mean barely clears 10% and 50 us, but its three observations are:
608.530→590.000 us, 557.480→591.340 us and 740.010→532.840 us. The last pair dominates the mean;
the descriptive paired-ratio median is only −3.045%, with a paired-delta median of −18.530 us.
The paired medians are a sensitivity check, not a substituted admission statistic. Flowchart is
slower in all three diagnostic pairs, while Sequence's positive mean is dominated by its last
pair after two improvements. Neither is a confirmed regression. Journey, XYChart and Kanban
improve in all three pairs, but their absolute mean savings are only 17.623/31.670/19.320 us.

The current target does not justify treating this edge signal as an admitted ordinary win or
spending a full confirmation budget before investigating the larger terminal owner. Keep the
unchanged canonical A/A, power-sized confirmation and joint 10%/50 us improvement contract for
future latency claims. Secondary allocation and binary-size changes do not replace that primary.

## Correctness, allocation and size

All 8 focused tests pass, including differential results and fail-at-each-checkpoint behavior at
block offsets, Unicode and partial prefixes. All 3,324 renderer tests pass with 2 existing skips;
workspace formatting and all-target renderer Clippy pass with all diagrams, ELK, Cytoscape and math.
The complete native pipeline smoke yields 459 preflight receipts per side, all 152 render/end-to-end
SVG identities equal. Unordered non-SVG State projections remain a separate recorded caveat.

The public-family render allocation probe holds parsing/preparation/session work outside counting
and output hashing/destruction after counting. It uses two warmups and three identical allocation
observations for each side and each of Class medium/tiny, Journey and Kanban. Every fixture removes
exactly one allocation and 128 requested bytes; peak growth and net retained delta are unchanged.
These are family-render allocation controls, not complete-facade memory or RSS measurements.

The frozen benchmark executable falls from 40,518,656 to 40,439,808 bytes, −78,848 bytes. This is a
secondary native artifact observation, not a preregistered size admission or a WASM result.
Independent source and complete data review pass; correctness is not the reason for stopping.

## Local evidence

`target/bench/experiments/reserved-token-search-20261016/` retains the registration, exact patch,
source/extern/probe hashes, Cargo artifact JSON, frozen executables, allocation rows, all identities,
42 timing records and 84 boundary snapshots. Root verifies the two candidate file hashes before
applying only the exact inverse owned patch, then verifies both baseline file hashes. No unrelated
work is discarded. The product keeps the original observer while further attribution moves inside
mandatory standalone finalization.
