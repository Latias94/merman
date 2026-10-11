# Native Font PR Review and Integration Checkpoint

Date: 2026-10-12, Asia/Shanghai. Windows 11 / Rust 1.95.0.
Branch: `feat/cli-native-font-context`.

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
