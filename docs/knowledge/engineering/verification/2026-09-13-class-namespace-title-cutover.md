# Class namespace Title scalar cutover verification

Date: 2026-09-13

Source commit: `51f6308acb4c931ad135aca255ba940288932641`.

Class's historical `Title.fill` channel colors namespace labels. Four static unqualified/Default
solid/transparent routes now use the shared namespace writer and direct terminal receipts.
Generic `Text.fill` retains its four legacy routes and participates in author order. The top-level
diagram title has not acquired a new consumer.

The verified inventory is KTD17 v84, 470 routes and 706 native route-profile witnesses;
36 legacy routes remain (Block 32, Class 4). KTD23 stays at v9 with 80 historical routes and
160 value probes. Support revision 86 describes Class Title as a partial typed surface, and
its shared discovery fixture contains 15 vectors. These counts are migration inventory, not
C7a qualification or completion percentages.

## Terminal and ownership boundaries

Ordinary, extracted-root, and ELK namespaces share one writer. It preserves coordinates and
unthemed markup while emitting both CSS `color` and `fill` on the actual label span. The receipt
requires the complete expected namespace ID set and exact rule/style bindings; missing,
foreign, duplicate, and mismatched events remain rejected.

Source `titleColor` provenance, including default `textColor` and base `tertiaryTextColor`
derivations, retains ownership. An unrelated source property does not suppress typed paint.
Absent namespaces and overwritten rules are NotApplicable. Winning Clear, ordinal, and
unsupported sibling facets keep their residuals. Solid and transparent values remain distinct
from Clear.

A new mathematical namespace test exposed an intermediate bug: the receipt treated unproven
paint inheritance as malformed emission and rejected BestEffort with InvalidModel. Structural
completion now checks emitted terminals separately from paint evidence. Valid mathematical or
invisible terminals can complete emission but cannot prove Title paint; BestEffort returns the
SVG with Incomplete evidence (one required mechanism, zero accounted), while RequirePortable
rejects it. The test asserts the actual mathematical writer marker, not merely a fabricated
receipt event.

## Independent review

Standards: no actionable findings. The shared writer removes duplicated output logic and the
private plan follows existing Class ownership boundaries.

Spec/correctness: no actionable findings after follow-up. The review requested direct Clear,
derived-owner, and mathematical-writer cases; these cases were added. The reviewer separately
checked the structural-completion correction and the revision-86 partial support claim.
The reviews were read-only and do not substitute for executable checks.

## Verification

The current-worktree private Release owner combination passed 3270/3270, with two existing skipped
tests. It includes full renderer, facade, bindings-core and acceptance library tests, Class SVG
with ELK/math, support discovery, authoring, route authorization, and Block/Class retirement tests.
All 470 routes and 706 native route-profile witnesses passed. Log:
`/tmp/class-namespace-title-owner-final.log`.

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render -p merman-theme-acceptance -p merman-bindings-core -p merman \
  --features merman-render/layout-elk,merman-render/math,merman-bindings-core/svg --lib \
  --test class_svg_test --test theme_support_discovery_test --test theme_authoring \
  --test route_cutover_runtime --test legacy_projection_retirement \
  --test block_title_legacy_projection --test class_edge_label_background_legacy_projection \
  --no-fail-fast
```

The first native combination passed 475/476; the remaining failure was the independent legacy
inventory's stale 40-route count/digest, updated to the authorized 36-route inventory before the
final run. The original migration fixture failed against the old implementation with
LegacyFamilyThemeCompatibility; log `/tmp/class-namespace-title-red.log`.

C ABI, UniFFI (one-shot and reusable), and Typst compiled transport support golden tests passed
3/3 against all 15 revision-86 vectors. These are compiled Rust transport checks, not installed
foreign-language packages. Log: `/tmp/class-namespace-title-transport.log`.

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-ffi -p merman-uniffi -p merman-typst-plugin -p merman-theme-acceptance \
  --features merman-ffi/svg,merman-ffi/math,merman-ffi/layout-elk,merman-uniffi/svg,merman-typst-plugin/svg \
  --lib -E 'test(theme_support_matches_shared_golden)'
```

The complete blocking SVG structure comparison passed (exit 0); formatting and diff checks also
passed. Log: `/tmp/class-namespace-title-structure.log`.

```text
CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
```

## Clean-checkout confirmation

The exact source checkout `/tmp/merman-class-title-51f6308ac` passed the same owner command:
3270/3270, with two existing skipped tests. It was clean both before and after the run. Dependencies
used the shared target; all workspace Rust/manifest timestamps were refreshed to force source
rebuilding. The log confirms core, renderer, facade, and bindings-core compilation from the clean
checkout. This avoids the stale cross-checkout linkage described in the earlier Class Cluster record.

The resulting `class_svg_test-1f99acb4b68b252f` executable contains the clean-checkout path;
its only embedded core source root is
`/private/tmp/merman-class-title-51f6308ac/crates/merman-core`.
Executable SHA-256: `70838b0069f24f20f784e9ab0d950d398b80add3024646480d9644c0c8cb6d4b`.

Log: `/tmp/class-namespace-title-clean-owner.log`.
Log SHA-256: `b36fa1256f579056edd6b048c9be7423836e049c7f4dbb6c09be6589f4c1fe40`.
Source/status/binary observations: `/tmp/class-namespace-title-clean-state.json`.
The clean confirmation covers the owner command; transport golden and complete SVG structure
results above were run from the main worktree against the identical committed source.

## Remaining scope

Class generic Text and Block retain live compatibility work. C7a artifact/profile rollout,
installed revision-86 consumer verification, Web size budgets, and formal candidate/contract
freeze remain open. Earlier installed revision-85 observations remain tied to their recorded
source commits. This slice does not publish qualification cells or upgrade artifact profiles.
