# Mermaid 12 UTF-16 JSON and title measurement follow-up

## Scope

Continue from `3dd75965702b4bed17f043db54a4a87cbee40c18` on the existing Mermaid 12
alignment branch. The selected graph remains Mermaid 12.0.0 at
`98a0945418c76238f15df2afaddbba4272656c3b`; no dependencies, family features, runtime
providers, or reference identities change. Preserve unrelated working-tree files.
Local verification evidence belongs under `target/branch-review-mermaid12-round4/`.

## Lossless Usecase JSON strings

JavaScript strings can contain unpaired UTF-16 surrogate code units. Rust strings
and `serde_json::Value` object keys cannot. Rejecting `json Data@{"key":"\ud800"}`
therefore rejected valid Mermaid input. Replacing surrogates while constructing the
semantic model would also merge distinct keys such as `\ud800`, `\udc00`, and U+FFFD.

The existing JSON syntax validator remains authoritative. Its `deserialize_bytes`
string visitor supports WTF-8, including unpaired surrogates. A small family-local
adapter converts those bytes into UTF-16 code units; no second JSON grammar or
additional dependency is introduced.

Ordinary JSON nodes retain their current model and serialized shape. When a node's
JSON source contains an unpaired surrogate, it carries `stringEncoding: "json-utf16"`.
In that mode **every object key and string value** in its `value` tree is a complete
canonical JSON string literal, including the outer quotes. Each code unit uses a
lowercase `\uXXXX` escape. Equivalent literal/escaped strings and paired surrogates
have the same identity. Numbers, booleans, null, arrays, and objects retain their
usual roles. The tag makes this representation explicit rather than pretending the
stored literals are display strings.

`propertyOrder` uses those encoded keys. Its JSON pointers, and pointers in
`nonFiniteNumbers`, address the encoded tree. Duplicate keys retain their first
position and replace their value; overwritten subtrees remove obsolete metadata.
Never use a lossy display key to look up a child or metadata entry.

Both typed serialization and the compatibility projection retain the encoding tag.
The field is optional and omitted for ordinary nodes; old typed JSON still decodes.
Rust struct literals of the branch's new `UsecaseJsonNode` type need
`string_encoding: None` when constructing ordinary JSON values.

Rust consumers can use:

- `string_code_units` for lossless JavaScript string identity;
- `display_string` for UTF-8 presentation, with one replacement character per
  unpaired surrogate;
- `to_javascript_json` for a complete ordinary JSON text that JavaScript can parse
  without losing code units. Non-finite numbers serialize as null, as JSON.stringify does.

JavaScript consumers of the compatibility projection should check the tag. For an
encoded node, recursively apply `JSON.parse` to each string value and object key.
Use `Object.fromEntries` or a null-prototype object when rebuilding objects so that
an authored `__proto__` key cannot mutate a prototype. Keep identity and display
conversion separate. Ordinary nodes require no conversion.

Mermaid's DOM and XML serialization retain lone surrogates internally. Mermaid CLI's
`TextEncoder().encode(svgXML)` is the UTF-8 replacement boundary. Merman follows that
boundary for its UTF-8 SVG output while keeping semantic code units lossless. Label
measurement, sanitization, and math-capability detection use decoded display strings;
they do not show literal encoding quotes or interpret encoded keys as identities.

## Title measurement boundary

Title fixture `flowchart/stress_flowchart_title_padding_subgraph_029` already uses
Merman's public `TitleBBoxX` callback correctly. The Web SDK already implements that
operation with real SVG `getBBox`, and Playground defaults to browser measurement.
The missing evidence was a regression through the assembled public Web package and
fresh WASM, rather than only a Rust test host with captured measurements.

The browser regression covers the configured font and a monospace override, tests
the actual mounted title against the emitted viewport, and uses the unchanged paint
oracle for the reference case. A host that declines measurement must still produce
the same output as the deterministic provider. The test prepares fonts before the
authoritative synchronous render; it does not claim that the synchronous SDK waits
for asynchronously loaded webfonts.

ADR-0086 deliberately keeps the deterministic provider font-agnostic. It cannot infer
arbitrary fonts installed on the eventual display host. The canonical deterministic
029 artifact and its browser-root gate are not changed or relabeled by this fix.
No extra padding, font coefficient, fixture-specific table, or relaxed comparison
is added. Callers requiring bounds for a particular display must use that display's
measurement callback after its fonts are ready.

## Validation

Completed native checks:

- 33 focused Usecase parser and public contract tests passed with the minimal
  `diagram-usecase` feature set.
- 25 renderer tests passed, including both Dagre and ELK, HTML and SVG labels,
  distinct surrogate keys, non-finite values, and decoded math admission.
- 54 all-feature core tests passed, including the semantic snapshot corpus,
  editor facts, nesting limits, and adjacent surrogate code units.
- Clippy passed with warnings denied for all targets and features of
  `merman-core`, `merman-render`, and `merman-wasm`.
- Workspace formatting, whitespace checks, Web TypeScript build, and Web export
  contract checks passed.

The three test counts overlap and must not be summed as unique test coverage.
The independent correctness review found no remaining confirmed defect after the
review fixes. Local source evidence is recorded in
`target/branch-review-mermaid12-round4/usecase-utf16-boundary-evidence.json`:
locked serde_json 1.0.151 supplies lossless byte decoding, and Mermaid CLI 11.17.0
`src/index.js:611` applies the TextEncoder boundary. The selected Mermaid graph
and its receipt are unchanged.

The full WASM build completed successfully, followed by standard package assembly
and input freshness verification (transaction `f3c68b881941`). All three public Web
browser regressions passed in Chromium:

- configured-font title measurement includes the mounted title in the root bounds
  and passes the unchanged root-containment oracle;
- the same public callback handles a monospace override;
- semantic JSON and SVG APIs retain four distinct surrogate/replacement/literal
  keys and render valid UTF-8 XML.

The default preview port 4178 was denied by the local OS before any test executed.
The supported `PLAYWRIGHT_PORT=18789` override completed all three tests in 14.9s.
The test verifies current WASM inputs and assembled artifact provenance before
loading the public package. Evidence is in `web-package-assemble.log`,
`web-wasm-freshness.log`, and `web-browser-regressions-port18789.log` under the local
evidence directory. No full browser matrix rerun is claimed: the canonical
font-agnostic provider and fixture 029 artifact are unchanged.

## Commit disposition

`65e923ed6` commits the atomic UTF-16 model, renderer, and native regression changes.
The public browser regression and this report are a separate follow-up commit.
No changes have been pushed. Unrelated working-tree files remain untouched.
