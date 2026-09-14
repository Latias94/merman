# Class Text writer emission

Source: `ab35340381fe2a52d0319119f6a6eebe14ba2007`. Baseline: `0ac136dbf`.

Class edge labels, cardinalities, diagram titles and notes now pass actual writer style
emissions into the existing text receipt. They no longer reconstruct an emitted style
from the expected paint. The shared attribute writer returns a borrowed value only when
the output sink accepts the attribute. No SVG reparse, second string cache or public
contract was added.

HTML/SVG labels retain their original attribute order, empty styles, text interpretation
and escaping. The note path preserves source-owned and unverified/math text facts.
An accepted attribute is not a completed document: subsequent output failure remains
sticky, and Class seals its receipt only after output and root completion succeed.

## Verification

The final private Release owner command passed **195/195** selected tests: 109 Class SVG
integration tests and 86 internal Class/shared-label tests. The 2539 other tests were
filtered out. The earlier 193-pass run preceded the note and output-failure additions;
it is not the final verification.

```text
CARGO_BUILD_JOBS=2 python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-render --features layout-elk,math --lib --test class_svg_test \
  -E 'test(class::) | binary(class_svg_test) | test(svg::parity::label::)' --no-fail-fast

CARGO_BUILD_JOBS=2 cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 --diagnostic-browser-text-layout
```

Eight writer/receipt channels cover HTML, SVG and fallback edge labels; start/end
cardinality slots; root titles; and HTML/SVG notes. Each fixes the theme expectation and
passes absent, incomplete, wrong-color, wrong-fill and correct styles through the real
writer and receipt. Only the correct paint is Applied. Cardinality cases exercise the
shared opening writer and distinct receipt slots, not fault injection into both complete
edge-layout branches.

Bounded-sink cases reject writes before the attribute, within its value and before its
closing quote. Another case accepts the attribute but rejects the body and `finish()`.
The public themed renderer additionally accepts the exact SVG byte budget and rejects
one byte less without returning a completed artifact.

The complete structure command passed. Class selected 251 fixtures, rendered 249, and
retained its two existing exclusions and reviewed text-layout residuals. No new waiver
was added. Structure mode does not prove root containment, browser visibility or native
export paint. Formatting, diff checks and both read-only review axes passed.

Logs: `/tmp/class-writer-emission-owner-final.log`,
`/tmp/class-writer-emission-structure.log`, and
`/tmp/class-writer-emission-structure-report.md`.
Owner log SHA-256: `a7c16b990442b89b9d9d11ac8b5f020664aef4c294d0b065d760295303493c47`.

## Remaining work

This source was verified in the main worktree, not a new clean checkout. The prior
`9be480c86` clean result does not cover this change. The completed Class migration must
pass its broader owner suite and clean-checkout replay before retirement.

Partially shadowed mixed rules still need capability accounting based on surviving
properties. Class support, bridge dispatch and cutover authorization remain unreconciled;
the earlier broader run's 38 failures have not been closed by these focused tests.
Native cutover witnesses, provider retirement and C7a artifact/rollout gates remain open.
