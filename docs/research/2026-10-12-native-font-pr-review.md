# Native Font PR Review and Integration Checkpoint

Date: 2026-10-12, Asia/Shanghai. Windows 11 / Rust 1.95.0.
Branch: `feat/cli-native-font-context`.


Current delivery policy: ordinary Mermaid graphical CLI output defaults to
host-independent deterministic measurement. Compiled `native-fonts` and
`system-fonts` remain available, but a native context is created only for explicit
font sources, `--font-measurement native`, or `--font-system on`. Fixed font
sources remain closed unless system fallback is requested. Default SVG layout
performs no font acquisition; legacy PNG/JPEG/PDF painting can still discover
fonts. The maintainer chose not to bundle diagram text fonts. Earlier default-native
policy and measurements below are historical integration records.

The native-font implementation was committed as `cee23328b`. Remote main at
`54c5c6dfb` was merged in `728397269`, preserving upstream Unicode HTML wrapping,
managed-operation lifecycle and ASCII geometry changes. This checkpoint records
review corrections on top of that merge. Earlier performance reports retain their
original source identities and are not measurements of this final revision.

## Confirmed Review Corrections

- Keep fallback preference independent of paint order. A controlled Cousine/Noto
  fixture reproduced different measured and painted advances before the fix;
  discovery now chooses from the stable catalog, while dynamic databases reuse
  admitted snapshots without changing candidate priority.
- Share explicit/discovered face descriptor construction and grapheme splitting.
  Preserve fallible measurement, zero-width prefixes and progress on oversized
  visible graphemes.
- Measure already-decoded HTML candidates with the raw canvas-width operation in
  the Wrap phase. Keep full-label callback routing observable. Remove a second
  built-in HTML break parser that reinterpreted literal `<br>` text.
- Preserve original fragments when extracted text contains literal angle markup;
  layout and resvg-safe output now agree on native glyph advances and line breaks.
- Reuse one Markdown HTML projection, sanitization and XHTML normalization for
  output and inline-style admission. Pinned Mermaid emits unsupported tokens as
  raw HTML, so code/escaped spelling does not bypass native typography checks.
  Delete the separate Flowchart minimal Markdown projection. The independent
  XHTML-safe fragment API retains its existing literal-text behavior.
- Check Class typography using the canonical `classDiagram` family. Check rendered
  inline typography, allowing non-overriding `inherit` declarations including
  `!important`. Inheritance that overrides intrinsic bold, italic or code faces
  remains unsupported and is rejected.
  Swimlane title admission follows its local HTML-label switch and dedicated
  Markdown rendering; nested groups retain ordinary Flowchart behavior.
- Protect explicit font files and directories from output aliases and nested
  output namespaces. With directory sources, reject existing multi-hardlink outputs
  through constant-time link counts rather than scanning the font directory again.
  Preserve whole-batch failure and source-position diagnostics on font-budget errors.
- Project tspan width and height whitespace through the shared binding contract in
  UniFFI and WASM; use one line-height and white-space policy across transports.
- Verify all 17 native-font capability fields with exact JSON keys, types and values
  in release archive and installation checks. Reject experimental unbounded platform
  discovery builds as complete release installations. Update the Homebrew source
  recipe to the descriptor-owned native-font features.
- Give every CLI diagram its own native measurement state, fatal outcome and
  operation control while retaining the admitted font snapshot. A slim parallel
  build reproduced a second chart's label-byte failure being attributed to chart
  one through the shared latch. Both SVG measurement and encoded painting now use
  the same operation context, and sequential/parallel transaction regressions
  require the failing second fence's exact source location.
- Isolate HTTP resource fixture tests from unrelated host-font discovery. Their
  five-second listener deadline otherwise expired during debug system discovery.
  Dedicated default-discovery tests continue to exercise the actual CLI default.

## Verification Recorded

- Native facade, cross-family geometry, wrapping, Flowchart and Swimlane typography:
  all 33 final-tree nextest cases passed.
