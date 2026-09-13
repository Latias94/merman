# Block EdgeLabelBackground typed cutover

Date: 2026-09-13

The baseline at `f1e25cbc3` established a real legacy CSS consumer. The first typed
implementation at `02e1a030e` passed its narrow Block tests, but did not close
acceptance: the private-cfg runner exposed missing projection obligations, stale
inventory assertions, and a stale public support claim. A plain Cargo invocation
without `merman_internal_theme_acceptance` ran zero acceptance tests and is not
verification evidence.

The completed slice uses KTD17 v81 with four added route identities (unqualified
and Default, each solid/transparent) and the existing EdgeLabelBackgroundFill
projection. The total is 452 routes and 666 route-profile witnesses. Each added
route runs Classic, Neo, and HandDrawn SVG/native PNG proof. KTD23 v9 remains
unchanged because this is a typed replacement, not Unsupported retirement.

The Block writer consumes the typed background in `.edgeLabel`, `.edgeLabel rect`,
and `.labelBkg`. Only finite positive label areas contribute terminal paint evidence.
The obsolete `themeVariables.edgeLabelBackground` bridge assignment is removed.
Explicit user configuration retains ownership; absent labels are NotApplicable;
unsupported ordinal fills and winning stroke siblings remain residuals. The normal
family evidence merge is retained, including its required-set assertion; an inactive
background plan is skipped instead of introducing a weaker subset-merge API.

Public support revision 83 changes Block background fill from legacy partial to
partial typed support. The shared support JSON adds a Block query. Compiled Native
C ABI, UniFFI, and Typst tests consume all shared vectors; installed packages last
verified at revision 82 must be rebuilt separately.

Verified locally during implementation:

- Block SVG and public discovery integration tests: 91/91.
- Acceptance manifest and background native proof unit tests: 22/22.
- Final Release renderer matrix, bridge, Block, and public discovery tests: 311/311.
  This includes the exact Block theme-residual error in strict mode.
- Native C ABI, UniFFI, and Typst shared support golden: 3/3 with explicit SVG features.

The complete structure gate passed:

```text
cargo run --locked --release -p xtask -- compare-all-svgs \
  --check-dom --dom-mode structure --dom-decimals 3 \
  --diagnostic-browser-text-layout
```

The final KTD17 v81 Release integration gate passed 9/9, including the complete
route-cutover authorization and Block/Class historical retirement witnesses:

```text
python3 scripts/run_theme_acceptance.py nextest run --release --locked \
  -p merman-theme-acceptance --test route_cutover_runtime \
  --test legacy_projection_retirement --test block_title_legacy_projection \
  --test class_edge_label_background_legacy_projection --no-fail-fast
```

The internal-cfg wrapper is required. These checks do not close C7a, C7b, or the
outstanding WASM size gate.
