# Architecture Terminal Support

Status: supported subset (`Partial`, diagrammatic primary).

The `diagram-architecture` feature projects
`merman_model::families::architecture::ArchitectureDiagramModel` into terminal output.
Canonical parsing and full presentation hosts share that model; `AsciiDiagram::Architecture`
borrows it directly. Architecture owns spatial interpretation and validation, then uses shared
terminal graph routing, resource accounting, and encoding. It does not require Flowchart or SVG
features, browser geometry, or an intermediate SVG document.

## Spatial semantics and complete records

The spatial subset represents services, junctions, nested groups, and relationships. Terminal
roles and frames approximate browser shapes. Every spatial result must preserve these authored
constraints:

- Source and target retain their independent `L`, `R`, `T`, or `B` sides. A convenient route cannot
  silently substitute another side.
- Arrow ownership remains independent at both endpoints. Headless, source-headed, target-headed,
  and bidirectional relationships keep their declared meaning.
- A `{group}` endpoint retains the referenced service or junction as its anchor and its direct
  owning group as the boundary contact. It is not reassigned to another member or treated as an
  unrelated group node.
- Row and Column alignment hints retain member order. Incompatible hints or side constraints
  cannot be satisfied by silently sorting, dropping members, or reversing a declaration.
- Service identity and group ownership remain separate from display titles. Equal titles do not
  merge declarations.

Complete keyed reading records accompany spatial output and provide the primary representation
when detached geometry cannot be interpreted honestly. Records retain node and group identities,
roles, ownership, titles, icon metadata, edge endpoints and sides, arrow/group flags, alignment
hints, accessibility fields, and source title/configuration. They describe the final canonical
model, including invalid or conflicting values, rather than inventing missing geometry.
Browser icon assets, fonts, and pixel layout are outside the terminal contract.

Services use rectangles with inline keys and titles; junctions use keyed diamonds marked `[+]`.
Nested groups use frames. Short `E1...` route labels connect the overview to complete captions and
endpoint records.
Junction diamonds denote one shared semantic junction, regardless of the number of incident
edges; an ordinary crossing does not create one. Browser icons remain named metadata rather
than ambiguous substitutes from an unrelated terminal symbol set.

The family plans two axes together before routing. Explicit Row and Column hints share node
centers and retain their ordered chains, including across groups. Frames retain independent
minimum and maximum boundaries, so aligned members do not force their groups to share centers.
Containment and sibling-separation constraints form two acyclic graphs; measured text reweights
those constraints without changing their accepted order. Side demand reserves contour capacity
and route gaps. Port-derived relative placement is a soft preference: explicit alignment may
override it while every actual connection still uses its declared side. Exclusive opposing
ports also propose shared centers for straight spines. These optional equalities are removed
if they conflict with the complete scene; multi-edge fans keep room to spread.

The family-local tests own detailed placement and routing coverage. This contract does not
promise that every parser-accepted architecture has a valid terminal scene or fits a particular
viewport. A successful small service pair is not evidence of unrestricted nested or dense
spatial coverage. There is no browser pixel-parity or reference ASCII byte-parity claim.

## StructuredText and operation boundaries

Unresolved identities, ambiguous ownership, invalid detached values, and incompatible explicit
alignment constraints use a complete **StructuredText primary**. Such output reports
`projection: structured_text` and
`outcome: primary` when it fits, without claiming that fallback was attempted. The records keep
the original declarations and constraints visible. Spatial interpretation must not resolve a
conflict by moving an endpoint to a different declared side or dropping an alignment hint.

This semantic classification is distinct from a failure to route an admitted spatial scene.
Strict routing and port-capacity failures remain errors, including when width fallback was
requested. Parse failures, resource/allocation failures, cancellation, and deadlines also remain
errors. A complete-record path does not grant a fresh operation allowance or bypass a terminal
operation state.

## Profiles, encoding, and width

Architecture admits Canonical layout and Strict graph presentation. Both `unicode` and `cjk`
width profiles and the five standard primary encodings are available: Plain, ANSI16, ANSI256,
TrueColor, and HTML. ASCII and Unicode structural charsets share the same semantic constraints;
the CJK width profile uses the shared single-cell structural fallback. Compact, Auto, and
Adaptive are not admitted.

Width applies to the complete emitted document, including its reading records. Allow retains a
wide primary; Error returns `WidthOverflow`. Plain viewport Fallback can reflow complete keyed
records after a valid primary exceeds the bound, or return `FallbackUnavailable` if they cannot
fit. Styled viewport fallback is rejected during admission, even when that input would fit.
Use the measured output report to inspect actual extents; the capability row does not establish
a universal spatial size bound.

## Implementation and verification owners

The selected Mermaid 12.3.0 revision is
`2086904e76c692331e34a45d7cc3bc30860d040e`. Its
[`architectureDb.ts`](https://github.com/mermaid-js/mermaid/blob/2086904e76c692331e34a45d7cc3bc30860d040e/packages/mermaid/src/diagrams/architecture/architectureDb.ts)
owns endpoint identity, direct group membership, independent arrow flags, and ordered alignment
members. [`architectureRenderer.ts`](https://github.com/mermaid-js/mermaid/blob/2086904e76c692331e34a45d7cc3bc30860d040e/packages/mermaid/src/diagrams/architecture/architectureRenderer.ts)
establishes explicit alignment priority over geometric heuristics, and
[`svgDraw.ts`](https://github.com/mermaid-js/mermaid/blob/2086904e76c692331e34a45d7cc3bc30860d040e/packages/mermaid/src/diagrams/architecture/svgDraw.ts)
establishes per-end arrow direction and junction ownership. The terminal uses real frame
contacts aligned with the anchoring member, rather than the SVG renderer's fixed pixel offset.

- [`ArchitectureDiagramModel`](../../../crates/merman-model/src/families/architecture.rs) owns
  canonical nodes, groups, edges, and layout hints.
- [`src/architecture`](../../../crates/merman-ascii/src/architecture/mod.rs) owns spatial
  interpretation and rendering; [`output/architecture.rs`](../../../crates/merman-ascii/src/output/architecture.rs)
  owns complete records.
- [`architecture_model.rs`](../../../crates/merman-ascii/tests/architecture_model.rs) owns
  family-local semantic, routing, record, resource, and cancellation coverage.
- [`ascii_api.rs`](../../../crates/merman/tests/ascii_api.rs),
  [`ascii_smoke.rs`](../../../crates/merman-cli/tests/ascii_smoke.rs), and the
  [binding ASCII tests](../../../crates/merman-bindings-core/src/ascii.rs) own host parity and
  report transport for a small service pair and Plain width fallback.
- [`fixtures/architecture`](../../../fixtures/architecture/) and the selected Mermaid revision
  in [`REPOS.lock.json`](../../../tools/upstreams/REPOS.lock.json) own source specimens and the
  upstream semantic baseline.

The [support matrix](../ASCII_SUPPORT_MATRIX.md), [gap registry](ASCII_GAP_REGISTRY.md), and
[reference comparison](ASCII_REFERENCE_COMPARISON.md) describe the shared admission boundary.
