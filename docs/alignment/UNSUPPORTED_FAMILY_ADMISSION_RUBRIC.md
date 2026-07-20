# Mermaid 11.16 Family Admission Record

Status: Completed
Baseline: pinned Mermaid `11.16.0`
Last updated: 2026-07-17

The filename is retained for stable links. The Mermaid 11.16 family-admission work is complete:
the executable inventory contains 35 primary SVG families plus the external-renderer family
`zenuml`. ZenUML owns a complete local parser/semantic/editor/headless path but remains a distinct
external comparison lane because Mermaid registers its browser renderer from a companion package.
There is no family-level intermediate admission queue.

## Completed Admission Contract

Each primary family entered the matrix only after all of these gates were satisfied:

1. Pinned-source authority: detector headers and behavior come from the commit locked by
   `tools/upstreams/REPOS.lock.json`.
2. Single semantic ownership: one typed, span-rich family source projects compatibility JSON,
   typed rendering input, and editor facts instead of maintaining parallel parsers.
3. Headless layout: layout and measurement dependencies are explicit and do not execute Mermaid
   in a browser at runtime.
4. Evidence: normalized source-backed fixtures have semantic and layout goldens, pinned upstream
   SVGs, and complete provenance.
5. Executable comparison: every primary family owns a `compare-*-svgs` fact used by
   `compare-all-svgs` and `check-alignment`.
6. Honest residuals: browser text metrics, `foreignObject`, RoughJS, and third-party layout
   differences are documented narrowly; fixture-keyed geometry is not an admission mechanism.

## Mermaid 11.16 Result

| Family group | Admission result | Evidence boundary |
| --- | --- | --- |
| `treeView`, `ishikawa`, `eventmodeling`, `venn` | Primary | Family-owned semantics, typed layout/SVG, pinned baselines, and compare facts |
| `swimlane` | Primary | Reuses Flowchart semantics while owning swimlane layout, routing, styles, and SVG evidence |
| `railroad`, `railroadEbnf`, `railroadAbnf`, `railroadPeg` | Primary | Four grammars share one family model, renderer, provenance corpus, and compare harness |
| `cynefin` | Primary | Source-backed domain geometry with bounded browser text-measurement behavior |
| `wardley` | Primary | Full map semantics/editor facts, typed geometry, ten Cypress fixtures, and parity/root gates |
| `error` | Primary | Suppressed parse failures and direct `error` input converge on one typed layout/SVG family path |
| `zenuml` | External comparison lane | Grammar-derived semantic/editor model, typed local layout/SVG, admitted ZenUML Core behavior source, and strict native-SVG browser plugin evidence |

## Residual Boundary

Primary admission means source-backed semantic and structural convergence, not browser pixel
identity. The remaining cross-family residual classes are:

- browser and platform font selection, `getBBox()` floats, and `getComputedTextLength()`;
- `foreignObject` HTML layout and serialization;
- JavaScript RoughJS versus Rust `roughr` path geometry;
- pinned third-party layout engines whose floating-point behavior is not exactly reproducible.

These residuals stay visible in family coverage documents and comparison modes. Comparator
normalization must remain narrow and non-semantic.

## Future Baselines

When a later Mermaid baseline adds a family, repeat the completed contract above in this order:

- detector, typed semantic source, and editor facts;
- render projection and deterministic layout;
- family SVG serializer;
- source-exact fixtures, semantic/layout goldens, and schema-v2 SVG provenance;
- a dedicated compare fact and primary-matrix promotion;
- focused residual documentation backed by upstream source.

Do not add detection-only records to the family admission inventory. Capability discovery belongs
in the core family registry; the admission inventory is the release-quality rendering contract.
