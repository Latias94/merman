# Sequence Fixture Gaps

Status: Five named Unicode geometry corrections; copied fixture bytes remain immutable
Last updated: 2026-10-10

## Copied Upstream Fixtures

The immutable copied `mermaid-ascii` corpus contains 17 sequence fixtures. Every fixture must
render successfully. The existing comparison only removes trailing row spaces, outer blank
rows, and CRLF differences; it does not remove internal whitespace, lifelines, or arrows.

| Fixture group | Exact copied-output matches | Named corrected outputs | Total |
| --- | ---: | ---: | ---: |
| `sequence` | 7 | 5 | 12 |
| `sequence-ascii` | 5 | 0 | 5 |
| Total | 12 | 5 | 17 |

The named subset is explicit in [the executable fixture tests](../../sequence_model/fixtures.rs).
Its corrected outputs are captured separately in
[`corrected-fixtures`](../../sequence_model/corrected-fixtures/); the copied reference inputs and
outputs are unchanged. Corrected outputs also undergo exact comparison under the same narrow
normalization. No new fixture automatically inherits a disposition.

## Named Unicode Geometry Corrections

The copied renderer computes participant centers from actor names and fixed spacing before
considering message labels. Writing a longer label onto a lifeline row can erase another actor.
Merman measures unwrapped messages first and assigns each label to an adjacent-lifeline interval;
self messages also reserve their loop geometry. One blank cell separates text from the next actor.

| Copied fixture | Unsafe or unreadable copied behavior | Corrected behavior |
| --- | --- | --- |
| `dotted_arrows_only.txt` | `Return value` erases the target lifeline on its label row. | Widens the actor interval and retains the dotted shaft, target head, and both lifelines. |
| `four_participants.txt` | The nonadjacent `Response` label touches the intermediate B lifeline without a reading space. | Widens only the A-to-B label-host interval by one cell; preserves the D-to-A return signal. |
| `multiword_labels.txt` | The long request erases the System lifeline. | Fits the complete unwrapped request before System and preserves the reverse reply. |
| `self_message.txt` | `Self call` and `Then to B` erase B's lifeline on their label rows. | Reserves the A-to-B interval for both complete labels and keeps B outside A's self loop. |
| `three_participants.txt` | Request and reply labels overwrite WebApp or Database lifelines. | Reserves the relevant adjacent intervals and retains all three actors and four signal directions. |

These are verified geometry corrections for issue #185, not permitted text or topology loss.
The named tests require each authored label exactly once, an unchanged lifeline for every actor
on every label row, an admitted label host with a reading space, continuous normal signal shafts,
correct source junctions and target heads, and a closed self loop that clears foreign actors.
The independent message-spacing regressions also cover nonadjacent endpoints, reverse messages,
numbering, terminal normalization, explicit wrapping, Compact layout, and Unicode/CJK widths.

## Upstream Algorithm Boundary

The copied upstream sequence renderer is intentionally small. It covers:

- participant declarations and aliases
- implicit participants from messages
- participant boxes and lifelines
- `->>` solid messages
- `-->>` dotted messages
- self messages
- message labels
- autonumber
- ASCII and Unicode character sets

It does not cover activation boxes or loop/alt/opt/par blocks in the upstream README checklist.

## Merman Product Gaps

`merman-ascii` consumes `merman-core` typed sequence models, which are richer than the upstream
`mermaid-ascii` parser. Product gaps beyond copied fixture parity are tracked in
`crates/merman-ascii/SEQUENCE_SUPPORT.md` and the `ascii-sequence-parity` workstream.
