# ADR-0086: Deterministic Text Measurement Without Vendored Font Tables

## Status

Accepted.

## Date

2026-08-27

## Context

Merman must choose label geometry before a browser, native preview, or Typst document paints the
result. Earlier releases embedded browser-probed font tables captured with Headless Chrome 131 for
a bounded set of font stacks, sizes, glyphs, kerning pairs, DOM shapes, and rounding behavior. The
tables improved agreement for the measured domain, but they did not establish a general font
measurement contract:

- an unmeasured font, fallback chain, variable-font axis, script, browser version, or platform still
  used an approximation;
- matching the table did not prove that the eventual display surface selected the same fonts or
  applied the same shaping and rounding;
- the generated profiles, codecs, probes, and tests materially increased every SVG-capable native
  and WebAssembly product; and
- the additional implementation surface made a bounded heuristic look more authoritative than it
  was.

The useful abstraction is the operation-aware text-measurement seam. A host that owns the final
font stack can answer that seam faithfully. A standalone headless artifact cannot infer the final
surface from a small embedded browser corpus.

## Decision

1. Remove the generated Chrome 131 font tables, their runtime codec and lookup implementation, and
   the font-profile generation command from production and release tooling.
2. Keep the operation-aware `TextMeasurer` abstraction and all 19 protocol-v1 operations. Browser,
   Android, Apple, Flutter, and other display hosts may continue to provide a synchronous callback.
3. A successful, valid host result is authoritative for that request. An unsupported, invalid, or
   failed host result uses Merman's built-in deterministic fallback for that request.
4. The built-in fallback is dependency-light and font-agnostic. It applies stable character-class,
   Unicode display-width, spacing, line-height, and width-based wrapping rules. It does not claim to
   measure a named font or reproduce a particular browser's shaping, kerning, baseline, or bbox
   lattice.
5. `deterministic` is the only built-in provider and the only accepted built-in options value. The
   former `vendored` and `parity` provider names and public constructors are removed rather than
   retained as aliases.
6. Products that expose host services advertise `host-callback` and `deterministic`. The Typst
   plugin has no synchronous font-measurement import, so it advertises and uses `deterministic`
   only. Typst typography options still express SVG/CSS presentation intent; they do not imply
   measurement of a Typst font asset.
7. Browser font selection, shaping, `getBBox()`/`getComputedTextLength()` floats, baseline behavior,
   and final rasterization remain bounded residuals. Verification may measure and attribute them,
   but production code must not hide them in font- or fixture-specific lookup tables.

## Consequences

- SVG-capable Rust, CLI, native binding, Web, and Typst products share a smaller production data
  closure and one honest built-in fallback contract.
- Host callbacks remain the primary route when geometry must match the surface that displays the
  SVG; deterministic measurement remains suitable for CI, servers, static documentation, and
  offline rendering.
- Outputs can move where the removed tables previously supplied browser-specific advances,
  kerning, vertical metrics, or quantization. Such movement is an intentional breaking change and
  must be evaluated as semantic/layout correctness plus documented browser residuals, not repaired
  by restoring narrow font tables.
- The text-measurement protocol stays stable even though one provider disappears. Bindings keep
  per-request fallback, validation, lifecycle, and provenance behavior.

## Rejected Alternatives

### Keep a smaller subset of the browser tables

Rejected. A smaller font list has the same authority problem and still charges every product for a
corpus that cannot describe the user's final display stack.

### Bundle fonts and a complete shaping engine

Rejected as the default product boundary. It would add a large dependency and license closure and
still would not reproduce arbitrary browser CSS, platform fallback, or user-installed fonts. A
future opt-in product capability would require its own measured admission decision.

### Remove the measurement abstraction

Rejected. The callback is the only route that can intentionally share fonts and layout behavior
with the final host, and distinct DOM operations remain useful even when the built-in answer is an
approximation.

## Supersedes And Amends

- Supersedes the production font-table decisions in ADR-0049 and ADR-0051.
- Amends ADR-0057 and ADR-0062: their operation ownership, host authority, and residual boundaries
  remain accepted, while generated runtime font facts are removed.
- Amends ADR-0073 and ADR-0081: deterministic measurement is the canonical built-in render and
  verification environment.
