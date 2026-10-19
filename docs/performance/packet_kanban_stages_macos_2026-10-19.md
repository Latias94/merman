# Packet and Kanban stage attribution (macOS, 2026-10-19)

The calibrated release comparison still confirms Packet and Kanban regressions; see
[the public E2E confirmation](theme_v090_confirmation_macos_2026-10-19.md). This follow-up narrows
the next experiments. It does not admit an optimization or identify a theme-only cause.

The diagnostic reuses those frozen v0.9.0 and `cd7b23674` executables, identical benchmark inputs,
features and source. Each of six rows receives two balanced AB/BA pairs, with 30 Criterion samples,
2 s warm-up and 3 s measurement per invocation: 24 calls total. The table gives the median of the
two independent estimates on each side. These are uncalibrated stage diagnostics.

| Fixture / stage | v0.9.0 (us) | Current (us) | Cross-version stage identity |
| --- | ---: | ---: | --- |
| Packet parse | 2.20105 | 2.62630 | Different projection |
| Packet preparation/layout | 0.65471 | 0.99789 | Different projection |
| Packet SVG emission | 3.27455 | 4.17805 | Exact SVG |
| Kanban parse | 7.00935 | 7.31505 | Different projection |
| Kanban preparation/layout | 8.33975 | 16.56400 | Different projection |
| Kanban SVG emission | 26.05100 | 29.01750 | Exact SVG |

Every invocation retains its same-side pre/postflight identity check. Changed parse/layout
projections prevent equivalent-stage latency claims. Kanban also moved twelve-slot stylesheet
color computation from emission into preparation; adding stage deltas or assigning the preparation
gap to newly added work would be incorrect. None of these observations replaces the public E2E
confirmation.

After timing completed, each side/fixture received one serial twelve-second Criterion render
profile, sampled by macOS `sample` for six seconds at one-millisecond intervals. Criterion's
`iter_batched` profile includes setup and destruction. Analysis selects only outermost
`FamilyRenderArtifact::render_svg[_with_edge_geometry]` subtrees below `Routine::profile`,
excluding preparation and batch destruction outside that frame. Raw samples, executable hashes,
commands and the selected-subtree diagnostic remain under
`target/bench/experiments/packet-kanban-stages-20261019/`.

Current emission samples repeatedly reach `XmlTextOutput::write_str`,
`BoundedSvgOutput::record_write` and `BoundedSvgOutputState::error`. This supports examining
fragment classification, scanned-work accounting and the active-state/error conversion boundary.
Samples are not call counts or a causal estimate of their contribution to the release gap. The
process footprint includes Criterion batching and is not production RSS evidence.

A separate follow-up profiles the complete public `end_to_end` loop with the same four frozen
runner/fixture combinations and twelve-second/six-second recipe. Unlike the render group's
`iter_batched` setup, this loop includes request construction, parsing, preparation, completion
and result destruction. Analysis selects `Routine::profile` subtrees. Both fixtures retain their
exact SVG pre/postflight identities. Current whole-operation samples also reach XML serialization,
bounded writes and active-state error checks; parse/preparation and measurement-report owners
remain visible. This corroborates the next experiment's reachability but does not assign the E2E
gap to those functions or to theme preparation. Raw evidence is under
`target/bench/experiments/packet-kanban-e2e-profile-20261019/`.

A proposed replacement of exact reservation with geometric growth is closed without code:
`BoundedSvgOutput` already uses bounded doubling and explicit byte credit. That design existed in
its original implementation; later admission amortization retained it. `try_reserve_exact` executes
the policy's computed growth target. Replacing it with `try_reserve` could request capacity beyond
`MaxSvgBytes`, even when the next serialized length fits. Preserve allocation-before-write
admission, cancellation, scan work and sticky first failure.

The next shared-writer candidate should independently measure an inline active-state branch with
cold failure conversion, retaining every observation and error boundary. It remains unimplemented
and unmeasured. Separate Radar/State/Sequence ownership repairs continue under their own exact
allocation gates; none is a substitute for attribution of the confirmed Packet/Kanban gaps.
