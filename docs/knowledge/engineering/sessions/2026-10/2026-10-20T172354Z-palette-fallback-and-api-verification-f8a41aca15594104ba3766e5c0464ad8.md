---
type: "Session Handoff"
title: "Palette fallback and API verification"
description: "Session Handoff for Palette fallback and API verification."
timestamp: 2026-10-20T17:23:54Z
record_id: "f8a41aca15594104ba3766e5c0464ad8"
producer_id: "codex-windows"
run_id: "host-theme-228-232-20261020"
---

# Summary

# Verified State

# Open Threads

# Next Action

# Citations

## Scope and current source

Continue the active Goal and `docs/plans/2026-10-20-2244-fix-host-theme-backports-plan.md`.
Main HEAD remains `9a5bf169a`; maintenance HEAD remains `c6939dba5` on the v0.9.1 worktree.
Both have task-owned uncommitted implementation and test changes; do not discard anything.
No publication, push, tag or external message is authorized. The release candidate is not ready yet.

## Verified changes and decisions

The user's `+pub struct MermaidConfig` concern was answered against the v0.9.1 private tuple
field. The type was already public and opaque; constructors, traits and JSON/debug behavior remain.
Default-feature cargo-semver-checks passed all 223 checks each for maintenance core/render/export.
Feature-bearing checks and a real old public consumer remain pending.

Public bindings uncovered a real ER Forest HTML alias ownership bug. Main ER now selects the
actual HTML nodeTextColor role when present, otherwise textColor. The new 4-way Editor/HTML test
passes in `main-c4-er-preset-final2.log`. Its earlier standalone red was a test class-token selector
mistake; the earlier bindings red is the valid product evidence.

Maintenance core `config/presentation.rs` now filters six optional aliases from the host-only
hole-fill defaults when their actual selected fallback string exists: nodeTextColor/textColor,
stateBorder/nodeBorder, requirementEdgeLabelBackground/edgeLabelBackground,
commitLineColor/lineColor, emSwimlaneBackgroundOdd/emSwimlaneBackground,
emSwimlaneBackgroundStroke/emSwimlaneBorder. The last two preserve existing Merman compatibility
readers; do not claim upstream uses those aliases. Raw/site/source/detector explicit aliases remain.
Independent reviewer confirmed the mappings, asked for as_str presence, and root applied it.
Maintenance presentation tests add missing-alias and explicit-alias controls; faceColor replaces
commitLineColor as the valid extension-missing host fallback control.

Main C4 now admits paired Node/NodeLabel roles with actual receipts and source-local ownership.
No core c4.* palette broadening: selected Mermaid palettes lack native C4 node roles, so their
native defaults must not acquire authored ownership. Existing no-host blue/white remains. Evidence
is `target/diagnostics/issues-228-232-20261020/c4-node-paint-ownership-evidence.md`.
All but one C4 test passed; worker corrected the last test to read inline style effective fill
instead of a non-existent fill attribute. This final test correction awaits rerun.
Catalog's 10 fingerprints were updated from exact compiled values in main-c4-er-preset-final2.log;
rerun its unit test before declaring the catalog green.

Main empty Zen theme identity was fixed by passing an explicit replace-default-background bool to
SourceStyle::resolve, from the root plan's actual background request. A Some(empty theme) no
longer makes the native frame transparent. Original public bindings rerun remains pending.

## Test migration results

`main-migrations-final.log`: 374 run, 358 pass, 16 fail, 1 skipped. Requirement 14, Cluster 1,
Venn 1 were retained classic Default recipe mistakes: Mermaid 12 family defaults are Redux Color.
Requirement worker fixed recipe binding, exact compatibility residuals, strict author controls,
and a parse/session mismatch; frozen and not rerun yet.
Cluster helper is now named render_with_retained_recipe_baseline, explicitly installs Default/Base,
accounts for exactly 1 compatibility residual, preserves family RequirePortable enforcement, and
asserts source residual=0. Venn distinguishes recipe and external selection; external Default
retains its actual #333 terminal (Base derives #654321). Both passed in the 140-test run.
Timeline audit found a second output missing final evidence; root added compatibility=1,
applied=1 and theme residual=0 to that empty Timeline default-stroke case. Rerun pending.
Independent migration reviewer found no other P1/P2. RootCanvas origin-based assertion and ELK
fallback expectation repair pre-existing base tests; neither changes production.

`main-c4-er-preset-final2.log`: 140 tests, 138 pass, 2 fail (unsigned catalog fingerprints and
C4 test selector, both now corrected but not rerun).
Main `cargo fmt --all` + `cargo fmt --all -- --check` and diff check passed after all frozen edits.

## Active command and next actions

Only active compile/test command is maintenance full nextest, session `88721`:
`cargo nextest run --locked -p merman-core -p merman-render -p merman -p merman-bindings-core
--features all-diagrams,svg,layout-cytoscape -j 1 --no-fail-fast`.
Shared target is main/target; DEV/TEST debug=0 and incremental=0. Compile finished 2m37s, 4667 tests
across 126 binaries running; log main/target/diagnostics/issues-228-232-20261020/maintenance-final-full.log.
Poll before another Cargo build. Root main-fmt session72013 completed. All workers frozen.
Avoid parallel Cargo and heavy resource consumption. All future nextest can explicitly use 2 threads.

