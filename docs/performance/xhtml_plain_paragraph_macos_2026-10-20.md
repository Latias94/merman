# Literal XHTML paragraph ownership — macOS, October 20, 2026

The candidate removes wrapper allocation and XML DOM construction when a label is exactly one
literal paragraph with printable ASCII text. The no-theme Kanban primary drops from 615 to 531
allocations and saves 13,849 allocated bytes per public render, with unchanged peak and retained
growth. This is allocation and work removal; no latency improvement or release convergence is
claimed.

## Ownership change

`mermaid_xhtml_label_plain_text` previously wrapped every generated fragment in another element,
parsed a full document and walked its children to recognize one plain paragraph. It now recognizes
only an exact `<p>INNER</p>` where INNER contains only bytes in ASCII 0x20 through 0x7e,
excluding `<`, `&` and `]`. This proves the structure and literal text without entity decoding or newline normalization;
excluding `]` also prevents an illegal `]]>` sequence. Empty paragraphs retain `Some("")`.

All unproven inputs retain the original parser unchanged, including entities, Unicode, CR/LF/tab,
attributes, namespaces, nested markup, comments, CDATA and malformed XML. Quotes and ordinary `>`
are valid literal text. The returned owned string, all three callers, native measurement precision
and subsequent HTML fallback remain unchanged. The parser has no operation callbacks, work-meter
or cancellation hooks in this call. The five added assertions extend the existing structural
paragraph test with quotes/greater-than text, empty content, CR normalization, forbidden `]]>`
and comment rejection.

The preceding system sample located 192 of 2871 current public Kanban Renderer samples in plain
paragraph extraction, including 147 XML-parser samples. Both releases already have this work;
v0.9.0 has 262 of 3141 Renderer samples there. These are inclusive sampling counts, not a causal
explanation or a conversion to saved microseconds. The separate None-theme audit confirms that
Kanban already skips typed visible-style fact collection. CSS work also moved between preparation
and emission, so stage differences are not summed.

## Public operation evidence

The frozen baseline is `07b7dcbbf3359996fd96af9b8218058ea30fb0ad`, a documentation-only descendant
of the latest measured release head. Both sides retain default and all public features, matching
release optimization, rich public render results and the original deterministic custom-measurer
controls. One warmup and five measured allocation operations are kept; builds, probes and tests
run serially. Theme compilation stays outside the per-render measurement.

All 645 saved SVG/report controls and ordered measurement traces match; all 26 public error
controls match. Of 505 jointly stable controls, 33 improve and 472 retain all four metrics, with
no increases. The other 140 variable controls remain inconclusive without resampling. Both
preselected None Kanban controls are stable and save 84 allocations and 13,849 bytes. Their peak
growth remains 45,822 bytes and retained growth remains 30,319 bytes.

All 418 catalog cells across 35 logical families and 38 fixture aliases, ten presets plus None,
are stable and exact within the original contract: warmup result metadata plus all five saved
sample SVG/report bytes and metadata. Warmup SVG bytes are not captured. Sixty-six cells improve
and 352 retain every metric; none regress. Each affected scene improves in all eleven modes:

| Scene | Fewer allocations | Fewer allocated bytes |
| --- | ---: | ---: |
| `flowchart_medium` | 1116 | 183,771 |
| `class_medium` | 528 | 86,944 |
| `er_medium` | 396 | 65,289 |
| `kanban_medium` | 84 | 13,849 |
| `swimlane_medium` | 204 | 33,593 |
| `agentflow_basic` | 60 | 9,883 |

Catalog retained growth is unchanged everywhere. Only the None Flowchart, Swimlane and Agentflow
cells lower peak growth, by 1696, 1653 and 1702 bytes respectively; every other catalog peak is
unchanged. The None Kanban catalog primary is 615 to 531 allocations and 117,043 to 103,194
allocated bytes, with peak 46,608 and retained 31,087 bytes unchanged. Editor Light Kanban is
3745 to 3661 allocations and 769,127 to 755,278 bytes, with peak 311,625 and retained 31,225
bytes unchanged. These equal savings do not remove compiled-theme initialization costs.

## Validation and execution record

All 13 registered steps pass. All 131 focused tests and 5613 full release tests pass (seven full
tests skipped) using the same `merman` and `merman-render` all-feature graph. Strict release
Clippy, all 37 DOM entries in three modes, the optimized full WASM build including `wasm-opt`,
formatting and diff/path checks pass. Independent source and raw-evidence reviews pass.
Browser-text/parser diagnostic residuals retain their original scopes. No new WASM runtime smoke
or latency experiment is claimed.

The original baseline driver stopped before catalog sampling because a preparation source
snapshot occupied the reserved output directory. The snapshot and failure record were preserved.
An execution amendment moved only that snapshot and completed the previously unexecuted catalog
and error stages; successful allocation/trace samples were not repeated. The original patch
needed formatting of two new test assertions, recorded separately; the production guard did not
change. All four frozen baseline probe identities remain available.

## Evidence and next work

Ignored artifacts are under `target/bench/experiments/xhtml-plain-paragraph-20261020/`, including
registration, original and formatted patches, exact source/probe identities, all raw controls,
five catalog samples, the baseline amendment and independent reviews.

- Registration SHA-256: `ba54e546e47b4c3acc7218892c3685e87e55d40c007674c9cbaf902c1dfc9fcb`.
- Production source SHA-256: `e24bf1e559b5156d7464a7490ca7bca1fafb418e4c2199686b9c8b95de1e875a`.
- Formatted patch SHA-256: `afe695a42c5b42996433a91c8d3e1586d0236990d4af97fea5b629ae5b9bc52a`.

The [latest complete release confirmation](theme_v090_post_ownership_confirmation_macos_2026-10-20.md)
remains 12 non-regressions, three regressions and four inconclusive scenes at its own frozen head.
This candidate does not update that scoreboard. Keep bounded ZenUML CSS string assembly and
Packet sink-dispatch hypotheses separate until independently registered and measured. The larger
compiled-theme initialization owner remains a source-backed proposal, with per-operation journal,
host-override and deferred-error contracts preserved. The all-family goal remains active.
