# Root Viewport Derivation Audit

This audit maps the workstream objective to concrete artifacts and gates.

## Objective

Replace fixture-scoped root viewport overrides with typed bounds derivation where practical,
starting with State and Mindmap, while keeping `parity-root` and strict release gates green.

## Prompt-to-Artifact Checklist

| Requirement | Artifact or command | Current state |
| --- | --- | --- |
| Track work in `docs/workstreams/root-viewport-derivation/` | This directory and its documents | Started |
| Start with State | `TODO.md`, `MILESTONES.md`, State override audit | In progress |
| Include Mindmap | `TODO.md`, `MILESTONES.md`, Mindmap override audit | Pending |
| Replace fixture-scoped overrides where practical | Code changes plus generated table deletion | Started: three State root pins removed |
| Keep `parity-root` green | Focused `compare-*-svgs --dom-mode parity-root` commands | Full State pass recorded |
| Keep clippy green for render edits | `cargo clippy -p merman-render --all-targets --all-features -- -D warnings` | Passed |
| Keep nextest green for shared behavior edits | `cargo nextest run` | Render crate nextest passed |
| Keep strict release gate green | `cargo run -p xtask -- verify --strict` | Pending |

## Current Baseline

The fearless-refactor closeout recorded these root viewport counts:

- State: `45` entries.
- Mindmap: `52` entries.

Current counts after the State style and entity-placeholder passes:

- State: `42` entries.
- Mindmap: `52` entries.
- Root viewport total: `757` entries.
- Text lookup total: `481` entries. This is an intentional one-entry increase because one shared
  State edge-label browser metric replaced two fixture-scoped root viewport pins.

The same closeout confirmed that broad disabled-root sweeps still fail for both buckets. This
workstream therefore focuses on derivation work, not blind deletion.

## Focused Commands

```sh
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- report-overrides --check-no-growth
cargo clippy -p merman-render --all-targets --all-features -- -D warnings
cargo run -p xtask -- verify --strict
```

PowerShell disabled-root diagnostic sweep:

```pwsh
$env:MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES='1'
cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
cargo run -p xtask -- compare-mindmap-svgs --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all
Remove-Item Env:\MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES
```

## Verification Log

- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter can_have_styles_applied` passed after deleting the State root pin.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3 --filter can_have_styles_applied` passed.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity
  --dom-decimals 3` passed for all State fixtures.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3` passed for all State fixtures.
- 2026-05-11: `cargo run -p xtask -- report-overrides --check-no-growth` passed with root total
  `759` and State root count `44`.
- 2026-05-11: `cargo clippy -p merman-render --all-targets --all-features -- -D warnings`
  passed.
- 2026-05-11: `cargo test -p xtask override_growth_check_rejects_category_growth` passed.
- 2026-05-11: `cargo nextest run -p merman-render` passed with `148` tests after refreshing the
  two affected State layout golden snapshots.
- 2026-05-11: `cargo test -p merman-render
  state_entity_decode_handles_mermaid_placeholders_and_colon_entity` passed.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter v2_states_can_have_a_class_applied --report-root-all` passed after
  deleting the corresponding State root pin.
- 2026-05-11: `cargo run -p xtask -- compare-state-svgs --check-dom --dom-mode parity-root
  --dom-decimals 3 --filter should_render_a_state_diagram_and_set_the_correct_length_of_t
  --report-root-all` passed after deleting the corresponding State root pin.

## Open Risks

- Root `viewBox` / `max-width` can be affected by browser-only `getBBox()` behavior inside
  `<foreignObject>`.
- Some entries may remain necessary until text measurement or shape bbox logic improves.
- A root table deletion can pass normal DOM parity but fail `parity-root`, so both modes must be
  checked for touched diagram families.
