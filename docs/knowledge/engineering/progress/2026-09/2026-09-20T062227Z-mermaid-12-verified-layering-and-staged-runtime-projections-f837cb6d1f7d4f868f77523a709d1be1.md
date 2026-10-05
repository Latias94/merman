---
type: "Work Progress"
title: "Mermaid 12 verified layering and staged runtime projections"
description: "Work Progress for Mermaid 12 verified layering and staged runtime projections."
timestamp: 2026-09-20T06:22:27Z
record_id: "f837cb6d1f7d4f868f77523a709d1be1"
producer_id: "codex-mermaid12"
run_id: "session-01a0bd10"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "e02f1e13d"
supersedes: "9c1028518e1842a08b5469d9b807f17a"
---

# Summary

The Mermaid 12 implementation goal remains active. This checkpoint supersedes the initial
implementation checkpoint. Five focused implementation commits are present; full Mermaid 12
admission is still incomplete. The selected production bundle and generated runtime defaults
remain 11.17.2. No baseline was relabeled, and no remote publication occurred.

# Details

## Committed implementation

- `75c36648f`: six source-backed ELK layerers, work controls, typed failures, and 30 published
  elkjs 0.9.3 phase observations, kept inside `merman-elk-layered`'s existing EPL boundary.
  All 351 crate tests passed. The Interactive discrepancy was an oracle capture bug: elkjs had
  already mutated input positions. Recapture clones the input before execution; all expected
  layer orders stayed unchanged. Complete-pipeline tests inspect published geometry because
  HierarchicalNodeResizer intentionally clears transient layer indexes.
- `7577b9874`: source/initialization/default appearance resolution, raw initialization deltas,
  theme sentinel handling, and removal of obsolete renderer-based family detection. Historical
  explicit known-type aliases remain. Nine appearance/source-presentation tests and the focused
  detector/config/catalog tests passed. This is the resolver foundation, not a completed switch
  to the target generated defaults.
- `e02f1e13d`: all upstream SVG families use the existing scripted Puppeteer renderer. Absent
  theme stays absent. Only directly selected external plugins register; standard Mermaid 12
  cannot be overwritten by the CLI's transitive old ELK plugin. Added a narrow
  `--reference-bundle` option to the existing default/theme generators, with exact package
  version/digest checks and explicit output paths. Built-in ELK registry extraction reads the
  known algorithm list from the selected Git commit; no generic TypeScript evaluator. The TS
  projection supports an absent external ELK companion. All 42 generator tests and 24
  config/theme/reference tests passed.
- `e61d3823b`: one shared renderer option projection replaces three copies in Flowchart/Class/ER.
  The adapter exposes seven layering choices and Coffman-Graham's layer bound. The public Rust
  `LayeredOptions` struct adds two fields (documented breaking change in the commit footer).
  Existing adapter tests passed (40), a new real-layout test proves bound 1 versus 2 changes
  diamond geometry, and four focused renderer tests passed. Preset and hierarchy migration
  are NOT complete; this commit only connects the root layering controls.

## Target runtime and artifacts

- Isolated workspace remains `target/mermaid-12-admission/reference`: Mermaid 12.0.0,
  CLI 11.17.0, Tidy 1.0.1, ZenUML adapter 1.0.1/core 3.50.1, Puppeteer 25.6.0.
- The exact official npm audit signatures command already succeeded once (53 verified,
  zero invalid/missing), via npm 12.0.2. Do not repeat without a changed install graph.
  Raw output remains ignored `target/mermaid-12-admission/npm-signatures.json`.
- Mermaid package digest:
  `df542ed953a0a169733d7a3d8f339a964d7ae19b196c83254ebfc4599904d333`.
- The temporary `target-bundle.json` is still an incomplete staging input: release identity and
  workspace are accurate, but other graph/receipt fields are old. Never promote it as-is.
- Puppeteer browser download completed successfully. No running download remains. Pinned binary:
  `C:/Users/Frankorz/.cache/puppeteer/chrome-headless-shell/win64-151.0.7922.77/chrome-headless-shell-win64/chrome-headless-shell.exe`.
- The actual updated renderer script successfully rendered default Flowchart, explicit
  classic/Dagre, Agentflow, Usecase, and Tidy using this pinned browser. Earlier Edge smoke
  samples were replaced by these target-browser samples; these are development evidence only.
- Existing generators successfully produced staged `default_config.json`,
  `default_config_shape.json`, `theme_variables_12_0_0.json`, and
  `theme_variables_oracle_12_0_0.json` under `target/mermaid-12-admission/`.
- Target defaults confirm global ELK, scoped redux-color/neo, 120px minimum widths, and ELK
  `preset: default`, `layeringLayerBound: 4`, `lineHops: true`, `straightenEdges: true`.
  Strategy/alignment options are undefined so presets can supply them. They must not be
  prefilled with legacy values when promoting the generated data.

## Core regression correction

A full core-library run exposed three Swimlane regressions during the mixed-baseline transition.
The old runtime has no `swimlane.layout` config key; it supplied that default in its renderer.
A narrow documented bridge in `config/appearance.rs` supplies only the missing family default,
below both user layers. Remove this bridge when the target generated defaults are promoted.
The old Swimlane test also expected initialized Dagre to lose to the family default. It now
asserts Mermaid 12's explicit initialization precedence. A direct call to the installed target
runtime confirmed directive `layout: null` preserves initialized Dagre, while `layout: elk`
selects ELK. The correction is committed as `2cf7bc846`; the full core retest passed all 1,538 tests
(nextest run `8cda40d0-3fa0-4b3a-ba3b-f761fe43368d`).

# Next Action

1. Continue U5 with the exact target presets and root/container option ownership. Current
   `merman-elk-layered/src/importer.rs::nested_graph_options` still embeds old selective Mermaid
   policy. Target container options explicitly resolve placement/alignment/cycle breaking;
   root layering is not automatically a container override. Default preset uses
   NETWORK_SIMPLEX / BRANDES_KOEPF / BALANCED / DEPTH_FIRST. Named modelOrder/depthFirst presets
   have different root and container placement. Move Mermaid policy to the adapter rather than
   blindly cloning root options inside the EPL kernel. Current shared renderer module is
   `crates/merman-render/src/elk_options.rs`.
2. Complete U1 companion selection identities and reviewed receipt through existing admission
   machinery. Then carry on with U3/U6+ and the native new-family/editor work in the accepted plan.
   U2 target defaults/theme evaluator, U3 backend dispatch/default features, U5 postprocessing,
   U6 extra algorithms, Agentflow/Usecase native implementations, editor/bindings, legal closure,
   and final integrated admission all remain open.
3. Perform independent code review before completing the overall goal. The earlier six review
   agents reviewed the PLAN only. No implementation-wide independent review is claimed yet.
4. Cargo remains serial and uses the normal target directory. Do not rerun passing broad suites
   without new changes or an unresolved concern. No push, PR or release is authorized here.

# Citations

- [Accepted implementation plan](../../../../plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md)
- [Reference workflow](../../../../../tools/upstreams/README.md)
