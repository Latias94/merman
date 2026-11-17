# Usecase ASCII Support

Status: supported subset (`Partial`, diagrammatic primary).

The Usecase adapter consumes `merman_model::families::usecase::UsecaseDiagramModel` through
the public parsed or borrowed-model renderer. It does not parse source or reconstruct parser
resolution. The `diagram-usecase` feature selects this adapter independently of Flowchart and State.

## Spatial support

| Capability | Terminal behavior |
| --- | --- |
| Actors | Rectangular terminal nodes labeled `[actor]`, `[actor: hollow]`, `[actor: awesome]`, or `[actor: icon]`. These preserve typed roles rather than reproduce SVG actor silhouettes. |
| Use cases | Ellipses use stadium contours with `[use case]`; rectangular use cases use rectangular contours with `[use case: rect]`. |
| Business, stereotypes, icons | Visible `[business]`, `[stereotype]`, and `[icon]` records within the node label retain the authored values. Icon names are text; no icon font or asset lookup runs. |
| Boundaries and packages | Flat group frames retain member ownership and visible `[boundary]` or `[package]` titles. Empty boundaries remain groups. |
| Notes | Document-shaped nodes labeled `[note]`, with independent dotted, markerless attachments to their actual targets. Notes inside a boundary follow their target's membership. Exact SVG note positioning is approximate. |
| Associations | Written source and target order, point and reversed-point arrows, markerless lines, circles, reversed circles, crosses, and reversed crosses are preserved independently. |
| Include and extend | Visible `<<include>>` and `<<extend>>` relationship labels, the stored stroke and endpoint markers, and additional authored labels are preserved. Parser-produced include/extend relationships are dotted. |
| Generalization | A visible `<<generalization>>` label distinguishes it from association. The shared point marker approximates the SVG hollow triangle. Actor-to-actor and use-case-to-use-case roles remain distinct. |
| Relationship length | The stored positive `minlen` participates in shared graph rank spacing. |
| Directions | `LR`, `RL`, `TB`/`TD`, and `BT`; reverse directions use the shared terminal transform and keep labels readable. |
| Terminal text | ASCII and Unicode structure, Unicode and CJK display-width profiles, emoji/grapheme-safe measurement, and terminal-control escaping use the shared text and graph implementation. ASCII structure does not transliterate authored Unicode labels. |

Small and typical diagrams use the shared spatial graph renderer. The family admits only Canonical
layout and Strict graph presentation. Flowchart's Compact, Auto, and adaptive recovery policies do
not apply to Usecase. A dense graph may return `RoutingSearchExhausted`; that result means the
bounded planner did not find a valid scene, not that Usecase syntax or topology is unsupported.
Independent endpoint capacity failures remain `PortCapacityExceeded`. Routing indexes enumerate
authored relationships first, followed by note attachments in note order.

## Width behavior and complete reading records

A valid spatial scene follows the requested viewport policy:

- Allow retains the complete primary, reporting `WideAllowed` when necessary.
- Error returns `WidthOverflow` with the measured width and width profile.
- Plain Fallback emits a complete keyed reading document, reporting `StructuredText`,
  `PrimaryOverflow`, and `PresentationOnly` lossiness.

Reading records give each node an `N` key, each boundary a `G` key, each relationship an `E` key,
and each note a `T` key. Declarations retain quoted original IDs and labels. Relationships explicitly
name written source and target, independent endpoint markers, stroke, type, and ID. Parent and
member keys identify the same declarations; note keys identify their target nodes.

All remaining serialized fields stay with their owner as complete detail records. These include
label types, actor metadata, styles and classes, relationship IDs and explicitness, animation,
lengths, the original membership/parent fields, class definitions, accessibility, title, and authored
source configuration/title. The writer borrows opaque JSON from the single admitted serialization;
it does not build another JSON value tree or truncate fields. Shared grapheme-safe wrapping controls
the emitted width. Resource failures and cancellation propagate without generating partial records.

Primary diagrams show literal Markdown source text with terminal label breaks rather than browser
Markdown styling. CSS classes/styles, animation, accessibility, title, and source configuration are
retained by the complete reading document; SVG styling, interaction, animation, and icon glyphs are
not spatially reproduced. Opt-in ANSI/HTML output uses the shared graph's renderer-owned colors.

## Explicit rejection and validation

JSON table nodes return `UnsupportedFeature { diagram_type: "usecase", feature: "JSON table nodes" }`
under every viewport policy. Their ordered JSON properties, UTF-16 strings, and non-finite number
metadata require a dedicated table projection; treating them as ordinary node labels would lose
semantics. Width fallback does not turn this semantic rejection into accepted support.

Borrowed direct models receive family validation before graph allocation. Empty/duplicate entity
and relationship identities, explicit-edge identity collisions, missing endpoints and note targets,
invalid actor/business metadata, invalid endpoint roles, and zero relationship lengths fail explicitly.
Boundary members must exist, be unique, have one owner, and agree with node `parent_id` in both
directions. Boundary membership cycles receive their own error; acyclic nested boundaries are
explicitly unsupported because the canonical grammar admits flat system boundaries.

Graph keys are generated in separate node, boundary, and note namespaces. A generated note
ID may therefore equal a direct-model node ID without shadowing it. Class definitions occupy their
own namespace. Structural indexes, deferred source copies, normalized text, retained edge/group
labels, and graph work use the existing resource ledger. Aggregate work, document cells, and bytes
are checked before materializing a graph or large source labels. Cancellation and resource errors
retain the caller's operation ownership; recoverable width overflow preserves spent planning work.

## Executable evidence

`tests/usecase_model.rs` covers public canonical/full-model entries, small/typical/CJK and dense
fixtures at 80/100/120 columns, character sets and width profiles, direction transforms, markers,
roles, metadata, membership, complete fallback records, malformed models, exact grid quotas, and
resource/cancellation propagation. Family unit tests additionally cover aggregate raw-label bytes
and display cells, exact versus one-below work quotas, cancellation inside large label scans, and
transaction rollback. Local fixtures live under `tests/testdata/local-semantic/usecase/`.

Run the family checks with the repository's serialized build/test workflow:

```text
cargo nextest run -p merman-ascii --features diagram-usecase --test usecase_model -j 2
cargo nextest run -p merman-ascii --features diagram-usecase --lib -E 'test(usecase::)' -j 2
```

The test matrix requires spatial success for small and typical diagrams. Dense cases permit a
truthful routing-search failure, or a valid primary/width fallback with accurately measured report
metadata; they do not accept an unrelated unsupported-feature or resource error as evidence.
