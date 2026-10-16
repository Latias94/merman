# Theme rendering: runtime responsibilities from first principles

This is the pre-implementation analysis. The
[cleanup checkpoint](../performance/native_theme_runtime_cleanup_macos_2026-10-16.md) records the
implemented responsibility changes, intentional contract migrations and fresh verification.

Date: 2026-10-16. Status: proposed architecture, not an implementation or accepted API migration.
Source: `3d8a738d4` with the two previously recorded build repairs. This analysis does not change
production code or replace the [measured regression checkpoint](../performance/native_theme_root_cause_macos_2026-10-16.md).

## Position

Theme support does not inherently require interpreting the completed SVG as unknown XML, analyzing
all downstream reference expansion, reserving a retired text-ID namespace, or constructing a chain
of content/evidence hashes on every ordinary render. Those are different responsibilities that have
accumulated on the default SVG path.

Redesign the ownership first. Ordinary generation should pay for effective style resolution,
layout, correct serialization, and bounded work actually performed by the generator. Raw SVG
admission, browser publication, native conversion, strict portability, and qualification evidence
should pay for their own additional requirements. Avoid adding a default-on certification layer
and then rebuilding a complete SVG proof framework just to make that layer cheaper.

This is a contract-redesign proposal. It deliberately reopens some recently introduced error,
metadata, and resource-accounting contracts. It must not be presented as a behavior-preserving
optimization admitted under the old benchmark rules.

## What the theme feature actually requires

A renderer that supports a theme must:

1. Validate the authored theme's shape, numeric domains, supported paint/effect values, and resource
   declarations when compiling it.
2. Resolve inheritance, Mermaid calculation/replay order, source precedence, and actual family
   support correctly.
3. Use the same resolved typography and geometry inputs for measurement, layout, and emission.
4. Serialize selected paints, definitions, references, text, and attributes correctly.
5. Explain or reject unsupported requested behavior according to an explicit application/strict
   policy, without silently substituting a different theme or output pipeline.

These requirements do not entail a second general interpretation of the final XML. An XML walk
cannot, by itself, prove that the correct theme won a precedence decision or that browser CSS
produced the intended visual result.

Typed effects are a concrete example. `diagram_theme/effects.rs:36` exposes a finite primitive
vocabulary; numeric checks, reference checks, input ordering, and resource receipts already exist
at construction/compilation. The writer owns the resulting filter definitions and application
sites. A typed theme is not an arbitrary SVG program supplied by a caller.

The useful distinction is between user-visible application decisions and internal implementation
consistency assertions. Keep actual unsupported/source-precedence outcomes available. Reconsider
per-occurrence expectation collections that only verify that one part of this implementation agrees
with another part on every call. Unit, differential, DOM, raster, and integration tests are the
proper place for many such consistency proofs. Runtime checks still belong where caller input or
an actual requested guarantee requires them.

The local Mermaid reference supports this separation. In
`repo-ref/mermaid/packages/mermaid/src/themes/theme-default.js:478-510`, theme calculation applies
overrides, derives colors, and replays overrides. `config.ts:24-74` combines configuration and
selects the theme variables. Those operations do not require examining generated SVG. Mermaid
does perform a separate whole-output `DOMPurify.sanitize` in `mermaidAPI.ts:627` for its applicable
browser security modes. That is a real publication responsibility, not evidence that the theme
algorithm needs an XML readback or that all completed-output inspection is dispensable.

## What the current path really buys

| Current work | Actual purpose | First-principles disposition |
| --- | --- | --- |
| Theme schema/color/effect validation | Reject invalid authored definitions | Keep at compilation; apply per-operation policy restrictions when needed |
| Source precedence and family applicability | Select the style that must actually render | Keep in resolution/preparation |
| XML escaping, finite geometry, label sanitization | Correctly construct markup from caller values | Keep at the producer; close existing producer gaps before dropping readback |
| Source/layout/output allocation budgets | Bound work and retained data inside Merman | Keep before the relevant work/allocation |
| Completed-output XML walk | Admit arbitrary bytes; also catch producer serialization defects | Use at opaque-input/output and explicit validation seams; do not make a second parse intrinsic to trusted generation |
| Reference-expansion analysis | Bound downstream use/marker/filter/image expansion | Keep before native/backend consumption; admit imported fragments under a clear policy; separately define any browser-publication guarantee |
| `merman-prepared-` whole-output search | Enforce a historical text-evidence namespace prohibition | Retire the inactive protocol and its consumers together if no supported producer remains |
| Resource/artifact/document/evidence/receipt hashes | Correlate immutable artifacts, reports, and qualification records | Remove mandatory computation from ordinary string output; retain only for a concrete requested identity/audit use |
| Theme application diagnostics | Explain applied, overridden, or unsupported settings | Produce where requested/needed; do not build and discard a full result for bytes-only calls |
| Strict portability/native finalization | Make a consumer-specific claim about actual output | Keep as an explicit, target-owned operation; never certify different bytes |

