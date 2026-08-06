# Presentation Theme Branch Audit

Date: 2026-08-06

Branch: `refactor/presentation-theme-model`

Plan: `docs/plans/2026-08-06-001-refactor-portable-diagram-theme-architecture-plan.md`

This audit classifies the uncommitted work that existed when the portable diagram-theme plan
superseded the provisional presentation-theme design. It is an implementation input, not a public
API contract or a progress log.

## Retained Independently

| Commit | Scope | Reason |
| --- | --- | --- |
| `59f29a0d1` | Scoped-CSS root selector containment | Correctness and isolation fix independent of the new theme model. |
| `96d131d5f` | Request-scoped site-config materialization | Preserves per-operation configuration ownership and cache invalidation independent of theme APIs. |

## Superseded Working Changes

| Paths | Disposition |
| --- | --- |
| `crates/merman-render/src/presentation/mod.rs`, `profile.rs` | Do not preserve the provisional public split. Migrate `look = neo`, the Flowchart ELK default, and the private Flowchart render policy into behavior-only `RenderProfile`; delete the conditional Redux/slate visual fallback and shallow resolved wrappers. |
| `crates/merman/src/svg/mod.rs` | The request-scoped site-config fix is already committed. Replace the remaining direct `HostTheme` facade, presentation names, fallback detection, and hidden-public policy plumbing through U8 rather than committing the interim API. |
| `crates/merman-bindings-core/src/render.rs`, `render/request.rs`, `crates/merman-cli/src/config.rs` | Preserve request-level site-config semantics, but replace the provisional `presentation.profile` / `presentation.theme` handling with the closed theme-first schema in U9. |
| `crates/merman-render/tests/presentation_test.rs`, `crates/merman/tests/presentation_layering.rs`, `presentation_theme_coverage.rs` | Retain default parity, builder-order independence, source precedence, profile behavior, and editor-token coverage as migration evidence. Delete assertions that make the conditional visual fallback or old names contractual. |
| `crates/merman/examples/custom_presentation_theme.rs`, `presentation_profile.rs` | Rewrite against `ThemeTokens` / `DiagramTheme` and behavior-only `RenderProfile`; do not publish the interim API. |
| `CHANGELOG.md`, ADR-0077, presentation rendering docs, Options JSON docs, alpha.4 guide, and Zed audit wording | Rewrite in U10 so no current-facing document describes raw composition or the conditional visual fallback as the final architecture. Preserve only historical context explicitly marked as superseded. |

## Rejected Interim Mechanisms

- `site_config_selects_mermaid_theme` and its non-empty-theme heuristic. It exists only to decide
  whether Redux/slate fallback colors should appear and becomes unnecessary when that fallback is
  deleted.
- Direct public `HostTheme` ownership as a complete visual theme. Its current CSS-value validation
  and Mermaid-variable projection cannot express or prove the typography, assets, canvas, effects,
  bounds, and target-specific portability required by the new contract.
- Hidden-public `ResolvedPresentation` and `PresentationRenderPolicy` as stable facade concepts.
  Behavior policy may remain private, but public ownership moves to `RenderProfile` and compiled
  diagram-theme types.
- Tests or documentation that infer a final theme contract from conditional Redux/slate defaults.

## Preservation Rules

- Do not restore, reset, stash, or discard the superseded working changes. Rewrite them in place as
  their owning implementation units land.
- Stage only coherent files or hunks for each implementation commit.
- Keep default no-theme rendering byte-compatible unless a separately evidenced Mermaid parity fix
  requires a change.
