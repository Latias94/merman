# Font measurement follow-up

Date: 2026-10-26. This records correctness evidence, not performance qualification.

## Decisions

- Preserve the independent host measurement protocol. DOM measurement stays
  opt-in, and the callback stays synchronous.
- Reuse a shared supported-font cascade on main. Resolve declarations against a
  fixed inherited style instead of repeatedly multiplying relative sizes.
- On 0.9, reuse its existing token-aware font declaration resolver rather than
  importing main's newer source-ownership or theme contracts.
- Match Class row measurement to the pinned terminal SVG. Inner tspans explicitly
  reset font style and weight; an abstract-method row's parent italic declaration
  does not override its normal child. Do not change that DOM to make a test pass.
- Match Flowchart HTML measurement to the actual emitted root font size. Retain
  the release line's existing numeric/string parsing semantics, including legacy
  leading-integer parsing of string sizes. Changing fractional-string rendering
  alone would leave 0.9 SVG layout and emission inconsistent.
- Main does not know a browser document's root font size. Keep compatible rem
  output approximate, but do not certify a root-dependent size as portable or
  native-exact without that context. Shape-only declarations must not be confused
  with label typography. The 0.9 backport does not import the newer strict
  source-style evidence framework.

## CI causes

The reviewed runs are main PR 246 run `40275906229` and maintenance PR 247 run
`40271585837`.

1. Both Web jobs fail in `smoke.mjs` because its fake Canvas context lacks the
   save/restore behavior required by the earlier font-state isolation fix. Update
   the test adapter, including DOM connection state; keep runtime admission strict.
2. Both Linux workspace jobs fail the exact layout snapshot test after the earlier
   Class typography fix. Regenerate with the existing owner after semantic tests
   pass; do not edit snapshot numbers by hand or loosen comparison precision.
3. Maintenance hygiene rejects an npm-generated lock that omits the parent local
   workspace entry. Accept either native npm shape while still requiring actual
   linked child packages and validating every present entry and version.

Each line refreshes 209 Class snapshots, one Zed Class snapshot, and one numeric
Flowchart font-size snapshot. The Flowchart case previously measured a displayed
24px HTML label at 16px. No upstream SVG, comparator normalization, browser
residual tolerance, or CI gate assertion is relaxed.

## Verification record

Web tests include illegal assignment isolation, probe detachment/reconnection,
resource cleanup, and 18 independent DOM controls. Each control changes only one
of family, weight, and style; its computed style and geometry are compared with a
native text/div element rather than with another call to the same adapter.

Both release lines pass real Chromium Canvas and DOM tests without skipping.
The complete Web script suites pass on main (171 passed, three environment-gated
browser tests skipped in that invocation) and 0.9 (139 passed, three skipped).
The Canvas and DOM browser tests are run separately with Chromium enabled.

Main passes 3,575 renderer tests (one existing test skipped), including the full
renderer library, Class/Flowchart integration suites, all layout snapshots, and
root-dependent typography admission. Nine native-font facade tests also pass.
Maintenance passes its 1,842-test focused renderer suite; after preserving legacy
string size parsing, a final 1,749-test library/Flowchart/snapshot invocation also
passes. These runs overlap and are not additive distinct-test totals.

Both release lines pass strict renderer-library Clippy and formatting. Main's
Class-only no-default-features build also succeeds; its existing partial-feature
warnings are not a full-feature Clippy pass. Both lines run the exact 37-family
release structure owner; no browser geometry receipt is broadened or refreshed.
The release projection suite passes all 18 tests on each line.

The assembled five-package WASM smoke remains a CI owner: local cached development
WASM is not a fresh production package assembly. The smoke measurement adapter is
checked separately against the current TypeScript implementation; this does not
replace package provenance or production-size validation.

The maintenance branch was pushed as `d4228a160`; its replacement CI run is in
progress, with no failed checks observed at the last inspection. Main's final
push starts a new run. Neither pending run is described as green.
