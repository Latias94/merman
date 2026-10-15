# Theme integration performance and artifact attribution — 2026-10-06

## Scope and evidence boundary

This is a diagnostic attribution follow-up to
[`theme_main_integration_2026-10-06.md`](theme_main_integration_2026-10-06.md). It compares
remote main `733fd2fa711cb25cf6d1d94f5797fff424f52d51` with the integrated-theme revision
`5c40e66679755fd0339653737cf5e55f596092cd`, using the same frozen native ARM64 benchmark
executables and local CLI artifacts. The comparison is between whole revisions; it is not a
counterfactual build with only theme code removed. No production source was changed for this
analysis.

Profiler percentages are inclusive attribution evidence and overlap when one function calls
another. They must not be added together. Render profiles also include Criterion preparation
that is outside the render timer; their percentages cannot be multiplied by the measured render
latency to estimate function time. The original timing checkpoint used two AB/BA pairs;
this follow-up does not claim a confirmed latency regression because it does not add the required
A/A calibration and eight-pair confirmation.

## Performance attribution

### Class parse is primarily configuration/theme materialization

The Class parse observation increased from 145.945 µs to 376.375 µs (+230.430 µs). In the head
10-second sample, 94.74% of main-thread samples were inside `parse_model_in_context`, with these
nested hotspots:

- `preprocess_with_directive_recovery_controlled`: 78.69%;
- `finish_preprocessed_controlled`: 76.41%;
- `clone_value_nonrecursive_controlled`: 59.47%;
- `materialize_selected_theme`: 35.32%;
- `apply_theme_defaults`: 19.63%;
- Class semantic construction: 15.27%.

The corresponding base sample had 45.04% in preprocessing, 39.15% in the finish step, 32.47% in
value cloning, 4.90% in theme default application, and 40.43% in Class construction. The profile
therefore points first to the new effective-config path: clone the config around type detection,
track mutations, resolve appearance, materialize the selected theme, apply defaults, then replay
mutations. The new Class style-precedence evidence is a plausible secondary cost, but it is not
the dominant sampled path for this fixture.

The causal hypothesis to test next is: **avoid redundant effective-config cloning and theme
materialization when the complete operation policy proves that the selected theme and derived
variables are already current, while preserving mutation replay and compatibility shadowing**.
A safe experiment needs an operation-scoped equivalence proof; a fixture or diagram-family
allow-list would be invalid.

### SVG rendering adds shared finalization work and prepared-text scanning

Class render increased from 230.625 µs to 321.395 µs (+90.770 µs), and Mindmap render increased
from 60.382 µs to 98.221 µs (+37.839 µs). The head profiles show:

- `FamilyRenderArtifact::render_svg`: 49.27% for Class and 60.29% for Mindmap;
- `partition_prepared_text_label_ids`: 10.31% for Class and 19.67% for Mindmap;
- the end-to-end samples also show SHA-256 frames at 4.34% for Class and 7.12% for Mindmap.

For an empty prepared-text ledger, the current code still scans the complete SVG for the reserved
prepared-text spelling before returning the original string. That safety check is semantically
intentional, so this is a hypothesis rather than an approved shortcut. The exact empty-ledger
branch frequency was not instrumented. The next experiment should
measure that branch and test an owner-produced “reserved token may exist” fact across every
renderer/postprocessor mutation, or an equivalent faster bounded check. It must not assume that
a family or fixture cannot emit the spelling. The final SVG validation/resource-closure path is
another shared render cost and should be measured separately before changing it.

### What the timing evidence does not show

Mindmap parsing was lower in this run (130.285 µs to 112.085 µs), and its layout delta was small
(+2.011 µs). This argues against a universal parser slowdown. The full seven-fixture end-to-end
run remains a contract failure because Flowchart, Sequence and Treemap outputs differ; those rows
are not valid timing controls until their output contracts are reconciled.

## CLI artifact attribution

The local `cli-release` ARM64 executable changed as follows:

