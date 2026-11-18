# ZenUML Terminal Support

Status: supported subset (`Partial`, diagrammatic primary).

The `diagram-zenuml` feature projects
`merman_model::families::zenuml::ZenumlDiagramModel` into terminal output.
`AsciiDiagram::Zenuml` borrows that canonical model directly in both canonical and full
presentation adapters. ZenUML owns statement interpretation, identity validation, and lifeline
planning. Shared terminal primitives provide display-cell accounting, encoding, and operation
limits. This feature is independently selectable from Sequence, SVG, and browser layout.

## Lifelines and statement ownership

The primary scene uses participant columns and lifelines. Stable `P1`, `P2`, and later participant
keys follow model declaration order. Display labels never replace participant names as identity;
equal labels do not merge distinct participants. Stable `S1`, `S2`, and later statement keys
follow depth-first statement order, including statements nested in call bodies and control
sections. Reading keys connect the scene to the complete records appended below it.

The admitted scene represents these typed semantics:

- Kind prefixes distinguish asynchronous messages (`async`), synchronous calls (`call`), returns
  (`return`), and creation (`new`), including with the ASCII structural charset.
- Self messages retain a self path. Nested calls retain their authored order and ownership.
- Assignments across distinct participants add a reverse `result` arrow from the callee back to
  the caller; complete records retain the authored assignment.
- An admitted created participant's lifeline begins at its creation point. Creation inside a
  control fragment uses complete StructuredText because its conditional lifetime is ambiguous.
- Loop, alternative, parallel, optional, critical, section, and try/catch/finally fragments retain
  their nested frames and individual sections, including section labels. Direct items of a `par`
  fragment have visible separators.
- Participant groups have heading bands; references retain their participant scope, and
  dividers retain their place in statement order.

Terminal boxes, message paths, and control frames approximate browser geometry. The renderer
does not establish browser pixel parity or reference ASCII byte parity. A successful small
lifeline scene does not establish that every dense or deeply nested scene fits a given viewport.

## Complete reading records

Complete ZenUML records accompany the scene and also serve as its width fallback. They retain
the original canonical data instead of reconstructing a reduced model from visible arrows:

- Participant names, labels, types, stereotypes, emoji, authored width values, colors, comments,
  group membership, explicit/starter flags, declaration spans, and occurrence spans.
- Group identities, ordered participant membership, and spans; model title and starter identity.
- Every statement's original identity, kind, comment, and span. Message and creation records
  retain explicit/resolved ownership, labels, assignments, styles, constructor/parameter values,
  nested body ownership, and body comments where present in that statement kind.
- Return endpoints, fragment kinds and labels, ordered sections with their labels/comments/spans,
  reference participants and labels, and divider labels.
- Source title and configuration supplied by the parsed envelope, plus retained envelope fields.

Serialization flattens statement and section ownership iteratively, preserving traversal order
and complete values without recursively serializing nested bodies. Record construction,
serialization, measurement, and reflow share the existing operation controls and budgets.
Flat serialization does not grant unlimited depth, size, work, or output bytes. A bare borrowed
model has no source envelope; it can only disclose source metadata when that metadata is supplied
by a source-backed input.

## StructuredText and errors

Detached models can contain missing references, duplicate identities, invalid group ownership,
or invalid lifecycles. When those values prevent an unambiguous scene, complete records become
the **StructuredText primary**. The renderer preserves the original declarations and endpoints;
it cannot merge duplicates, guess a missing participant, or move a creation point to make a scene.
When this primary fits the request, its report says `projection: structured_text`,
`outcome: primary`, and no fallback attempt. The family's capability still describes its admitted
diagrammatic subset.

Creation inside any control fragment also uses a complete StructuredText primary: a single
unconditional creation point would misrepresent a conditional or repeated lifetime. A reference
with neither explicit participants nor a lexical origin likewise retains complete records as its
primary. The renderer does not assign such a reference to every participant column.
Missing nested senders or return targets do not fall back to a different starter. Only the
model producer's implicit root starter and its corresponding return can use that default.
References count as uses for creation ordering; a reference before creation retains records.

