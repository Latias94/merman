# Gantt Terminal Support

Gantt has Partial semantic coverage and a Diagrammatic family capability; its derived support
level is Partial rather than Summary. The admitted view is **Gantt timeline (elapsed time)**,
followed by complete keyed reading records. Models outside the spatial domain have a
StructuredText primary, reported for that output independently of the family capability.

The semantic reference is pinned Mermaid `12.3.0`. `repo-ref/mermaid-ascii` supplies no Gantt
oracle, and no Grok or Codex renderer code was consulted for this projection.

## Shared time axis

All sections share one continuous elapsed-time axis in epoch milliseconds, from the minimum
task start to the maximum **scheduled** end. Ordinary bars end at `render_end_ms` when supplied,
otherwise at the scheduled end. A milestone uses the midpoint of its scheduled start and end,
including an exact half-millisecond midpoint before integer cell rounding. Visible-end
adjustment never moves that milestone or changes the global domain.

Endpoints round to the nearest terminal cell. A non-milestone interval whose endpoints share a
cell uses `*`; a milestone uses `o`. Independent `D`, `A`, and `!` badges disclose done, active,
and critical flags, including combinations. The heading discloses the axis bounds and cell
resolution; appended records retain the exact instants.
Axis endpoint dates use the operation's time zone and show their UTC offsets. Reading intervals
are explicitly UTC, so a daylight-saving transition cannot silently change the elapsed scale.

The renderer consumes resolved model facts. It does not run another scheduler, compress excluded
days, or read the current clock for a today marker. Includes, excludes, date/tick configuration,
and today-marker settings remain records. Dependency arrows and SVG styling/tick parity are
outside this Partial spatial contract; full literal typed dependency rules remain available.

## Identity and complete records

`T1`, `T2`, and later keys follow source task indices, independently of section grouping or
authored IDs. Declared sections retain each occurrence, including repeated names and empty
sections; `section_index` identifies the occurrence. A long or unsafe label uses its T key in
the chart and retains its exact quoted value below. Label length never changes the time scale.

Every timeline appends the same keyed task and metadata records used by the complete Plain
width fallback. They retain:

- Authored task IDs, labels, sections, occurrence indices, order, and exact UTC interval dates.
- Raw start, scheduled end, and render-end values; complete typed start/end constraints with
  literal dependency IDs, including multi-ID `after` and `until` rules.
- Flags, classes, processing state, previous-task identity, raw compatibility fields, and
  remaining serialized task fields. Raw fields are retained without reinterpretation.
- All serialized metadata, including repeated sections, title/accessibility fields, scheduling
  and presentation settings, links/callbacks as non-executable data, null/default values, future
  fields, and source title/configuration when supplied.

Records stay in source task order and reflow to the requested viewport, using 80 cells without
a bound. Their completeness does not depend on which labels fit beside the bars.
They are deliberately more verbose than the previous task summary. For example, the four-task
admission sample has a 12-row timeline followed by complete records, for 60 rows total at 80
columns. The [admission comparison](../ASCII_PHASE_GATE_REPORT.md#gantt-timeline-admission)
records this vertical cost alongside the spatial gain and the local Trident checker discrepancy.

## Spatial admission and errors

The timeline requires a nonempty task set with a positive global span, empty `displayMode`, and
all of the following for every task:

- `processed` is true and `vert` is false.
- `order` equals the zero-based source task index.
- Declared-section and orphan-section grouping preserves that source task order.
- `start <= visible_end <= scheduled_end`, where visible end is the render-end override or the
  scheduled end.

Compact and unknown nonempty display modes, vertical markers, unprocessed tasks, noncanonical
order, invalid interval bounds, empty models, and zero-span domains retain complete records as
their StructuredText primary. A fitting record primary reports `projection: structured_text`
and `outcome: primary`, without a fallback attempt.

Existing duplicate/empty task ID and ambiguous/invalid section-occurrence errors remain errors.
Resource or allocation failures, cancellation, and deadlines also remain errors, without a
partial result or a budget reset when changing presentation.
Output-byte admission includes the simultaneously retained timeline, serialized projection, and
reading candidate. Plain/ANSI transfer the safe reading buffer directly; HTML releases the JSON
projection before encoding and admits the retained logical text alongside the encoded result.

## Profiles and width

Canonical layout and Strict graph presentation are the only admitted policies. Both structural
charsets and Unicode/CJK width profiles retain the same semantics; CJK uses the shared ASCII
structural fallback. Primary encodings are Plain, ANSI16, ANSI256, TrueColor, and HTML; viewport
Fallback is Plain-only. There is no new schema, dependency, public limit, or larger default budget.

The timeline has a 60–120-cell presentation envelope, with a natural width of 80. Its 26-cell
label column includes the T key and status badges; the chart uses the remaining bounded width.
The requested viewport controls the envelope within those bounds, while records reflow to the
viewport independently. A spatial primary requested below 60 cells therefore overflows: Plain
Fallback selects the complete keyed records, Allow retains the measured primary, and Error
returns `WidthOverflow`. Fallback returns `FallbackUnavailable` if complete records cannot fit.
The report measures the complete output, including records, rather than only the chart.

The [family adapter](../../../crates/merman-ascii/src/gantt.rs),
[timeline](../../../crates/merman-ascii/src/gantt/timeline.rs), and
[record projection](../../../crates/merman-ascii/src/output/gantt.rs) own this contract. The later
[Gantt admission section](../ASCII_PHASE_GATE_REPORT.md#gantt-timeline-admission) preserves the
distinction from the original StructuredText disposition.