## Direct evidence that ordinary rendering is overcharged

### Full observation still ends in Unverified

`svg/pipeline/standalone.rs:104-113` performs XML/resource-budget observation for BestEffort ordinary
output, then returns `Unverified` without a finalization report. `render/document.rs:504` nevertheless
hashes the public SVG and builds document, target-evidence, and receipt identities.

This observation does enforce XML and resource errors; it is not merely dead code. But its product
meaning is distinct from applying a theme and distinct from certifying portability. A failure to
prove native compatibility is not proof that ordinary vector generation is unusable.

### Result identities do not drive the ordinary consumers

`render/target_admission/mod.rs:834` enforces RequirePortable by checking the status, not a digest.
The bindings projection in `theme_execution_evidence.rs:52-64` reads family/theme state, mutation,
target state/reasons, and font source, not the content/evidence digests.

There are real build-then-discard paths:

- `merman-bindings-core/src/render/request.rs:68` constructs theme metadata even for a data-only
  operation; `operation.rs:326` subsequently returns only bytes.
- Web `renderSvg` calls the full operation-result path and then drops metadata; `renderSvgResult`
  is the result-oriented entry point. Playground's preview uses `renderSvg`.
- Node's convenience methods execute/decode a complete result before selecting `result.data`.
- CLI consumes the fallback-CSS residual reason for a warning and discards the rest of the SVG
  result evidence. It does not use content or receipt digests.

Rust exposes experimental digest getters, so removal is an interface migration, not an assumption
that external callers cannot exist. Inside this repository, the strongest consumers of the full
identity chain are private qualification/acceptance tools. Those tools can request or compute their
own artifact identities rather than making every user's render pay for qualification machinery.

Native reports do use a resource-fingerprint equality check. However, exporter report seeds copy
that fingerprint from the same sealed input (`merman-export/src/lib.rs:1932`, `:2034`); they do not
independently re-hash a second input. This is internal identity propagation, not a separate content
verification. Keeping bytes, font policy, preparation, and export plan in one immutable owned
object can enforce that association without mandatory hashing. Persistent or external correlation
is a separate reason to request a hash.

### The text namespace has no positive production path

`svg/pipeline/mod.rs:333` explicitly states that no production text writer emits an admitted
prepared-text token. `PreparedTextLabelId` currently provides parsing, not a production ID builder.
The exporter sets expected and verified prepared-label counts to zero and classifies observed
tokens as mismatches (`font_environment.rs:260-265`).

The ordinary family writer still searches the entire SVG for this prefix; native sealing searches
again. A sanitized HTML label can retain an ordinary author-supplied ID such as
`merman-prepared-state-0` and fail this prohibition even though the browser output has no consumer
that would grant it a privileged font identity.

Retiring only one scan would leave the obsolete protocol in ID sanitization and native report
classification. Retire the prefix parser/prohibition, ID rewriting special cases, zero-expected
native accounting, and related evidence fields as one owned change after a complete producer/use
inventory. The active prepared-math protocol is different and must not be swept into this deletion.

### Class proves theme completion even when no theme evidence can be returned

For `ResolvedDiagramTheme=None`, `class/theme/evidence.rs:52-55` selects `CompletionOnly`.
The renderer still builds per-relation expectations and cluster IDs
(`svg/parity/class/render.rs:65-94`), creates cluster/relation/node completion arrays
(`class/theme/terminal.rs:390`, `:404`, `:684`), and verifies completion at emission end.
After successful verification, `class/theme/evidence.rs:70-85` discards the receipt's substantive
result and returns empty theme evidence for this no-theme case.