- Render text/HTML/Markdown, wrapping and Flowchart label output: 168 final-tree
  selected nextest cases passed.
- CLI unit and resource boundary suite: 264 cases passed, one platform-specific
  case skipped. An earlier full run passed 256 cases before two HTTP fixtures
  timed out; both fixtures passed after the isolation correction.
- UniFFI and WASM request projection regressions: both passed. Eight shared binding
  text-measurement cases and the `wasm32-unknown-unknown` target check passed.
- Release archive, installed CLI and Homebrew Python contracts: 102 cases passed,
  one platform-specific case skipped.
- CLI process, third-party materials, release surface and platform-script Python
  contracts: 63 cases passed, two platform-specific cases skipped.
- CI plan contract: 36 cases passed, including the new cross-platform Swimlane test.

Additional final-tree checks are recorded below after completion. Linux/macOS host
font behavior, Android instrumentation and Apple package execution are owned by
remote CI and are not represented as locally executed.

## Final Contract Checks

- Native ABI, shared binding contract and generated-source freshness checks passed.
- Feature-matrix verification passed: 36 descriptor recipes, 16 isolated consumers
  and 42 representative compile configurations.
- All 394 release legal-material projections and the third-party license contract
  passed. Bash and PowerShell completion syntax/routing checks passed using native
  Git Bash and PowerShell; fish, zsh, elvish and mandoc are unavailable locally.
- Full-corpus SVG comparison passed in one release-mode invocation with
  `structure,parity,parity-root`, three-decimal precision, root reporting and exact
  reviewed browser-text-layout receipts. Parser diagnostic residuals are reported
  separately and are not counted as DOM parity.
- Platform binding verification passed both Android `aarch64-linux-android`
  JNI and C ABI descriptor builds with Clippy `-D warnings`. Flutter package
  resolution stopped because the installed Dart SDK is 3.9.2 while the repository
  requires >=3.10.0. Flutter generation, analysis, formatting and native-assets
  execution remain deferred to the platform-binding CI lane with its supported SDK.
- Final native-font primitive/export tests passed all eight cases; all ten focused
  SVG root-contract tests passed.
- CLI final directory-protection rerun: 262 unit/font-contract cases passed (one
  platform-specific case skipped), 11 slim native-font cases passed, and default
  CLI all-target Clippy passed with `-D warnings`. An independent security review
  confirmed the Windows file-link-count ABI against SDK 10.0.26100.0 and found no
  additional confirmed P1/P2 in the final directory protection.
- The descriptor-owned `cli-release` Windows host recipe built successfully with
  its exact features and thin-LTO `dist` profile. The real executable passed release
  runtime verification (version, exact capabilities, completion, SVG/PNG/JPEG/PDF
  and Rustdoc) and installation verification after copying it and all release
  support assets into a temporary prefix.

- After the per-diagram font-state correction, the default CLI unit/native/font
  suite passed all 272 cases (one platform-specific case skipped). The exact slim
  `svg,diagram-flowchart,native-fonts,markdown,parallel-markdown` recipe passed all
  13 contract cases, including both serial/parallel failure provenance and batch
  input-directory protection before any publication.
- A separate correctness reviewer traced the per-operation context through
  facade requests, Markdown child controls, raw-SVG codecs and registry copy-on-write
  admission. It found no additional confirmed P1/P2; the CLI regressions above
  supply the runtime evidence for the parallel failure scenario.
- The final CLI operation/directory correction is committed as `2e614e8a9`.
  Default all-target CLI Clippy passed with `-D warnings`; a CLI build with
  `--no-default-features --features svg,diagram-flowchart` also passed, proving
  the native-context changes remain gated. Workspace formatting and staged diff
  whitespace checks passed.
- The descriptor-owned Windows `dist` binary was rebuilt after `2e614e8a9`
  (2m 46s incremental build) and again passed both runtime and installation/asset
  verification. This rerun covers the operation-context correction in the actual
  release executable, including all compiled SVG/PNG/JPEG/PDF and Rustdoc routes.

