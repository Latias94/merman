# Theme reference assets and live scenes

This directory preserves licensed design-reference material from the theme architecture
investigation and hosts the fixed scenes consumed by current preset qualification.

## Historical design references

`manifest.json` retains all 24 pinned Modern Mermaid source themes and 18 source-to-semantic
mechanism translations. It binds upstream revisions and paths, SHA-256 hashes, reviewed
mechanisms and value-level facets, Mermaid precedence evidence, and font provenance. The
`evidence/`, `inputs/`, `sources/`, `expectations/`, and `acceptance/` directories preserve
reviewed source facts, reference fixtures, golden records, and historical C6 acceptance records.

These historical records are not an executable support registry, a current acceptance gate,
or a catalog of 24 public Merman presets. Historical portability classifications describe
intended semantics. Current supported and residual capabilities belong to the public theme
support API and the emitted-artifact qualification tests. The private Rust catalog and its
alternate CSS/directive parser and lineage checker have been retired.

`licenses/` contains MIT and SIL OFL license mirrors and font attribution notices. The retained
Excalifont and Xiaolai WOFF2 assets, their hashes, and declared ranges remain reference resources;
they do not establish production embedding support. Preserve asset and license bytes and
attribution when updating the pinned corpus.

See [the capability corpus](../../docs/alignment/MODERN_MERMAID_THEME_CAPABILITY_CORPUS.md)
for the reviewed provenance and architectural boundaries.

## Live representative scenes

`public-cyberpunk/` contains byte-preserved Flowchart, Sequence, and XY Chart Mermaid inputs
plus their original source/provenance README. They are shared by renderer/facade tests,
native qualification and benchmarks, browser and language smoke tests, and debug tooling.
Selecting a preset or successfully parsing a scene does not qualify its visual output.

See [preset qualification](../../docs/rendering/preset-qualification.md) and
[theme coverage](../../docs/rendering/diagram-theme-coverage.md) for current product claims.