`ClassNodeVisualPlan` similarly combines real drawing inputs with proof metadata:
`class/node_binding.rs:29-46` copies node IDs into expectations and an index; `:244-283` constructs
and stores terminal paint observations even when typed paints and theme winners are absent.
Source fill/stroke selection, label styling, and default CSS fallback in the same module are real
rendering inputs and must survive. Separate those decisions from per-node proof slots and stored
observations. No-theme completion arrays can disappear rather than become a cheaper parallel
validation system. Explicit-theme behavior must still report actual unsupported/applied outcomes;
that does not automatically justify retaining the same duplicate expectation/observation design.

Flowchart has narrower remaining work. Nodes and edges already condition recording on an actual
theme or source residual (`flowchart/render/node.rs:423`, `edge_path.rs:221`), while cluster and
edge-label paths still enter the recorder unconditionally (`cluster.rs:200`, `edge_label.rs:171`).
Skip empty observations, including their locking/accounting, but preserve source residuals even
without an explicit theme: they affect verification and strict rejection (`family.rs:553-600`).

Do not generalize this finding into every family's default receipt path. Class text receipts are
already optional (`class/theme/text.rs:415-421`); Journey's default task plan has no per-task
expectations (`journey/theme.rs:450-460`); Kanban's default items and task receipt are absent
(`kanban/theme.rs:696-707`, `:811`); and no-theme `FamilyThemeEvidence` is empty (`family.rs:915`).
These observations identify structural work, not independently measured latency contributions.

## Why simply deleting all final checks today would be incorrect

### Producer correctness is not yet complete by construction

Ordinary XML text/attribute utilities already escape content and remove forbidden XML scalars
(`svg/parity/util.rs:364-389`). HTML labels are parsed, sanitized, and serialized with paired tags,
namespaces, and escaped values. Icon bodies are bounded and XML-validated at registry admission,
then sanitized and XML-validated again when rendered under the effective request policy.

However, the CSS character-data writer explicitly leaves forbidden XML scalars for final
validation (`svg/parity/output.rs:52-55`). Removing that final check without assigning this
responsibility to the producer would expose an existing gap. Review dynamic IDs, CSS, links, root
attributes, and raw-fragment insertion similarly. Correct literal tag construction and escaping
should be a small serialization module with strong tests, not a second full SVG compiler.

The generator must keep byte/allocation checks before it grows output. A full scan after the
complete string has been allocated cannot substitute for those guards. Likewise, generated element
counts should be accounted while creating elements or importing already-admitted fragments, not
estimated from Mermaid node counts.

### Imported or custom SVG can amplify downstream work

A small legal SVG can define `b` as two uses of `a`, `c` as two uses of `b`, and so on. Its encoded
size grows linearly while the referenced rendering work grows exponentially. Public custom
postprocessors can inject such a graph. Icon packs can carry it when the effective sanitization
configuration admits `use`; the default allowlist does not admit that tag.

Native export actually consumes the expansion plan before `usvg` to budget duplicated embedded
resources (`merman-export/src/lib.rs:1893-1900`). Retain this preflight before backend tree
materialization, together with image/decode/filter/pixel/depth limits and disabled external file
resolution. Moving the check after the backend has allocated its tree would defeat the purpose.

At the import seam, validate the effective sanitized fragment, its local reference scope, and
resource facts. A registry's raw-body syntax check alone is insufficient, and sanitization depends
on request policy. An icon-ID-only cache or an unconditional trust flag is not a substitute.
Repeated insertion can reuse an admitted representation only with complete policy/lifetime keys
and aggregate accounting. No new cache is justified by this analysis alone.

Local admission is composable only under explicit reference scoping. Current icon ID rewriting
preserves references to names not declared in the fragment (`svg/icon_registry/xml.rs:1284`), so
an imported `href="#host-group"` can acquire a target from the surrounding document. If an admitted
fragment is meant to carry reusable rendering bounds, its rendering references must be closed
within the admitted scope, and IDs and references must be rewritten together on insertion. If
host-dependent references remain supported, their cost cannot be certified from that fragment
alone; the consuming operation owns that decision.

