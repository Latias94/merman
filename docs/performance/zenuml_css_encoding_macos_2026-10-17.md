# Zenuml CSS encoding ownership — macOS, 2026-10-17

This checkpoint fixes configured CSS values being XML-encoded twice in Zenuml. The adjacent base
is `db465f6e6ab725db00d20503f09dd812c65872a0` (runtime equal to `4060f91e1`). This is a
correctness repair, with a separate allocation reduction on identical-output public controls.
No calibrated latency improvement or release-gap recovery is claimed.

## Encoding ownership

`zenuml_css` previously called `escape_xml` on 25 value arguments before formatting the complete
stylesheet, which then passed through `XmlTextOutput`. That text-label helper also decodes Mermaid
entity spellings; it is not the owner of configured CSS. An accepted font such as `"A&B", monospace`
therefore remained entity-encoded after parsing the SVG as XML, producing incorrect CSS.

The generator now formats logical values into its existing complete CSS String. The same single
write still sends that String through the existing final XML writer. Font-stack normalization,
selectors, attributes, label escaping, shared XML code, cancellation and sticky-first-error logic
are unchanged. The repair adds no cache, dependency, unsafe code, streaming formatter or new
output observer. Literal `&amp;`, `#quot;`, numeric spellings and Unicode remain logical CSS values;
forbidden XML scalars are removed by the existing final writer. Raw configured colors gain no
new CSS-grammar safety guarantee.

Corrected authored CSS intentionally changes serialized bytes. Its byte ceilings, error `actual`,
retained prefix and internal cancellation observation count can therefore differ from the broken
old stream. The existing sink rules apply to the corrected stream; old/new error-detail equality
is not claimed. The new public exact-length/one-byte-below test uses the corrected output length.

## Public controls

The preregistered corpus retains 59 preceding public inputs and adds 15 direct site-config cases
before production edits. The older source-init font rows remain recorded without being counted
as effective authored-font coverage. Five-repeat allocation captures and full ordered custom text
measurement traces cover all 74 inputs on both sides.

Fifty-seven SVGs match byte for byte: 39 reduce allocation count/bytes and 18 are unchanged.
None increases peak growth or retained bytes. All 74 theme reports and ordered measurement traces
match; all 152 default benchmark SVG identities match. The other 17 inputs change CSS text only:
after removing style text, their parsed SVG structure, attributes and geometry are identical.

| Identical-output public input | Allocation count, base → candidate | Allocated bytes saved |
| --- | ---: | ---: |
| Default medium Zenuml | 564 → 471 | 528 |
| Empty Zenuml | 233 → 140 | 528 |
| One ordinary message | 339 → 246 | 528 |
| 128 ordinary messages | 9565 → 9472 | 528 |

This is fixed stylesheet work per render rather than a message-count complexity improvement.
`escape_xml` allocates a String per call; its label decoder adds work even for ordinary `#hex`
colors. Removing that entire wrong-owner operation removes 93 allocations on these defaults.

The 17 corrected-output inputs save 77–108 allocation calls each. They are not same-work memory
or latency evidence. Five have 618–1,438 more cumulative allocated bytes and 1,676 more retained
bytes despite shorter final SVGs; this is consistent with changed append patterns and buffer growth, not a larger final document. Their peak growth
is unchanged. The ledger retains all four metrics and actual output lengths rather than hiding
these tradeoffs. The first checker incorrectly extended the identical-SVG memory gate to these
changed-output cases; its failure and checker correction are preserved. The preregistered gate
itself is unchanged.

## Latency and release context

The registered diagnostic used two balanced AB/BA pairs, 30 Criterion samples, two-second
warmup and three-second measurement per invocation. Outputs and postflight identities matched.
The candidate versus the unchanged current base measured:

| Operation | Base, us | Candidate, us | Observed change |
| --- | ---: | ---: | ---: |
| render/Zenuml medium | 13.7495 | 10.0680 | -26.78% |
| end-to-end/Zenuml medium | 31.4675 | 27.6735 | -12.06% |
| render/Packet control | 4.2939 | 4.3093 | +0.36% |
| end-to-end/Packet control | 11.1505 | 11.2785 | +1.15% |
| render/Timeline control | 14.8785 | 14.9305 | +0.35% |
| end-to-end/Timeline control | 43.5930 | 43.7290 | +0.31% |

These are attribution diagnostics only: no A/A calibration was run, so they do not establish a
latency admission claim. The Zenuml reduction is compatible with removing repeated CSS escaping,
but the correctness change also changes authored-output bytes; do not use it as a universal theme
latency explanation. The control rows stay within the preregistered material-regression screen.

Against the frozen v0.9.0 tag executable, the same-output Zenuml end-to-end context measured
20.0870 → 27.5365 us, or +37.09%. This is a release-range context measurement with differing
error/resource contracts and no A/A calibration; it does not isolate the theme refactor or claim
that this CSS repair closes the release gap.

## Verification and provenance

The two main public regressions fail on the old implementation and pass after the repair. Two
additional public tests cover invalid XML scalars and corrected actual output ceilings. The
focused Zenuml/XML suite passes all 49 tests. Existing shared-writer tests retain cancellation,
chunk-boundary and first-error coverage; no old assertion is removed or weakened.

Three independent read-only reviews cover correctness, XML security and maintainability/test
scope, with no required changes. Validation passes:

- Six-package all-features release nextest: 8,250 passed, 13 skipped.
- Five-package all-features/all-targets Clippy with warnings denied; format and diff checks.
- Private theme qualification: 34 passed; WASM host tests: 47 passed.
- Minimal SVG/Zenuml/Treemap/Venn feature compile.
- Actual optimized full Web WASM build and 37-diagram runtime smoke.
- DOM comparisons across 37 family groups in structure/parity/parity-root modes.

Production commit: `44fccd368` (`fix(zenuml): encode stylesheet values at the XML boundary`).
Pipeline SHA-256: `b922461aed1304595ab7248abdce7911bb74a4cbcc270e3235b98236f59c96e2`.

Raw preregistration, frozen corpus/source/artifact digests, red/green logs, allocator results,
measurement callbacks, output comparisons and diagnostic samples are in
`target/bench/experiments/zenuml-css-encoding-20261017/`. Cargo, allocator capture and timing
processes run serially on Apple M4 Pro, 48 GiB, macOS aarch64, Rust 1.95.0. Previous XML A/A
failure remains inconclusive and is not retried. No full strict release/transport-matrix
certification is claimed.
