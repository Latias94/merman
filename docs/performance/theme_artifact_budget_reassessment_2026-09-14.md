# Theme artifact budget reassessment — 2026-09-14

## Decision rule

Size budgets are regression guards, not immutable product requirements. The maintainer accepts
raising them when the larger product is justified. Preserve required semantics, diagnostics,
resource limits and font safety; remove accidental dependency reachability and material duplication
before treating an increase as a new baseline. Exact attribution of every byte is not required.

A replacement baseline needs one committed source, the canonical recipes for all five Web profiles
and Typst, functional/package checks, tool versions and artifact hashes, and all four measured size
metrics. Keep the existing approximately three-percent headroom policy unless another margin has
a stated workload reason. Do not raise budgets from mixed historical artifacts. This record does
not change limits or close the C7a artifact gate.

## Existing Web packages measured

The existing package files were measured with the built `xtask wasm-size-matrix --surface web
--web-package-root <absolute package root> --budget-file <absolute budget file>` command.
This mode reads package artifacts and does not rebuild them. It checks their profile metadata.
Their provenance JSON does not contain a source commit; these are **not current-HEAD build claims**.
The source checkout was `2e06b0101` plus in-flight Block Cluster changes, which the size command
neither compiled nor included in those package bytes. Raw inputs were unchanged by measurement.

| Profile | Raw | Stripped | Gzip | Brotli | Brotli above existing limit |
| --- | ---: | ---: | ---: | ---: | ---: |
| web-analysis | 3,674,774 | 3,674,509 | 1,412,163 | 1,074,487 | 2.33% |
| web-ascii | 5,195,193 | 5,194,928 | 1,913,913 | 1,445,450 | 1.44% |
| web-editor | 3,785,907 | 3,785,642 | 1,456,727 | 1,105,081 | 0.46% |
| web-full | 15,900,334 | 15,900,069 | 5,933,543 | 4,367,710 | 9.74% |
| web-render | 14,003,464 | 14,003,199 | 5,283,006 | 3,893,562 | 13.68% |

All twenty checks fail the existing budget. The small-profile overages and complete-renderer
overages require separate explanations; font/theme rendering costs do not explain an analysis-only
package. The result is a size observation, not a benchmark speedup or attribution experiment.

| Package | Input SHA-256 |
| --- | --- |
| analysis | `dcebf8f920791f3204e79193bea1253b5f055a01a1898100e34af2cdc079cf23` |
| ascii | `fb9493da708faee13b5893e6c41165b781f1e9cb2bb95ee9359e943a9ddad59c` |
| editor | `cc470999edb824d4aafd3fb4f219955f84fc91ea3a94001bfa4a6f2caa098fca` |
| full | `410cd59ef64aaf71716bc6844d96e64efc24dfc31f0af914493783c096b6f2c0` |
| render | `ce1b470617a845f093b0ac8526e8bd15f783fce17fffc38564def905184754c1` |

Measurement executable SHA-256: `e7b76b8d10beccb34d0bc8db6ccce4493d88420414a391df54337ede3180fd89`.
The ignored experiment ledger, full retained provenance, budget hash and raw output are under
`target/bench/experiments/web-existing-artifacts-20260914-2e06b0101/`.

## What the historical anchors establish

The six artifact feature lists retain the same names, but their implementation capabilities have
changed. The accepted budget notes in `docs/release/WASM_SIZE_BUDGETS.json` identify the historical
anchors; they are not an equal-capability requirement for the new theme product.

- The ASCII semantic-depth increase was already admitted in the August 23 ASCII/full budget
  change. Do not count that work again to justify a second increase.
- The old renderer anchors predate the complete typed theme authoring/schema, materialization,
  preset export and resource/font-validation surface. Their fixed size cannot be imposed without
  accounting for those new capabilities. Font shaping, font parsing, WOFF2/Brotli decoding and
  resource digests have specific production consumers; mere presence in Cargo.lock is not proof
  that a dependency reaches every artifact.
- Earlier ICU collation and vendored font-table runtime removal offset some additions. Net size
  cannot be attributed by summing new dependencies.
- The retained `4e4f3acc3` Web package set is a useful recent control. Slim-profile core/ASCII
  sources, manifests, lockfile and profile registry are unchanged between that source and
  `2e06b0101`; complete render profiles also include subsequent typed-support changes.
- The `1ca41032c` CI measurement cited by the budget is not a locally available Git object.
  Its later merge commit must not be represented as the identical binary baseline.

The [assignment extraction](theme_assignment_wasm_size_2026-09-12.md) already removed about
109–111 KB raw from each Web artifact. The [evidence index repair](theme_evidence_requirement_index_2026-09-13.md)
removed quadratic work with a small size decrease. The
[typography winner repair](theme_typography_recomputation_2026-09-13.md) deliberately accepted
small size growth for reduced repeated work. Failed `-Oz`, unstable-sort and evidence-clone
experiments remain documented; repeating those rejected micro-optimizations is not the next gate.

## Next evidence needed

Finish the current consumer/renderer slice, then build and smoke all six canonical artifacts at
one source. Capture effective dependency closures for slim versus render profiles to rule out
unintended capability inclusion. Use the recent same-product control to detect unexplained growth,
and the older accepted anchors only to explain product evolution. If no material accidental cost
remains, record that source as the accepted product baseline and update each budget from its actual
raw/stripped/gzip/Brotli result plus headroom. Typst needs its canonical post-link measurement;
this Web-only run provides no new Typst size evidence.