## CI Correction Pass

The first PR run (`38164621151`, head `0d9f3d229`) identified owner-specific
contract gaps. Its release artifact recipes passed on Linux, macOS and Windows;
the aggregate gate failed on the following independent causes:

- Linux all-feature CLI Clippy and experimental platform-query tests require
  `libfontconfig1-dev` and `pkg-config`. Both Linux owners now install these before
  their checks, with the existing bounded APT retry/timeout policy.
- The registry alias test must canonicalize its host temporary root before
  constructing a normal font source. This resolves macOS `/var` aliases without
  changing production symlink rejection.
- Web pure-dist smoke now asserts the complete append-only 22-operation protocol,
  stable original mapping and the three new signed-length operations. Flutter's
  native ABI projection includes the exact six-line ffigen delta recorded by CI.
- Swimlane's controlled measurement fixtures now implement Canvas advance width.
  SVG fallback tests retain the already-normalized whitespace in emitted text,
  including normal-mode lines, instead of expecting a missing `xml:space` marker.
- The existing layout snapshot generator refreshed exactly 15 Class, one
  Flowchart and one State golden. Class/Flowchart literal-angle labels now use
  structured HTML measurement and its final 1/64px lattice. State Markdown escape
  handling removes the extra backslash that old measurement counted; the edge
  label width changes from 179.48 to 173.18 and derived geometry follows. Pinned
  Mermaid's Marked 16.4.2 projection and checked-in upstream SVG agree with the new
  escape semantics. All 99 generated differences are numeric geometry; IDs,
  labels, topology and other values are unchanged. Inputs, comparator policy and
  numeric test tolerance are unchanged.

Local correction checks passed: 73 fallback/Swimlane cases; 45 native/bounded-font
cases; 114 experimental-font/codec cases (11 host cases intentionally ignored);
all-feature CLI Clippy with `-D warnings`; 77 workflow/platform/process Python
contracts (two platform-specific skips); Web pure-dist smoke and its focused Node
contracts. The local complete Web smoke has stale WASM package inputs, which the
freshness verifier rejects; the rebuilt remote Web owner supplies that evidence.
Local Dart 3.9.2 remains below the package constraint. CI's exact ffigen delta was
applied and statically checked against the C header without lowering the SDK
requirement; the supported-SDK platform owner performs full generation and checks.

The expanded render owner rerun passed all 1,638 library, Swimlane and complete
layout-snapshot cases with all diagrams, both optional layout engines and math.
Formatting and diff whitespace checks passed. Corrections are recorded in
`3ef2c52bd` (platform/prerequisite/projection contracts) and the following focused
render-test/snapshot commit; remote owners rerun after publication.

A second CI run (`38192873616`, head `d64a98db9`) passed Flutter platform
verification and reached a further macOS default-discovery failure: a 188,589,668
byte system collection was compared with the 67,108,864 byte resident budget
while scanning metadata. Discovery now uses only its cumulative temporary
scan-byte budget; selected snapshots retain the independent resident check. No
default limit was raised. The new controlled-font regression failed on the old
code, then passed together with all 46 bounded-font and 115 experimental/codec
cases (11 intentionally ignored host cases). All-feature CLI Clippy passed again.
An independent correctness review confirmed the separate budget ownership and
selected-source enforcement. Documentation clarifies that resident byte limits
exclude temporary whole-file scan buffers and do not describe peak RSS.

All five local Web/WASM packages were rebuilt through the descriptor-owned npm
pipeline and passed input freshness, assembled-package validation, package and
same-process smoke, and DOM-safety smoke. The complete Web script suite then
identified two upstream integration test projections: complete renderer closures
must include `merman-unicode-linebreak`, and the interactive layout ceiling is
14,100,000 (as owned by Rust and generated contracts), rather than the old 800,000.
The precise expected closures and exact/plus-one boundary assertions were aligned
without changing production policy. The final suite passed 129 cases with two
platform-specific skips and no failures.