Next: finish maintenance full tests; format maintenance; run current main core/render/bindings full
owners with latest C4/ER/Requirement/Zen fixes; build current native CLI with svg/png/pdf/native-fonts
and inspect 18 native outputs. Run feature leaves, default plus feature API compatibility/old-consumer,
Clippy, full SVG structure gate, legal generators and feature closure checks. Current existing CLI
is stale and only has SVG; do not use it to claim final native output evidence.

Then focused local commits on both branches, final mapping/readiness report, and clean linked
maintenance release-version projection to 0.9.2. npm12.0.2 matches the pin on both branches.
The existing verify_prerelease_compatibility.py skips stable releases; its fresh consumer recipe
can be reused, but a skipped command is not evidence of old stable facade compatibility.
Do not claim completed or mark Goal complete while these required gates remain.


## Later verification and findings

Maintenance full run completed: 4667 tests, 4640 pass, 27 bindings fail, 4 skipped. All non-binding
core/render/facade owners pass. The binding failures are a pre-existing ambient feature-owner mismatch:
-p merman enables ELK/math below a bindings owner without its own cfg flags. Relevant paths match tag.
Rerun only bindings with all-diagrams,svg,layout-cytoscape,layout-elk,math passed 220/220, including
all four original issue public regressions (`maintenance-bindings-complete.log`).
Use nextest `--build-jobs 1 --test-threads 2`: its -j means TEST concurrency, not Cargo build jobs.
Set CARGO_BUILD_JOBS=1 for other commands. Earlier -j1 runs were test-serial, not build-serial.

Native maintenance public binding probe generated 18 SVG/PNG/PDF each, six original families and
no/light/dark preset. Probe projects are ignored under target/diagnostics/.../native-probe-{branch}.
18 PNGs have real content and 18 PDFs parse as single pages. Contact sheet revealed a real Zen
failure despite previous string CSS assertions: raw XML serializer changed CSS quotes to &quot;,
whose semicolons confused the existing resvg-safe style sanitizer; unclosed font-family swallowed
following declarations. Native light/dark participants black (7069 black pixels), dark actor surface
zero pixels. Red SVG/PNG/PDFs retained in native-maintenance-red. Worker repaired maintenance Zen
writer with existing svg::pipeline::escape_xml_text_with_checkpoints (preserves quotes/hash), added
roxmltree+cssparser parsed stylesheet regression across light/dark×parity/resvg-safe. Needs nextest
and native regeneration; no global sanitizer changes. Main already uses XmlTextOutput correctly.

Maintenance Clippy first lacked jpeg, exposing pre-existing test-only unused helper. Complete
png/jpeg/pdf/native font/math/layout owner rerun caught manual_contains in our Requirement CSS test;
root corrected styles.iter().any(equal) to styles.contains. Complete Clippy needs final rerun.
Maintenance full cargo fmt --all is Windows OS206 due long worktree+argument list. Per-package
cargo fmt and --check passed for core/render/export/facade/bindings; root diff check passed.
Main fmt all passed; all added indexed paths still within160 UTF16units (stage final new files
before final length check).

CURRENT ROOT CARGO: session75400 main full nextest of core/render/bindings with all-diagrams,svg,
layout-cytoscape,layout-elk,math, buildjobs1/testthreads2, debug0/incremental0. It compiled and
runs7627 tests. Log main-final-full.log. Early failure empty typed theme Wardley byte identity,
worker main_er_facade_owner_fix now owns main Wardley to fix actual None-vs-empty behavior.
No other Cargo active. Three new C4 occurrence regressions were added AFTER this binary build;
root must run them red after current run: c4_node_receipts_track_layout_occurrences_after_alias_updates,
c4_text_receipts_track_each_relationship_label_occurrence,
c4_boundary_text_receipts_track_layout_occurrences_after_alias_updates. Worker main_c4_node_paint_fix
awaits red before changing production occurrence storage. Independent correctness reviewer found
alias-keyed receipt incorrectly rejects legal alias collisions, endpoints-keyed relationship paint
loses per-occurrence visible label receipt. Preserve last source metadata by alias/endpoints but
use actual layout occurrence paint/index for validation and actual shape-type defaults.

C4 review also found generic TVtext/line/cluster keys unused by native C4 terminal, so main C4
worker removed that false role ownership, retained source/c4.* explicit and generic Text fallback
owner, and added C4 scoped EdgeLabel/ClusterLabel preset roles. This changes10 fingerprints AGAIN;
root must update exact values from latest compiled fingerprint unit test before final pass.

Independent maintenance reviewer found unrelated custom detector as_value_mut root writes copied
entire host palette as raw (P2). Agent final_maintenance_semantics_review now owns maintenance
core/config/mod,presentation,parse_pipeline and necessary private equal_values helper plus
presentation_test regressions. Root must not edit those until frozen. Suggested cheap begin-tracker
snapshot, actual broad-access diff paths, preserve precise same-value setter writes and whole config
replacement semantics. No provenance framework/new API.

Maintenance docs worker completed CHANGELOG Unreleased and OPTIONS_JSON/presentation-themes docs,
only legacy contract, no theme API port, old0.9.1history unchanged. Main custom guide adds Node pair
and independent Wardley arrow labels. All docs uncommitted.