This classification applies to semantic ambiguity before spatial rendering. It does not recover
from parse errors, resource/allocation failures, cancellation, or deadlines. Those remain errors
with no partial result. Geometry and records use the same operation allowance; changing the
projection or requesting width fallback never increases a budget or resets terminal operation
state.

## Profiles, encodings, and width

ZenUML admits Canonical layout and Strict graph presentation, both `unicode` and `cjk` display-width
profiles, and Plain, ANSI16, ANSI256, TrueColor, and HTML primary encodings. ASCII and Unicode
structural charsets retain the same semantic ownership; the CJK width profile uses the shared
single-cell structural fallback. Compact, Auto, and Adaptive are not admitted.

The viewport applies to the complete output, including appended records. Allow retains a wide
primary. Error returns `WidthOverflow`. Plain Fallback can reflow the complete records after a
valid primary exceeds the requested width; it returns `FallbackUnavailable` if those records
cannot fit. A successful width fallback reports `projection: structured_text`, `outcome: fallback`,
and `fallback_reason: primary_overflow`. Styled viewport fallback requests are rejected during
admission, including when the input would otherwise fit.

Inspect measured primary and emitted extents in the output report. Capability admission does not
promise a universal spatial extent or dense-scene bound. Terminal quantization and browser-only
presentation details remain explicit limits of the Partial contract.

## Implementation and verification owners

- [`ZenumlDiagramModel`](../../../crates/merman-model/src/families/zenuml.rs) owns participant,
  group, statement, section, and resolved endpoint semantics.
- [`AsciiDiagram`](../../../crates/merman-ascii/src/model.rs) owns the borrowed variant and
  canonical/full adapter boundary. The [family adapter](../../../crates/merman-ascii/src/zenuml/mod.rs)
  owns the scene; the [output projection](../../../crates/merman-ascii/src/output/zenuml.rs)
  owns complete flat records.
- [`zenuml_model.rs`](../../../crates/merman-ascii/tests/zenuml_model.rs) owns detailed family
  semantics, invalid detached identities/groups/lifecycles, metadata preservation, viewport,
  encoding, resource, cancellation, and deadline coverage. Output-module unit tests cover
  controlled projection traversal and typed traversal-allocation errors.
- [`ascii_api.rs`](../../../crates/merman/tests/ascii_api.rs) compares canonical/full output and
  reports for one basic scene and its complete Plain width fallback.
- [`ascii_smoke.rs`](../../../crates/merman-cli/tests/ascii_smoke.rs) verifies CLI capability
  admission, detector mapping, primary output, complete fallback, and report transport.
- The [binding ASCII tests](../../../crates/merman-bindings-core/src/ascii.rs) verify the same
  primary/fallback result plan and rejected output combinations with `diagram-zenuml` selected
  independently. The [host render contract](../../../crates/merman-bindings-core/tests/host_render_contract.rs)
  compares cached and one-shot hosts while preserving source title/configuration records.
- The [Playground capability tests](../../../playground/src/lib/ascii-support.test.ts) verify
  generated support labels and admission arrays. Regenerate that projection with
  `cargo run -p xtask -- gen-playground-ascii-capabilities` after changing the capability owner.
- [`fixtures/zenuml`](../../../fixtures/zenuml/) and the
  [ZenUML compatibility notes](../../alignment/FAMILY_NOTES.md#zenuml) own source specimens and
  the upstream semantic boundary.

Host tests cover a small complete example and its width fallback. They complement the detailed
family tests and do not replace dense-scene, lifecycle, or operation-boundary evidence.
The [support matrix](../ASCII_SUPPORT_MATRIX.md) and [gap registry](ASCII_GAP_REGISTRY.md) retain
the shared admission contract and remaining work.