| Measurement | Main | Integrated theme | Delta |
| --- | ---: | ---: | ---: |
| Raw executable | 45,511,408 | 53,462,736 | +7,951,328 (+17.47%) |
| Raw gzip-9 | 18,803,233 | 22,054,346 | +3,251,113 (+17.29%) |
| `__text` | 28,615,244 | 34,082,052 | +5,466,808 |
| `strip -x` executable | 39,890,048 | 46,613,408 | +6,723,360 |
| `strip -x` gzip-9 | 17,222,716 | 20,151,446 | +2,928,730 |

The strip controls preserved the exact `__text` bytes and reproduced each original artifact's
version, capabilities and four SVG outputs. These are within-revision controls, not a claim that
all base/head outputs match. Removing local symbols reduces only about 1.17 MiB of the raw delta.
The remaining approximately
6.41 MiB still includes machine code, read-only data, load metadata and other link-edit content;
it is not all proven removable code. The `__text` increase is nevertheless clear evidence that
this is not a symbol-table-only effect.

Function extent attribution gives the following directional split:

| Named owner | Main text | Integrated-theme text | Delta |
| --- | ---: | ---: | ---: |
| `merman_render` | 4,396,772 | 7,437,528 | +3,040,756 |
| `merman_core` | 3,464,468 | 3,619,464 | +154,996 |
| `merman_theme_contract` | 0 | 149,956 | +149,956 |
| `merman_bindings_core` | 0 | 14,492 | +14,492 |
| Other/shared named Merman crates | 3,762,492 | 3,934,020 | +171,528 |
| No Merman namespace (includes external/generic code) | 16,991,512 | 18,926,592 | +1,935,080 |

The `merman_render` delta is the largest named contribution. Direct module-name growth includes
`merman_render::svg` (+958,668 bytes), `merman_render::diagram_theme` (+252,888),
`merman_render::family` (+196,204), and `merman_render::state` (+177,240). Separately, the binary-wide
generic/trait bucket grows by 849,280 bytes; it overlaps project association and is not an
additional renderer subtotal. These numbers attribute symbol extents, not counterfactual
removable bytes; inlining, generic monomorphization, aliases and thin-LTO make exact ownership
impossible from the binary alone.

There is one correction to avoid in interpretation: `merman-bindings-core` and
`merman-theme-contract` are new workspace packages, not external dependencies. The only new
third-party package identity in the normal feature closure is `serde_json_canonicalizer`.
`data-encoding` changes its feature set, but is already present in the dependency closure, as
is `sha2`; new direct dependency edges are not necessarily new packages. The canonicalizer has
no separately named text symbol in this LTO build, which does not imply zero inlined cost. The
new package identities are therefore not sufficient to explain the approximately 6.41 MiB
stripped delta. The current evidence points mainly to newly reachable renderer/theme integration
and validation code, with generic code generation as a substantial secondary contributor.

## Conclusions and next experiments

1. **Most likely latency source:** operation config cloning and theme materialization in the
   Class parse path. Measure an operation-scoped no-op/materialization reuse candidate with exact
   config and output receipts.
2. **Likely shared render source:** prepared-text empty-ledger scanning, plus final SVG validation
   and resource fingerprinting. Attribute each with an owned counter or a separate benchmark
   stage before editing.
3. **Most likely size source:** `merman-render` and shared generic/LTO code, not the new external
   package alone. Build-size experiments should isolate feature reachability and generic
   instantiations; symbol totals must remain attribution evidence.
4. **No optimization is admitted by this report:** no candidate has passed semantic, SVG/DOM,
   resource, cancellation, error and decision-grade timing gates.

Raw profiles, symbol maps, dependency deltas, strip controls and the machine-readable receipt are
in `target/bench/experiments/theme-attribution-2026-10-06/` and
[`theme_main_integration_attribution_2026-10-06.json`](evidence/theme_main_integration_attribution_2026-10-06.json).
