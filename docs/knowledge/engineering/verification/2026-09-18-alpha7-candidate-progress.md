# Alpha.7 candidate progress at `425679fd7`

Date: 2026-09-18. Release line: `v0.8.0-alpha.7`. This is a candidate progress record;
it does not tag, publish or freeze C7a.

## Current-source evidence

- The public Cyberpunk recipe now has a separate Flowchart Edge glow. Updating the recipe changed
  its canonical fingerprint from the previous catalog value to
  `1583f2a8e9d1a249624873741825f6df6b864cdb042dd832223dde8dbdb57ff9`.
- `preset_catalog_fingerprints_match_the_exact_compiled_recipes` passed after the catalog update.
- The complete preset-focused renderer suite passed 13/13, including recipe exchange, resource
  limits, effect limits, complete-spec round trips and catalog projections.
- Theme acceptance `preset_qualification` passed 2/2 under the workspace-only acceptance cfg. The
  current contract still intentionally leaves all public preset `qualified_cells` empty; Cyberpunk
  remains rejected by qualification because text/other scene consumers are incomplete.
- `python3 -m unittest discover -s scripts -p 'test_*.py'` passed 613/613. This includes the
  alpha.7 version projection, legal-material, release/preflight and compatibility-script tests.
  The output records the accepted alpha.6 transition policy and fresh alpha.7 consumer resolution.
- The bounded Flowchart Cyberpunk recipe/export/import and native edge/node regressions remain
  green from the preceding scoped record.

## Not established by this record

The following are still open for the final same-source candidate and must not be inferred from the
local checks above:

- installed Web, Node, Python, Typst, C/UniFFI, Apple, Flutter and Android package matrix;
- Linux/Windows/macOS archive replay from one clean candidate source;
- public catalog qualification cells bound to final artifacts;
- complete Flowchart text glow, Sequence and XY Chart public scenes;
- full modern_mermaid portfolio/application matrix on current Merman output;
- matched alpha.6/alpha.7 package size, cold start, throughput and memory comparison;
- release-preflight execution on every declared host and final contract freeze.

The two pre-existing untracked `docs/knowledge/engineering/{logs,registry}/2026-08/` directories
were not modified or staged.
