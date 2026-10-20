# General theme authoring verification

Date: 2026-10-19. Scope: the coordinated replacement of the unreleased compact theme facade
under [ADR 0095](../adr/0095-general-theme-authoring-and-retained-bindings.md).

## Contract and migration

`ThemeRecipeV1` retains `schema_version: 1`. Its variants are `design` and `complete_spec`.
`ThemeDesignV1` replaces the compact definition in place; no V2 recipe, old decoder, or preset
inheritance interpreter remains. First-party transports, CLI assets, examples, fixtures, and
Playground exchange use the same grammar.

All ten built-in presets export ordinary designs with explicit shared bindings. Concrete
fingerprint assertions passed after updating representation changes: generated effect IDs,
structured solid paints, deterministic palette placement, and removal of redundant defaults.
Shared references have independent golden fixtures; literal equality never creates a binding.

## Executed checks

| Surface | Result |
| --- | --- |
| Contract crate | 61 nextest cases and 3 documentation examples passed |
| Authoring, materialization, renderer and bindings | Scoped nextest checks passed, including missing/cyclic/type-invalid references, map-only object admission, incremental expansion bounds, source paths, preset fingerprints, and independent reference goldens |
| SVG and native shadows | Composition tests passed, including independent red/blue shadows through SVG, PNG, and localized PDF; linearRGB raster assertions allow bounded RGB quantization while keeping alpha exact |
| WASM operation | The canonical theme-authoring operation test passed |
| CLI | Theme contract cases and regenerated completion/manual checks passed |
| Web runtime | Production full WASM rebuilt; full package smoke passed all 37 diagram cases; authoring/catalog tests and TypeScript checks passed |
| Playground | Production build, type checks, ESLint, and 169 runtime cases passed |
| Browser | All 18 theme scenarios passed in Chromium, plus 2 mobile toolbar/layout scenarios |
| Other transports | Node wire tests, Dart analysis, and example syntax checks passed; Apple/Android installed-library execution was not performed on this Windows host |

Browser scenarios exercise retained Cyberpunk bindings, custom named effects, independent hard
shadows, local detachment, atomic resource-ID rename, JSON/file/share exchange, undo/redo,
Apply/Cancel isolation, gradient stop bounds, invalid-form repair, workbench resizing, and a
Chinese 320px layout. Flowchart effect execution uses the supported Classic look. The existing
Neo effect limitation remains documented in the [theme guide](../rendering/custom-diagram-themes.md).

The scoped native test invocation must enable `merman-bindings-core/layout-elk` when workspace
feature unification enables ELK in the renderer. Omitting that binding feature caused an initial
admission mismatch; the corrected binding/export run passed all 100 selected cases.

## Performance scope

The development-profile `theme_operations` benchmark completed ten-sample sanity runs:

| Materialization workload | Observed interval |
| --- | --- |
| Default | 5.39–5.76 µs |
| Light | 687–764 µs |
| Dark | 678–712 µs |
| Complex Cyberpunk | 5.21–5.32 ms |
| 24 aliases and 128 consuming rules | 5.61–5.93 ms |

These short debug runs establish executable workloads, not release-performance comparisons or
a speedup claim. Alias resolution shares immutable cached values; authored input, unique cache
storage, incremental output collections, and final exact output have explicit admission bounds.
The recursion ceiling bounds active uncached calls, not total memoized alias-chain length.
Token and reference-use caps bound the whole graph. Prepared-theme rendering does not traverse
the authoring document again.

## Review and follow-up

The completed code review (`20261019-180820-e9302994`) reported no remaining actionable findings.
Its initial findings were fixed: shared-value amplification before admission, Serde sequence
representations of named objects, gradient ordering controls, and recovery of rejected visual
edits. Later atomic rename and own-property-safe path updates passed focused unit and browser
checks.

The testing review suggested an additional browser scenario for metadata regrouping and removal
after the final direct/alias consumer is detached. It remains advisory coverage: no corresponding
defect was established, and the delivered browser scenarios already verify ID rename, reference
preservation, detach, and exchange. Catalog qualification cells were not populated from these
development checks; design intent and actual output support remain separate.
