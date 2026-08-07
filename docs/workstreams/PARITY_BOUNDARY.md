# Mermaid Parity Boundary

This document defines how far merman should go when reproducing Mermaid output. The goal is to
keep merman useful as a Rust library: deterministic, browser-free, fast, and maintainable. Mermaid
CLI pixel parity is a diagnostic tool, not the product boundary.

Pinned upstream baseline: Mermaid `@11.16.1`.

## Product Boundary

merman should optimize for:

- semantic compatibility: the same diagram input produces the same graph, labels, links, security
  decisions, and user-visible meaning;
- stable visual compatibility: common diagrams should look close to Mermaid and keep predictable
  layout;
- deterministic Rust implementation: no runtime browser dependency and no fixture-answer logic in
  core rendering code;
- explicit parity debt: browser-only residuals stay visible in verification reports and narrow
  accepted-residual policy rather than changing production output.

merman should not optimize for:

- reproducing every Chromium sub-pixel measurement when doing so requires broad hand-written
  special cases;
- hiding parser, semantic-model, layout, sanitizer, or DOM-order bugs behind overrides;
- adding per-fixture or per-icon literals because a narrow SVG diff becomes green.

## Decision Matrix

| Mismatch source | Default action | Examples |
| --- | --- | --- |
| Mermaid semantics or layout rules | Required derivation | parser fields, config precedence, DOM order, rank/spacing, edge routing, label wrapper structure |
| Shared deterministic browser-like behavior | Required derivation when the rule is explainable | HTML line boxes, whitespace around inline elements, CSS inheritance rules, stable quantization |
| Browser/font measurement fact | General generated fact or host measurement | glyph overhang, kerning, fallback font width, `getBBox()`, `getComputedTextLength()` |
| Upstream export-root difference | Required derivation or accepted residual | root `viewBox`, root `max-width`, emitted-bounds drift after DOM insertion |
| Strict serialization or tiny browser lattice noise | Accepted drift when narrowly documented | 1/64px root drift, attribute serialization order already canonicalized elsewhere |
| Fixture-answer literal without source | Reject | per-icon widths from root drift, one-off label width match arms, broad renderer-local constants |

## Required Derivation

These differences must be fixed in parser, model, layout, renderer, or generalized measurement
code. Production overrides are not an acceptable substitute.

- Parser and semantic model facts: nodes, edges, labels, subgraphs, diagram-specific fields,
  comments, IDs, directives, and config precedence.
- Security and escaping: sanitizer behavior, entity decode/encode order, link wrapping, Markdown
  tokenization, and DOM namespace placement.
- DOM structure and ordering: emitted elements, CSS selector paths, marker/link IDs, label wrapper
  shape, and diagram-specific grouping order.
- Layout rules: rank/order, spacing, padding, margins, shape dimensions, cluster bounds, edge
  routing, clipping, label attachment, and config-driven wrapping width.
- Shared deterministic measurements when the rule is general enough: line-height, whitespace
  preservation, known CSS inheritance rules, quantization, and text segmentation behavior that
  applies across multiple fixtures or diagram families.

Rule of thumb: if a human can explain the behavior as Mermaid semantics or a stable layout rule,
derive it. Do not encode it as a fixture value.

## Generated Data Only

Some upstream facts are browser/font/export facts rather than Mermaid semantics. They may be
represented as generated data when all of the following are true:

- the source is repeatable: an upstream SVG baseline, a browser probe, a vendored font/CSS file, or
  a pinned Mermaid CLI export;
- the generator or extraction path is documented;
- the data is narrow and version-scoped to the Mermaid baseline;
- the key generalizes to unseen labels and contains no fixture id or complete label string;
- fixture corpora are independent validation inputs rather than training answers.

Valid generated-data categories include:

- browser text metrics: `getBBox()`, `getComputedTextLength()`, `getBoundingClientRect()`, glyph
  overhang, kerning, fallback fonts, and SVG/HTML text export quirks;
- generated font metrics from upstream font/CSS assets, provided they are generated or probed and
  not hand-entered because a fixture failed.

Generated data must not encode:

- missing typed model data;
- layout, routing, DOM-order, sanitizer, escaping, or Markdown bugs;
- broad fallbacks that change unrelated diagrams;
- hand-curated glyph or icon tables derived from root drift;
- fixture ids, complete source strings, or complete label strings.

## Exact Verification Catalogs

An admission catalog may use fixture id, semantic key, and exact text only as a verification index
when every entry is also bound to the baseline version, source commit, comparator revision, input
hash, signed upstream artifact hash, and complete upstream/local signature. Such a catalog must
reject stale, missing, changed, or newly observed entries and must never be read by production
rendering. This narrow exception is what makes a reviewed browser residual auditable; it does not
permit fixture-keyed layout behavior or broad comparator tolerance.

Root viewport and semantic edge-label catalogs remain separate because they protect different
contracts. A root residual cannot suppress a label identity, geometry, path, or presentation
failure.

## Accepted Drift

Some differences should be accepted instead of modeled, especially when the implementation cost
would harm the Rust library boundary.

Accept drift when:

- the difference is strict-XML serialization noise with no meaningful visual or semantic impact;
- the difference is small browser float/lattice drift and is isolated by a narrow verification
  residual;
- exact parity would require per-browser or per-font behavior that is not available without a
  browser engine;
- exact parity would add broad special cases that are harder to explain than the observed
  difference;
- the fixture remains useful as a guard but the core model should stay clean.

Accepted drift must still be visible in a parity report, narrow accepted-residual policy, or a
changelog/TODO note explaining why exact modeling is intentionally out of scope.

## FontAwesome And Icon Labels

FontAwesome is the canonical boundary example.

Allowed:

- model Mermaid icon-token substitution and emitted `<i class="fa ..."></i>` structure;
- preserve HTML line boxes and whitespace around inline icons;
- use a clean nominal inline width when that keeps the renderer predictable;
- treat unregistered custom-pack examples as empty inline elements when upstream emits no usable
  icon font;
- accept a bounded root residual when exact width depends on an unavailable FontAwesome font.

Not allowed:

- adding a hand-written table of icon names and widths because a set of fixtures drifted;
- deriving per-icon widths from root viewport deltas;
- changing plain text labels because they happen to share normalized text with icon labels.

Potential future path:

- If exact FontAwesome layout becomes worth the maintenance cost, add a generator that reads the
  pinned Mermaid CLI bundled FontAwesome CSS/font files and emits a versioned metrics table. Review
  it as generated font data, not as a renderer-local match arm.

## Review Checklist

Before merging a parity change, answer these questions in the PR, commit note, or workstream log:

1. Is the mismatch semantic/layout behavior, browser/font measurement, export-root drift, or
   serialization noise?
2. If it is semantic/layout behavior, where is the typed derivation?
3. If it is generated data, what repeatable upstream source produced it?
4. If it is accepted drift, where is the guard or documented retention note?
5. Does generated measurement data generalize to unseen strings and use fixtures only for
   validation?
6. Did the relevant focused and global parity commands pass without production pins?

## Command Evidence

Use these commands according to blast radius:

```sh
cargo fmt --all --check
cargo clippy -p merman-render --all-targets -- -D warnings
cargo nextest run -p merman-render
cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```
