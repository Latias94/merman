# Semantic Shortcut Review

Use this review before implementing any fast path, cache, prepared artifact, parser shortcut,
sanitizer bypass, or family-specific optimization.

## Contents

- Proof obligation
- Acceptable and rejected designs
- Adversarial matrix
- Differential testing
- Prepared artifact and cache review
- Final acceptance question

## Proof Obligation

A shortcut is correct only when all five statements are true:

1. **Ownership:** the layer deciding the shortcut owns the semantics being skipped.
2. **Exact domain:** the accepted input set is defined positively, not as a blacklist of known
   syntax.
3. **Equivalent result:** output, errors, policy, resource accounting, and observable side effects
   match the reference path.
4. **Complete context:** the decision includes every config value, feature, environment input,
   renderer, measurer, and preprocessing stage that can affect the result.
5. **Total fallback:** every unproven case uses the full reference path.

If any statement is argued with "normally", "currently", "for this fixture", or "should not", the
shortcut is heuristic and must not ship.

## Acceptable Designs

- Define an exact syntax-free sublanguage inside the interpreter that owns its projection.
- Let a sanitizer or policy module prove that an input is already a fixed point of the complete
  effective policy.
- Prepare a semantic artifact once and pass it to layout and rendering with explicit private
  ownership.
- Cache with an exact key covering source, config, features, policy, environment, and measurer;
  bound the cache and define its operation or session lifetime.
- Replace an algorithm or data structure while retaining source-backed semantics and differential
  output tests.
- Remove duplicated serialization, allocation, or traversal whose result is already owned and
  immutable.

## Rejected Patterns

- Character or punctuation blacklists for deciding that Markdown, HTML, entities, math, or icons
  are absent.
- Family allow-lists used to skip shared correctness or resource-policy work.
- Caller-side assumptions that sanitization, escaping, URL validation, or entity decoding is
  unnecessary.
- Size thresholds or magic constants that change semantics only to make one fixture faster.
- Caches missing configuration, policy, font/measurer, feature, or environment inputs.
- Unbounded process-global caches for request-scoped work.
- Dropping editor facts, diagnostics, source maps, limits, cancellation, or typed errors without a
  separately defined public operation.
- Comparing preprocessed text with final output while ignoring placeholders or later DOM decoding.

## Adversarial Matrix

Construct counterexamples from every relevant row before coding:

| Boundary | Cases |
| --- | --- |
| Text | empty, leading/trailing/repeated whitespace, CRLF, multiline, ASCII punctuation |
| Unicode | non-ASCII labels, combining marks, emoji, RTL, normalization differences |
| Entities | `&`, named entities, decimal/hex references, malformed entities, Mermaid placeholders such as `#quot;` |
| Markdown | emphasis, code, links, headings, lists, tables, escaped delimiters, malformed openers |
| HTML/XML | raw and malformed tags, attributes, comments, CDATA-like text, `<br>`, images |
| Extensions | FontAwesome/icon syntax, math delimiters, image labels, Markdown auto-wrap |
| Policy | strict, antiscript, sandbox, loose, unknown levels, HTML-label toggles, allow/add/forbid tag and attribute sets |
| Environment | custom text measurer, fonts, locale, renderer/layout engine, viewport or wrap width |
| Runtime | source/model/layout/output budgets, timeout, cancellation, concurrency, deterministic mode |
| Errors | same error kind, span, details, partial output, and fallback behavior |

Add family-local fixtures and at least one unaffected control family when modifying shared code.

## Differential Test

Keep or expose the full reference path in tests. For a corpus containing accepted shortcut inputs
and adversarial fallbacks:

1. run both paths with identical source, config, environment, and capabilities;
2. compare semantic models, layouts, canonical DOM or SVG, errors, and resource accounting as
   applicable;
3. preserve array order and canonicalize only explicitly unordered map-like JSON;
4. assert that fallback cases actually execute the reference path when observable;
5. run the production entry point end-to-end so preprocessing placeholders and policy stages are
   included.

Snapshot equality alone is insufficient when errors, security policy, cancellation, or resource
accounting can differ.

## Prepared Artifact and Cache Review

For reused work, document:

- owner and lifetime;
- construction point and all inputs;
- key and invalidation rules;
- maximum retained entries or bytes;
- thread and cancellation behavior;
- which public projections may observe it;
- why storing it does not change serialization or API compatibility.

Prefer an operation-scoped private prepared artifact over a generic payload bag or global cache.
If layout and SVG consume the same transformed label, store that owned result once instead of
maintaining two classifiers.

## Acceptance Question

Ask: "Could a future valid input or configuration make the skipped code observable?" If yes, either
move the proof to the semantic owner and cover the case exactly, or keep the full path.
