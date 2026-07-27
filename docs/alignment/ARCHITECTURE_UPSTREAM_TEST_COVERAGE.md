# Architecture Upstream Test Coverage

Architecture parity targets Mermaid `11.16.0` at source commit
`7c0cafcf42e76bfaf79d0cbbd12edb986612f014`.

## Evidence Authority

The generated baseline manifest is the authority for fixture admission:

- manifest: `fixtures/upstream-svgs/architecture/_baseline-manifest.json`
- source tag: `mermaid@11.16.0`
- manifest status: `complete: true`
- excluded fixtures: none
- admitted inputs: 185
- imported or source-derived `upstream_*` inputs: 79
- authored `stress_*` inputs: 98
- focused `probe_*` inputs: 8

Every admitted input has:

- one semantic snapshot under `fixtures/architecture/`;
- one typed layout snapshot under `fixtures/architecture/`; and
- one pinned Mermaid SVG baseline under `fixtures/upstream-svgs/architecture/`.

Do not maintain a second exhaustive fixture list in this document. The manifest, fixture directory,
and family compare gate must agree on the exact set.

## Source Categories

The corpus draws from:

- parser and model tests in
  `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/architecture.spec.ts`;
- Mermaid 11.16 database and renderer behavior in `architectureDb.ts` and
  `architectureRenderer.ts`, including row/column alignment hints;
- syntax examples in `repo-ref/mermaid/docs/syntax/architecture.md`;
- browser rendering cases in
  `repo-ref/mermaid/cypress/integration/rendering/architecture/architecture.spec.ts`;
- SVG structure tests in
  `repo-ref/mermaid/packages/mermaid/src/diagrams/architecture/svgDraw.spec.ts`;
- security cases in `repo-ref/mermaid/cypress/integration/other/xss.spec.js`;
- production examples in `repo-ref/mermaid/demos/architecture.html`; and
- authored stress and probe inputs for dense routing, nested groups, junctions, ports, labels,
  icons, XML escaping, and root geometry.

## Admission Contract

Architecture has no filename-based parser-only policy and no excluded baseline entries. All 185
inputs participate in semantic, layout, and SVG evidence.

Some fixture names retain `normalized` from their import history. That suffix is provenance, not a
separate Mermaid 11.12 grammar mode or an exemption from the current parity gate. Historical
11.12.3 parser-only explanations no longer describe this corpus.

## Verification

Run the family evidence gates with:

```bash
cargo run -p xtask -- check-upstream-svgs --diagram architecture
cargo run -p xtask -- compare-architecture-svgs \
  --check-dom \
  --dom-mode parity-root \
  --dom-decimals 3
```

Semantic and layout snapshots are refreshed through the repository snapshot workflow; upstream SVG
baselines remain provenance-attested generated artifacts rather than hand-edited expectations.
