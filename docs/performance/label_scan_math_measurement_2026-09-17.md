# Label scanning and math measurement structural repair

Base: `9edd6d86a2d6ad2d1707aa0edb3488483240277f`.
Evidence class: structural work removal; no latency or peak-memory improvement claim.
Local evidence: `target/bench/experiments/label-scan-math-fix-2026-09-17/`.

## Image-label normalization

Flowchart HTML emission previously converted the entire remaining label to ASCII lowercase at
every `<` position, only to check the four-byte `<img` prefix. With L input bytes and K candidate
positions, this adds O(KL) work and cumulative temporary allocation. Repeated fixed-size HTML tags
followed by an image reach Theta(L²). The live suffix allocation is O(L), not quadratic memory.

The replacement compares four bytes with `eq_ignore_ascii_case`. ASCII case folding preserves
byte length, so this predicate is equivalent even next to multibyte text. The cursor remains on
UTF-8 boundaries; only the fixed prefix is viewed as bytes. Real image tags consume non-overlapping
regions before advancing the cursor, making this normalization helper O(L), with no added state.
The initial image-presence scan, source extraction, image styles, sanitizer, and XHTML conversion
remain unchanged. This does not establish linear complexity for the complete HTML pipeline.

Tests cover strict/loose/antiscript/sandbox policy, case variants, Unicode, image-like prefixes,
missing attributes, unclosed tags, 1,600 surrounding spans, and public Flowchart SVG output.

## Built-in formula measurement

RaTeX measurement previously parsed and laid out each formula, constructed its display list,
serialized glyph SVG, rewrote the SVG root, and discarded the SVG. Measurement now stops after
display-list preparation and the original six-decimal dimension rounding. Actual rendering uses
the same preparation followed by the unchanged SVG serialization.

For N formulas in a successfully measured label, this removes N discarded SVG serializations and
root rewrites. Parsing, layout, display-list allocation, and their errors remain. Display-list ink
bounds are deliberately retained: smash/lap expressions can overflow the nominal layout box.
No cache, persistent state, public API, feature, or resource-accounting change is introduced.
Custom math callbacks and mixed-label probing are unchanged.

Tests compare measurements with actual emitted SVG width/height attributes, including fractions,
Unicode, overflow commands, multiple lines, font-size clamping, malformed formulas, and labels
that must continue to decline the built-in measurement path.

## Validation

- Focused nextest: 14 passed, including the public image and RaTeX Flowchart tests.
- Full `merman-render` nextest with `math,layout-cytoscape`: 1,758 passed, two skipped.
- Scoped Clippy with warnings denied and workspace formatting checks passed.
- Full `compare-all-svgs --check-dom --dom-mode structure --dom-decimals 3
  --diagnostic-browser-text-layout` passed on Windows.
- The separate full `parity-root` sweep also passed with the same precision and reviewed
  browser-text-layout residual policy.
- Independent source review confirmed the prefix equivalence, normalization bound, display-list
  dimensions, and lack of removed recoverable SVG-emission errors.
- `verify --strict` passed workspace all-feature compilation, then stopped at the pre-existing
  `clippy::field_reassign_with_default` in `merman-export/src/lib.rs:3234`. The same test helper is
  present in the base commit and untouched by this branch. Later strict gates were not reached.

Public-operation controls used the unchanged `pipeline` harness, default features plus `svg`, on
Windows 11 / Intel i7-11700 / Rust 1.95.0. The preserved base executable and candidate ran eight
alternating AB/BA pairs per fixture, with 30 Criterion samples, two-second warmup and three-second
measurement windows per invocation (48 invocations total). Every pre/post identity check passed;
base and candidate SVG bytes, hashes, and element counts matched for each fixture.

| Control | Median paired head/base | Median paired delta |
| --- | ---: | ---: |
| flowchart_medium | 1.0283 | +120.70 us |
| flowchart_large | 0.9987 | -25.54 us |
| sequence_medium | 1.0172 | +5.20 us |

No control breached the registered joint `>10% AND >50 us` investigation threshold. These are
supporting non-regression observations, not a speedup claim: there was no A/A power calibration,
the candidate is an uncommitted source snapshot, and the ordinary controls do not isolate the
image or formula stress shapes. Executable hashes, source-file hashes, fixture identities, raw
samples and per-run output receipts are retained in the local ledger directory.

This repair follows the existing runtime-neutral architecture. It does not change LSP scheduling,
Node-WASM execution, or the layout algorithms. Timing results from the earlier diagnostic review
are not candidate-admission evidence.