The latest Linux owner (`38194358823`, head `26773050d`) passed all 9,416
workspace tests (seven intentional skips), the explicit native-font process
contracts and host-sensitive font contracts before detecting a stale committed
Rustdoc generator capability digest. Regenerating the bundle with the default
CLI changed only `receipt.json`'s capability digest; both Markdown/SVG fragments
and their recorded hashes remain identical. Rustdoc rendering already explicitly
selects deterministic measurement and performs no native font discovery. The
strict `rustdoc check` command passed after the reviewed regeneration. No receipt
validation, rendering policy or CI check was relaxed.


## Runtime Opt-In Policy Revision

After reviewing the font-heavy host's complete CLI startup cost, the maintainer
requested deterministic measurement as the ordinary default. The revised font
admission creates no native context for default SVG, explicit deterministic SVG
or source-free `--font-system off` SVG; its direct unit regression checks the
returned context, and the process contracts verify default/deterministic output
identity and post-layout CSS compatibility. Explicit files/directories still
activate shared native measurement with a closed font set. System native
measurement requires an explicit runtime request. Native and system support stay
compiled into default/release recipes; binary painting's existing acquisition
policy is preserved. No diagram text font was bundled.

An independent correctness review found no confirmed P1/P2 in the policy change.
Verification passed: 299 CLI unit/native/distribution/contract cases (one
intentional skip), 12 minimal explicit-font process cases, the direct default
context regression, selected graphical export cases, all-feature/all-target
Clippy, 80 release/installation Python contracts (two platform skips), 36 asset
and artifact-recipe Python cases (one platform skip), capability projection
freshness, all 36 artifact recipes, and Web's 37 exports/48 bindings/five package
entry contract check. Rustdoc regeneration changed only its capability digest
and the strict check passed. The descriptor-owned Windows `dist` binary passed
real runtime and installation checks with the tracked support assets.

The [runtime-policy probe](../performance/cli_font_runtime_policy_2026-10-12.md)
records nine complete-process observations per input/policy, source and artifact
identities, and its limited same-host scope. Candidate default SVG latency is
43.37/45.74 ms for two/25 nodes; explicit-font latency is 45.60/52.84 ms. Default
matches explicit deterministic SVG hashes, and deterministic/explicit-font SVG
hashes are unchanged across the policy revision. Historical measurement records
remain intact. The Changelog includes font-family configuration, file/directory
examples and the recommendation to use fixed local fonts for layout accuracy.


## Documentation and release preparation

The complete CI run [38198585396](https://github.com/Latias94/merman/actions/runs/38198585396)
passed on code commit `720b5bd46d1fe84a7abb0e6cab29c43ec83cb446`: 56 successful jobs and
one policy-selected skip. This includes Linux workspace tests, SVG DOM comparison and Chromium
root-viewport checks, Windows/macOS native owners, Web, Apple, Flutter, and VS Code packaging.
The Performance workflow passed its selected contract lanes; benchmark execution was skipped.

README history shows deliberate homepage simplification on 2026-07-27 (`1349fcf09`) and
Rust-library positioning on 2026-08-13 (`766454e7e`). Later additions accumulated release,
migration, lifecycle, feature, and font details. The project overview now has 141 lines rather
than 335, with rendered examples ahead of a complete Rust quick start, a short CLI entry,
integration links, and compatibility/license boundaries. Detailed font guidance remains in
the CLI README and Changelog. Entry guides were reconciled with the published 0.8.0 baseline.

Typst Universe was checked on 2026-10-12 and lists wrapper 0.4.0 as published on 2026-10-09,
bundling Merman 0.8.0 and requiring Typst 0.15.0. Its source README now starts with direct
registry usage; local builds remain in Development. The release skill and public release guide
require final user documentation before source freeze and package inspection. Registry
availability and publication receipts stay in release records.

The documentation follow-up checked 110 repository link targets, fenced code, README anchors,
and whitespace. An independent review checked the onboarding path and release-preparation
rules. No compiled code or artifact profiles changed in this documentation follow-up.
