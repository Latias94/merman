# Render Parser Registry - Milestones

Status: Complete
Last updated: 2026-05-28

## M0 - Scope Freeze

Status: Done

Exit criteria:

- Registry extraction scope is explicit.
- Non-goals are recorded.

## M1 - Registry Extraction

Status: Done

Exit criteria:

- `RenderDiagramRegistry` owns typed render parser lookup.
- `Engine::parse_render_semantic_model` no longer contains the diagram parser match.
- Focused tests cover alias lookup and JSON fallback.

## M2 - Verification And Closeout

Status: Done

Exit criteria:

- Fresh gates pass.
- Evidence is recorded.
- Follow-on generation decision is explicit.
