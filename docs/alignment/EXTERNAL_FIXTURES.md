# External Sources and Licensing

## Fixture and Oracle Policy

The primary Mermaid SVG oracle is the [selected reference graph](README.md#machine-authorities).
Imported upstream tests/docs and authored regression inputs must be traceable to their source;
committed baselines bind the selected runtime and exact input/output hashes. A historical collection
version in an input comment does not select the current renderer.

Do not import another renderer's example corpus or expected SVGs as Mermaid acceptance evidence.
Exploratory performance comparisons and debugging inputs from other renderers remain local. An
independently authored reproduction of a user report may become an ordinary fixture only with
reviewed provenance/licensing and the normal Mermaid semantic, layout, and SVG evidence. A different
renderer agreeing with Merman does not replace that evidence.

External behavior lanes must be named explicitly. ZenUML follows the selected ZenUML Core graph
and its [browser probe contract](../../tools/upstreams/ZENUML_BROWSER_ADMISSION_PROBES.json),
separately from the built-in Mermaid SVG matrix. Source-backed algorithm ports, such as Venn/fmin,
retain their own source and license notices. Pinned repositories live in the
[source lock](../../tools/upstreams/REPOS.lock.json); the
[reference bundle](../../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json) owns executable dependency
selection. A workspace's incidental dependency resolution is not a substitute oracle.

## Design References and Fonts

The [theme reference corpus](../../fixtures/themes/README.md) preserves licensed design material
from Modern Mermaid and Excalidraw. Its
[manifest](../../fixtures/themes/manifest.json) binds source revisions, paths, hashes, mechanism
citations, font assets, and declared ranges. Historical theme translations and portability labels
are design references, not an executable support registry, current acceptance gate, or public
preset catalog. Merman does not execute their TypeScript or adopt arbitrary CSS as a theme language.

| Reference | License and attribution owner |
| --- | --- |
| Modern Mermaid design source | [MIT mirror](../../fixtures/themes/licenses/modern-mermaid-MIT.txt) and pinned corpus manifest |
| Excalidraw project source | [MIT mirror](../../fixtures/themes/licenses/excalidraw-MIT.txt), independently of font licenses |
| Excalifont reference WOFF2 | [SIL OFL 1.1](../../fixtures/themes/licenses/OFL-1.1.txt) and [Excalifont notice](../../fixtures/themes/licenses/Excalifont-NOTICE.txt) |
| Xiaolai reference WOFF2 | [SIL OFL 1.1](../../fixtures/themes/licenses/OFL-1.1.txt) and [Xiaolai notice](../../fixtures/themes/licenses/Xiaolai-NOTICE.txt) |

Retain asset, license, and NOTICE bytes, including trailing whitespace, when refreshing a source
snapshot. Review source identities and hashes together; a new snapshot does not establish production
font embedding, font API availability, or artifact qualification. Current claims belong to
[theme coverage](../rendering/diagram-theme-coverage.md),
[preset qualification](../rendering/preset-qualification.md), and their public API/artifact tests.
Application annotation overlays and arbitrary host element styling remain host-owned.

## PR 28 Attribution

Any retained example or preset derived from Ophir LOJKINE's (`@lovasoa`)
[PR 28](https://github.com/Latias94/merman/pull/28) Flowchart contribution must carry this credit:

> Based in part on the `merman-modern` Flowchart work contributed by Ophir LOJKINE (`@lovasoa`) in Merman PR #28.

An attributed visual recipe does not reinstate the removed aggregate profile. Theme selection,
Mermaid look, layout backend, routing policy, and SVG output policy have independent owners; see
[theme layering tests](../../crates/merman/tests/diagram_theme_layering.rs). Contribution history
remains in Git rather than a second migration ledger.

Runtime port/distribution notices are maintained by the
[repository notices](../../THIRD_PARTY_NOTICES.md), package-local notices, and their existing legal
owners. Design-reference licensing does not replace those obligations.