### Browser publication remains a different consumer

Ordinary XML/resource observation does not reject every active element and is not browser DOM
admission. Current parity custom output can contain valid XML with active content; the full native
compatibility path and Web publication policy provide different checks.

Playground already calls `projectNavigableInlineSvg` after `renderSvg`; it uses the Web package's
DOM admission before mounting. Preserve that independent seam and its actual-document/base-URI
checks. The current DOM policy checks bytes, tags, attributes, URLs, and embedded images, but admits
`use`, `filter`, and `mask` without an expanded-element or maximum-depth budget
(`platforms/web/src/svg-safety-policy.ts:248`). It therefore does not already replace downstream
work accounting. Conversely, the current core reference analysis accepts its original budget
result when reference resolution reports an invalid graph (`final_validation.rs:819`); it is not
a complete browser-expansion proof either.

Define the actual publication guarantee and migrate first-party dependencies before removing the
core fallback. A constrained imported-fragment vocabulary and closed reference scope may be enough;
an explicit browser-publication operation can own any stronger necessary bound. Do not build a
browser SVG interpreter merely to preserve a guarantee the existing implementation does not
provide. Arbitrary transformed output needs the checks required by its actual consumer.

### Strict must still describe the actual chosen artifact

The original standalone-sealing fix addressed a real defect: RequirePortable must not silently
replace parity/readable output with different resvg-safe bytes and certify those instead.
Preserve the user's selected pipeline. When strict assurance is requested, validate that actual
output or return a precise rejection. A useful strict operation does not imply that every
BestEffort string operation needs the same identity/evidence machinery.

## Proposed module seams

Use the existing separation between SVG output, document/native preparation, and browser mounting.
Do not add a public `fast`/`unsafe` switch or a general proof-request DSL.

```text
Source + compiled theme
    -> resolve style + prepare layout
    -> bounded, correctly escaped SVG generation
         -> ordinary SVG / requested lightweight application report
         -> opaque transformation: discard any prior checked capability
              -> validate final opaque output when its declared contract requires it
         -> native/RequirePortable: consumer-specific preflight + immutable prepared artifact
              -> PNG/JPEG/PDF conversion
         -> Web publication: DOM/host-policy admission + mount

Qualification / explicit artifact identity
    -> observe completed output and requested reports outside the ordinary hot path
```

Keep theme recipe identities computed at compilation when they are useful for compiled-theme reuse.
Separate theme application state from consumer portability state. A report describing unsupported
styling need not contain an artifact hash chain or claim portability. A no-theme/default operation
should use a cheap empty application state rather than allocating per-node theme proof tables.

For serialization, retain one shared bounded sink and clear XML text/attribute operations. Use
private ownership to distinguish generated/admitted data from opaque byte mutations. Do not permit
callers to manufacture trust with a boolean. Do not build a complete SVG AST, full proof-event
stream, generic validation framework, or second CSS engine merely to preserve unnecessary default
certification.

Three approaches are materially different:

| Approach | Benefit | Remaining cost/design issue | Recommendation |
| --- | --- | --- | --- |
| Optimize the same mandatory scans, checkpoints, and hashes | Can preserve the present contract | Still charges every plain SVG operation for broad output observation and qualification machinery | Useful only for checks that remain necessary at a real boundary |
| Make the current combined certification module lazy | Can avoid unused results | Alone, preserves confused ownership; bytes-only bindings and incidental runtime proof tables may still force the expensive path | A possible migration step, not the final abstraction |
| Separate production, opaque admission, consumer preparation, and explicit identity | Ordinary generation pays for its own work; each stronger operation pays for its requested guarantee | Requires explicit error/resource/API migration and closing producer gaps | Preferred design; delete obsolete protocols instead of introducing parallel fast/legacy paths |

## Contract changes to make explicit

1. `max_svg_elements` currently includes downstream reference expansion and conservative effect
   charging, despite its broad element-count description. Separate emitted-generation work from
   expanded consumer work, or preserve the old interpretation through a deliberate transition.
   Changing its meaning silently is unacceptable.
2. Ordinary generation should guarantee correct escaped SVG from admitted producer inputs, not
   automatic browser safety or universal native portability. Document what happens after a caller's
   arbitrary postprocessor and where final opaque-output validation occurs.
