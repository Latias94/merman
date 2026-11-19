# GitGraph Terminal Support

GitGraph has Partial semantic coverage and a Diagrammatic family capability. Its admitted
projection is **Git branch tracks**, followed by complete keyed records. Models outside this
spatial contract have a StructuredText primary; a fitting record primary reports `primary`
without attempting a fallback. Source semantics follow pinned Mermaid `12.3.0`.

## Branches, time, and connections

Branch tracks follow the canonical model's branch array, which already contains Mermaid's
stable branch-order sorting. Commit events follow the strictly increasing `seq` order, with
uniform spacing. Sequence values need not be consecutive or start at zero. LR advances right,
TB advances down, and BT advances up. BT does not reverse branch order. Empty branches retain
their own B key without invented commits or branch-creation edges.

Every ancestry line joins actual parent and child occurrences. Normal branch forks depart near
the parent; merges approach the child. Shared segments require a common actual endpoint, and
unrelated crossings use `x` (ASCII) or `╳` (Unicode) rather than a junction. Lines are monotone
along the event axis. Endpoint arrows point to the child; different parents join only at its
actual marker. The arrows prevent a reused source trunk from looking like ancestry from an
earlier merge on that trunk. The marker legend distinguishes normal, reverse, highlight, merge, and
cherry-pick appearances. A custom merge appearance changes its marker, never its two parents.

Mermaid stores a cherry-pick source in `parents[1]`. The terminal chart draws only its first
parent as ancestry and explicitly records `cherry-picked from` alongside the complete ordered
parent list. This differs from Mermaid SVG's source connector intentionally: a picked source
must not look like an additional ancestry parent. Both entries are retained when their IDs
match. A selected mainline for picking a merge is not independently available in the model;
the renderer does not recover it by guessing from tags.

Branch heads, branch creation instants, inherited heads of branches without commits, and the
authored merge branch name are absent from the typed model. The renderer does not invent those
facts or draw branch lifelines beyond known ancestry. Layout styling, browser labels, tag
shapes, `showBranches`/`showCommitLabel`, and SVG pixel parity are outside this Partial contract;
source configuration and all labels remain complete records.

## Identity and records

`C1..Cn` identify commit occurrences in model order; `B1..Bn` identify branch occurrences in
canonical branch order. Short, safe branch names appear beside horizontal tracks when space
permits. Long or unsafe names retain their B key; every exact quoted name remains in records.
Commit markers carry C keys so label length cannot distort the event spacing or cover routes.

Each primary and Plain width fallback retains the same family-owned records:

- Commit IDs, literal branch names, messages, ordered tags, and ordered parent IDs.
- Numeric `seq` and real `type`, custom display types, and explicit false ID overrides.
- Every branch occurrence, current branch, direction, title, accessibility fields, and warnings
  including rule IDs and spans. Unknown future serialized fields remain details.
- Source title and source configuration, including presentation settings and inert data.

Quoted framing distinguishes authored delimiters, empty strings, whitespace, and controls.
Repeated IDs never erase occurrences in a detached model: C keys remain distinct, while literal
references stay ambiguous. A parser warning about an overwritten ID preserves the remaining
canonical facts and warning; it cannot reconstruct the event already overwritten by the parser.

## Spatial admission

The spatial domain requires a nonempty commit set, LR/TB/BT direction, unique declared branch
names, unique commit IDs, strictly increasing sequence, and one declared branch for every
commit. Parents must identify earlier commits. The bounded route construction also requires:

- At most two ordered parent entries; identical entries are admitted for cherry-pick roles.
- Same-branch ancestry refers to the preceding event on that branch.
- Cross-branch ancestry either starts the target branch's first event or refers to the source
  branch's latest event before the child.
- Reused straight segments share an actual endpoint; crossings do not obscure endpoints,
  commits, or their keys. A branch does not contain multiple disconnected roots.
- `parallelCommits` is false/absent and custom appearance overrides are known NORMAL,
  REVERSE, or HIGHLIGHT values. Duplicate-ID overwrite warnings are absent.

Other models use complete record primaries with a reason. This includes legal histories such as
merging an inherited old head from a branch without its own commits: record admission is a
presentation boundary, not a claim that the Mermaid source is invalid. `parallelCommits=true`
uses a different upstream event axis and remains records until it has a separate contract.

Unknown direct-model real commit types remain errors. Resource/allocation failures, cancellation,
and deadlines also remain errors. No branch-count ceiling, public limit, dependency, schema
change, or larger default resource budget is introduced. Canvas allocation and every route
step use the shared grid/work policy. Retained tracks, serialized facts, and reading output are
admitted cumulatively; HTML additionally admits retained logical text while encoding it.

## Terminal policies and evidence

Canonical layout and Strict presentation are admitted. ASCII/Unicode structures and Unicode/CJK
width profiles retain the same identities and topology; CJK uses the shared ASCII structural
fallback. Primary encodings are Plain, ANSI16, ANSI256, TrueColor, and HTML. Records wrap to the
requested viewport, or 80 cells when no width is supplied.

Tracks never wrap or rotate to force a fit. `Allow` retains their measured extent; `Error`
returns `WidthOverflow`; Plain `Fallback` selects the complete records. If a complete safe
record cannot fit, the existing `FallbackUnavailable` error remains. Full records increase total
height compared with the previous history summary; the spatial overview is the readability
gain, not a promise of smaller total output or universally readable dense histories.

The [adapter](../../../crates/merman-ascii/src/git_graph.rs),
[track planner](../../../crates/merman-ascii/src/git_graph/tracks.rs), and
[records](../../../crates/merman-ascii/src/output/git_graph.rs) own this contract.
[Parser-backed tests](../../../crates/merman-ascii/tests/git_graph_tracks.rs) trace rendered
connections backwards along time, treating crossings independently, and compare the reachable
parents against the typed model. The [admission report](../ASCII_PHASE_GATE_REPORT.md#gitgraph-branch-track-admission)
records the representative widths and tradeoffs. `repo-ref/mermaid-ascii` has no GitGraph
implementation or accepted byte oracle. No Grok or Codex renderer code was consulted.
