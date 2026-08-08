# Semantic-token reducer decision — 2026-08-08

## Decision

The one-pass semantic-token reducer is **accepted-structural**. The optional public latency and
transient-memory claims are **rejected for this program** because no independent full/range
LSP/WASM allocation or adjacent timing receipt was registered; the structural deletion remains
useful and no broader active-set rewrite is retained.

## Revision boundary

| Role | Commit | Tree | Meaning |
|---|---|---|---|
| `A` | `c641cbf2daa80c8c69afc9aecd257df18a97a0d2` | `ca48f399ec76ee0ac45ed7f6dbf68f35381744ac` | Direct parent with per-interval precedence/narrowness passes and a `finalists: Vec`. |
| `B` | `65e294e544fe57ed852c8d23dc18f1a2d3e01a31` | `ff31e8c06708e24aba7423cc5c84cc8ba58193fa` | One active scan that tracks winner, conflict evidence, and modifier bits. |

The full-index candidate diff SHA-256 is
`fe41abfe5260b1c75f4922d25dcc255d3804869398400b23cd32c3964481acb4`; its stable patch-id is
`04ec82528959d9fc723e788778111c25ec1409ff`. Only
`crates/merman-editor-core/src/token_planner.rs` changes in this adjacent pair.

## Structural claim

For each active interval, `choose_candidate` now performs one scan. It retains the first winning
candidate, the first equal-rank/equal-width conflicting kind, and an OR-ed packed modifier mask.
The former three passes and per-interval `finalists` allocation are gone. Modifier validation also
stores packed bits or one duplicate-modifier error rather than one heap vector per candidate.

This does not claim that the complete planner is `O(N)` in all dimensions: boundary sorting,
active-set maintenance, source-map conversion, and public result encoding remain. It claims only
removal of the redundant finalist construction and precedence/narrowness rescans at the owner
boundary.

## Semantic controls

The private reference reducer remains in tests and is compared exhaustively over every non-empty
subset and both natural/reversed active orders for a seven-candidate overlap set. Additional tests
cover narrower-candidate conflict discard, modifier OR, duplicate modifiers, invalid spans,
lexical overlap, UTF-16/multiline splitting, adjacent token merging, and packed-token length.
Family token fixtures cover Architecture, Block, C4, EventModeling, GitGraph, Ishikawa, Kanban,
Langium, Mindmap, Railroad, Requirement, Timeline, Wardley, XYChart, and the line-parser families.

The LSP and WASM callers continue to consume the same `PlannedToken` and packed five-field ABI;
full and range entry points share the same reducer and preserve legend order, origin precedence,
narrowest-span choice, conflict errors, modifier OR, and range clipping.

## Claim boundary and cleanup

No finalists vector, redundant precedence pass, or modifier small-vector compatibility path remains
in production. The public ABI and error ordering are unchanged. Because the branch has no
decision-grade full/range LSP/WASM latency or allocation receipt, this document intentionally does
not claim `accepted-latency` or `accepted-memory`; a future candidate must register those lanes
against a current adjacent base before making either claim. No dormant switch or candidate-only
instrumentation was added.