3. Public experimental digest/receipt methods need a concrete migration or an explicit identity
   operation. Do not silently return zero digests, fake certificates, or promote Unverified.
4. Retiring the text-ID protocol intentionally changes reserved-prefix errors and native mismatch
   reporting. Update the contract/tests together, retaining actual font and math checks.
5. Metadata demand must reach the producer. A data-only convenience API should not create, serialize,
   decode, and discard the richer result as its normal implementation.
6. Cancellation should guarantee bounded work between observations and checks around expensive
   callbacks, not a permanent promise about every formatter fragment. A batching change still needs
   boundary, deadline, sticky-error, and resource tests.

## Refactor order and evidence

1. Freeze the intended contracts for ordinary SVG, opaque SVG, strict/native, DOM publication, and
   identity/qualification. Inventory first-party consumers before changing the shared defaults.
2. Retire the inactive prepared-text protocol end to end; preserve live math and actual font checks.
   Remove no-theme Class completion proof tables while retaining its actual source-style bindings.
3. Decouple application diagnostics and explicit identities from ordinary SVG output. Remove the
   binding data-only build/serialize/discard path.
4. Close producer escaping/counting gaps and imported-fragment admission gaps. Reuse existing
   serializers and parsers at their real seams.
5. Remove default generated-output XML/ref readback and mandatory hash sealing only when the new
   producer and consumer contracts are enforced. Keep explicit native/opaque validation.
6. Simplify remaining empty family observations and cancellation granularity where they exist to
   service the old certification path. Preserve source residuals that still drive real decisions.
   Delete obsolete types/tests rather than adding a parallel legacy route.

Test the surviving requirements rather than preserving every old incidental error:
Unicode/control characters, XML delimiters and namespace handling; source/style precedence;
ordinary element/byte ceilings before allocation; malformed opaque output; cyclic/exponential
references before native consumption; actual chosen-output strict rejection; no dropped math or
font guarantees; no false Applied/Portable result; bounded cancellation; matching first-party
binding behavior. Browser tests must enforce the publication contract actually selected for this
redesign. An active-content/network policy does not by itself require a new general reference-DAG
proof. Keep qualification/certification tests on their explicit operation.

Benchmark ordinary SVG and strict/native operations in separate lanes under the revised contracts.
Retain exact SVG identities and capability labels. Measure default/no-theme and styled scenes across
families, plus malformed/hostile controls. Structural counters should show that ordinary generated
SVG performs zero general XML readbacks of the completed generated document, zero retired-prefix
scans, and zero unrequested full-output hashes. This does not prohibit XML parsing at a real
imported-fragment boundary. Performance and semantic evidence must be fresh after the redesign;
the old diagnosis cannot prove the final speedup or an unchanged error contract.

## External reference and historical context

- [Typst's SVG exporter](https://github.com/typst/typst/blob/main/crates/typst-svg/src/lib.rs) constructs
  output with an XML writer and completes it with `end_document`; its ordinary export function does
  not reparse the completed SVG. This is a relevant producer design precedent, not proof that every
  Merman fragment is already safe to trust.
- [usvg's parser](https://github.com/linebender/resvg/blob/main/crates/usvg/src/parser/mod.rs) accepts
  SVG input and constructs a renderer tree, with an explicit element-limit error. This reinforces
  the distinction between serializing owned scene data and admitting SVG for conversion.
- [SVG 2 use semantics](https://www.w3.org/TR/SVG2/struct.html#UseElement) specify generation of a
  reused-graphics shadow tree. Local references therefore have real downstream expansion behavior;
  syntax and byte length do not by themselves bound that work.
- Local [ADR 0063](../adr/0063-extensible-svg-output-pipeline.md) originally separates parity/readable
  output from explicit terminal resvg sealing. [ADR 0078](../adr/0078-headless-svg-security-capability-boundaries.md)
  separates source sanitization, native compatibility, and browser DOM admission. These remain useful
  design evidence; their existence does not prevent revising an interface where current consumers
  demonstrate a better separation.

The external source pages were consulted on 2026-10-16. This document establishes an architectural
recommendation; it does not claim that the proposed removals are already implemented or measured.
