---
type: "Subagent Finding"
title: "Semantic and presentation configuration ownership"
description: "Concrete U5 config split preserving mutation events and excluding presentation definitions from lean builds."
timestamp: 2026-10-20T13:00:52Z
record_id: "95f1309d69b049bc9747d99802546d64"
producer_id: "codex-frankorz"
run_id: "semantic-capabilities-20261020"
subagent_id: "u5_config_compile_boundary"
related_plan: "docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md"
git_branch: "refactor/semantic-capability-boundaries"
git_commit: "0b6b4bf8e"
---

# Finding

Keep raw `MermaidConfig` as the always-compiled Arc-backed value owner. Introduce a concrete
`PresentationConfig` under the positive `presentation` capability, containing raw inputs plus
the existing theme compatibility, explicit paths, mutation journal/revision and detector epoch.
Place its implementation in `config/presentation.rs` and export it through `presentation`.
This is an independent owner with observable assignment semantics, not a migration alias.

Base retains raw constructors/readers, iterative COW/clone/drop, pure setters, the existing
assignWithDepth merge kernel, secure filtering/CSS admission, legacy font normalization, cheap
theme/look/layout selection, and raw default/directive-shape includes. Presentation owns all
ThemeParseBinding/compatibility/paint types, detector checkpoints/changes, three assignment
authorities, initialization epochs, sealing, frozen evidence and initialized-config caches.

# Implementation Boundary

Keep one merge algorithm: base invokes `assign_with_depth_recording` without collecting path
strings; full merge records paths during that same actual merge and preserves shadow/record/
explicit ordering. Do not rescan incoming JSON or add a mutation-provider trait.

Full setter ordering remains:

- public: shadow, public assignment, raw write;
- explicit: shadow, framework revision, raw write, mark explicit;
- preserving: framework revision, raw write.

Same-value writes still produce events while avoiding unnecessary JSON COW. Preserve both
framework revisions in `commit_legacy_theme_variables`.

Do not expose `DerefMut` or `&mut MermaidConfig` from the full owner. Full `as_value_mut` shadows
and records the root before returning the raw private mutable value. An immutable `as_config`
borrow serves family/sanitizer code. Whole-object replacements remain full owners, preserving
transferred-ownership detection. Post-hoc JSON comparison cannot recover same-value, old-clone,
root-assignment or nested-epoch events.

Full custom detector callbacks and `with_site_config`/`with_exact_site_config` take the full
owner. Converting their input to raw would erase unfinished nested-detector history. SemanticParser
accepts raw config on its independent methods. Presentation metadata keeps admitted source raw
config separately from the full effective config. Builtin lean matching uses the catalog and an
explicit Flowchart-ELK layout decision, never a fallback to full custom callbacks.

# Migration Hazards

1. Full defaults must start with upstream raw defaults and apply hardened secure policy through
   the full setter. Wrapping already-hardened raw defaults omits the old revision/epoch event.
2. Legacy font mirroring currently uses private `value_mut` without shadowing or recording a
   root assignment. Do not replace it with full `as_value_mut`. Keep private raw mutation access
   inside the implementation, not in the public API.
3. Security/CSS helpers read values rather than mutation/paint evidence and can take raw borrows.
4. Theme calculations operate on the full owner throughout; computing on raw and reconstructing
   provenance afterward loses authority.
5. Existing `MermaidConfig` equality compares values only. Preserve that public comparison on
   the full owner instead of deriving equality over its mutation journal and paint evidence.

# Compile Boundary

Move `MermaidThemeId` and its parse error to a cheap module; share `is_js_truthy` from config.
Gate theme calculation, staged runtime/oracle includes, `theme_color`, `ryu-js`, MaterializationPlan,
theme-only errors/reexports and `__private` paint/evidence definitions behind presentation.
Raw generated defaults remain in base. Appearance selection is shared, with two small concrete
apply methods; no generic setter abstraction is required.

# Protected Verification

Migrate these assertions intact to the full owner:

- `same_value_public_setter_keeps_detector_claims_and_shadows_without_json_copy`;
- `same_value_explicit_and_framework_setters_preserve_their_distinct_authority`;
- `detector_checkpoint_distinguishes_old_clone_setters_from_historical_journal`;
- `detector_epoch_survives_nested_detection_and_thread_handoff`;
- `detector_checkpoint_does_not_promote_framework_or_completed_operation_events`;
- `legacy_theme_commit_preserves_framework_mutation_order_and_detector_replay`;
- `replacement_replay_handles_literal_keys_deletions_and_only_actual_value_changes`;
- PR 225's `compiled_install_matches_legacy_host_reset_unknown_and_font_authority`,
  `compiled_bad_compatibility_color_stays_deferred_behind_preprocess_and_cancellation`, and
  `compiled_install_preserves_unfinished_nested_detector_clone_claims`;
- existing transactional theme, paint ownership and secure/source-filtering tests.

Use isolated lean compile-fail, full positive and ambient-union fixtures. Union-enabled
SemanticParser must still avoid theme initialization, while full cached-error phase and
assignment order remain unchanged.

# Disposition

Accepted as an implementation packet for the approved U5 boundary. This was read-only research;
no production change, compilation or runtime verification is claimed. U3 adjacent receipts and
U4 still precede U5. Reuse the existing admission and family-config audits rather than repeating
them. The approved plan remains the authority.

# Citations

- `crates/merman-core/src/config/mod.rs` and `config/paint_inputs.rs`.
- `crates/merman-core/src/{theme,theme_color,parse_pipeline}.rs`.
- `docs/knowledge/engineering/subagents/2026-10/semantic-admission-functions.md`.
- `docs/knowledge/engineering/subagents/2026-10/semantic-config-reads.md`.
- `docs/plans/2026-10-20-1252-refactor-semantic-capability-boundaries-plan.md` U5.
