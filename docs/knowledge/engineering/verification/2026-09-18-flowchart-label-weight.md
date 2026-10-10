# Flowchart Label Weight Consumption

Date: 2026-09-18. Base: `21c76a030`. Plan: U6 in
[theme product boundaries](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md).

## Contract

Static/default NodeLabel and EdgeLabel font-weight rules now have Flowchart and Swimlane
consumers. Each diagram resolves the two target weights once, and the existing label sidecar
shares them between measurement and concrete SVG/HTML writers. Clear restores the resolved
base weight. The ordinary public Cyberpunk recipe requests weight 600 for the two Flowchart
targets, preserving these rules through recipe export/import. Its compiled fingerprint is
`1583f2a8e9d1a249624873741825f6df6b864cdb042dd832223dde8dbdb57ff9`.

Source declarations retain precedence. Explicit root string `fontWeight` also overrides typed
rules; layout and emission normalize it against CSS initial weight 400 before source-relative
weights are evaluated. For example, root 900 plus source lighter produces 700. Invalid root
weight retains an unverified outcome instead of hiding behind configuration ownership. Numeric
root configuration is not a new input domain in this increment. Without a typed theme, an
explicit string root weight also reaches writer measurement consistently; no-root defaults
retain their existing behavior. This is a deliberate configuration consistency change.

Ordinary SVG word tspans receive the effective weight themselves: their existing normal-weight
reset cannot override the outer declaration. This is independent of complete font-catalog
admission, whose ledger and other typography facets retain their own requirements. No font
asset or dependency is added. Native font availability and measurement profile guarantees
remain separate from consuming a numeric weight declaration.

HTML emphasis, headings, table headers, form controls, inline font ownership, custom classes,
math and Markdown remain conservative residuals where the writer cannot establish the same
measured weight. Discovery reports Conditional for these target/facet combinations. Other
variants, ordinal selectors and base typography weight are not newly supported.

## Evidence so far

- Before implementation, the plain SVG strict test failed with two structured theme residuals.
- The first writer implementation failed because inner tspans still had normal weight. The
  terminal fix then passed ordinary SVG/HTML, Flowchart/Swimlane, source800, relative source900
  and Clear400 cases.
- Measurement callbacks confirm the requested node600, edge500 and source900 values. HTML
  writers may reuse layout bounds without another callback; tests check every callback that
  occurs without requiring redundant measurement.
- Chromium 151.0.7922.34 verified eight actual SVGs and 24 visible nonzero-area text terminals.
  Computed weights were node600 (or Clear400), source800 and edge500. Artifacts are under
  `/tmp/merman-flowchart-weight-browser`; temporary capture code was removed.
- The public compile/export/import test first failed with normal weight, then passed with 600.
  The catalog fingerprint was updated only after comparing its actual compiled value.
- A focused native PNG test passed: typed600 and equivalent source-owned600 produced identical
  dimensions and pixels; replacing both recipe weights with400 changed native output. This is
  an end-to-end consistency observation, not an isolated glyph-shape benchmark, because layout
  can also change.
- An expanded Release run passed 2,932 tests and failed three assertions exposing the missing
  public support-manifest rows and HTML form-control admission. Both issues were independently
  reviewed, reproduced before their fixes, and corrected. A separate root-weight measurement
  probe exposed layout900 versus emitted600; configuration ownership and shared relative
  baselines were then corrected.

The final Renderer Release run with `math,layout-cytoscape` passed **2,936 tests, two skipped**.
It covers library tests, theme resolution, label measurement, public recipe/node effects,
Flowchart SVG and marker output, and public support discovery. Root configuration, source
relative/class/important precedence, and valid configuration with invalid source values are
included. The final tracked code diff matches the completed review snapshot exactly
(`71acfc77a195644ef36ed50c31dbf849cf50e2ae53021b2bc05ed98ffb3567c3`); the only subsequent
code change adds two local Clippy arity allowances to the existing row writers.

The seven-lens code review and independent finding validator completed with no remaining
findings after caller fixes. Simplification used three persona prompts; balanced-model routes
failed, so an inherited-model reviewer completed all three lenses. It removed repeated owned
style cloning and redundant node source-style resolution without adding a generic abstraction.
Native Release geometry and marker regression passed **6/6, none skipped**, including the
final typed/source weight contrast. Workspace formatting and whitespace checks passed.
C6 runtime and preset qualification passed **4/4, none skipped**. Nextest initially flagged
one successful catalog test as `leaky`; its isolated recheck passed without the flag. The cause
of the transient flag was not established, and the initial observation is retained here.
Scoped Renderer Release Clippy completed successfully with 156 warnings, matching the
previous increment's warning count. This is not a warning-free build or a dead-code cleanup.

## Scope limits

This increment does not complete U6: text/edge glow and the complete SVG/PNG/PDF reference scene
remain open. It does not close C7a, rebuild installed platforms, add qualification cells, or
claim new runtime/size measurements. The two unnecessary style clones found by simplification
review were removed; no speedup is claimed without a benchmark.

Logs: `/tmp/merman-flowchart-weight-{green3,preset-red,focused,native,native-final,regression,config,config-check,final,qualification,qualification-recheck,clippy-final}.log`.
Review: `/tmp/compound-engineering-501/ce-code-review/20260918-143621-weight-6483de`.
