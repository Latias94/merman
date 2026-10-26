# Astro rendering contract repair: validation record

Date: 2026-10-26. Scope: Class typography, Canvas font admission, and scoped
JavaScript icon services. This is correctness and compatibility evidence, not a
performance or release-size qualification.

## Release boundaries

| Line | Base | Mermaid | Options JSON | Web transport API |
| --- | --- | --- | --- | --- |
| main | `93034e2076322e6e52515999e6b3c39cf4d69aff` | 12.2 | 3 | 8 |
| release/0.9.x | `18fcf90a4` | 12.1 | 2 | 5 |

Both implementations preserve their existing public options and ordinary
service-free calls. The maintained Web session exposes its existing SVG/layout/
edge outputs; only main additionally exposes SVG-result metadata methods.

## Focused verification

- Main: 567 renderer/Class, binding-core, and WASM tests pass after the final
  pack-count fix, including the controlled ELK routing fixture.
- Maintenance: 243 binding-core/WASM and 121 renderer/Class tests pass. Three
  focused JSON icon admission tests pass after the final pack-count fix.
- Node Rust transport tests: main 23, maintenance 18, all passing.
- Web script suites: main 171, maintenance 139, all passing with Chromium enabled.
- Node script suites: main 119, maintenance 103, all passing with npm 12.0.2.
- TypeScript and generated Web facade checks pass on both lines. The observed
  raw WASM export counts are 42/37, with 56/49 runtime bindings across five entries.

Rust tests use nextest and serial Cargo builds (`CARGO_BUILD_JOBS=1`). Browser
and Node smoke artifacts were built in the development profile to verify actual
execution; their bytes and timing are not production-size or performance data.

## Actual browser and transport evidence

Chromium tests exercise locally built browser WASM. They check default typography,
serif, and monospace with a deliberately different root wrapping font. Both
Vehicle and Driver title bounds fit; member, method, and relation HTML rows also
fit their width and height containers. Font readiness is explicit.

For example, default Vehicle text and container widths are both 48.265625 CSS
pixels; Driver widths are both 40.46875. These observations describe this browser
environment, not exact arbitrary-font deterministic rendering.

The Canvas test covers rejected assignments, canonicalized and relative weights,
quoted punctuation, sequential stale-font risks, and save/restore behavior.

Native and Node-WASM smoke checks pass on both lines. Each executes nine SVG
renders, four constructor-error cases, four isolation checks, and three disposal
checks. They exercise already loaded packs, registration-name and built-in icon
overrides, request overlays, omitted/empty services, and structured errors. Web
also exercises synchronous callback reentry and callback-time disposal against
the real WASM artifact.

## Independent review and follow-up

An independent Codex review covered both diffs, API contracts, typography,
resource admission, lifecycle, generated contracts, and the controlled routing
fixture. Review run: `20261026-093859-7f40598b`.

The confirmed issue was a sequence-visitor early return for 18 or more packs.
Both implementations now stop at the first excess pack and preserve the
structured resource error before handling generic JSON errors. Tests cover
16 successful packs and rejected 17/18/32 packs, with actual=17, max=16, and
pack_index=16. Independent static follow-up confirms the correction.

The ELK routing test uses a test-local fixed measurement profile so font repairs
do not change its staircase precondition. It retains both terminal directions,
unchanged endpoints, removal of exactly two points, and channel alignment.

No upstream SVG baseline or comparator normalization is changed. The exact
Class semantic residual refresh is explained in the issue research appendix.

## Structure gate and toolchain

Both lines independently pass the exact full 37-family structure invocation:

```text
cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 \
  --diagnostic-browser-text-layout
```

The first run identified missing worktree-local Mermaid CLI dependencies for
math fixtures and eight obsolete exact Class local geometry signatures. Install
those dependencies from the checked-in lockfile; do not change renderer features
to bypass math admission. Refresh only the reviewed Class local signatures and
reasons, preserving input/upstream hashes and every comparator rule. The rebuilt
invocation on each line then exits successfully across all 37 families.

Strict Clippy passes on both lines with the affected render, binding-core, and
WASM production libraries under the full browser feature recipe. Formatting and
path-length checks and minimal-feature binding-core/WASM builds pass on both lines.
Other-platform packaging and CI jobs remain owned by CI; this local run qualifies macOS arm64 native, browser WASM, and Node-WASM.

## Local commit mapping

| Unit | main implementation | Maintenance adaptation |
| --- | --- | --- |
| Canvas font admission | `d086a3f1f` | `c2c52d112` |
| Final Class typography | `ad6da20cc` | `5e5bc2fd8` |
| Scoped JavaScript icons | `3619de837` | `4aabeeeb5` |

The patches are local feature-branch commits based on each remote release line.
No remote branch, package, tag, pull request, or upstream issue was changed.
