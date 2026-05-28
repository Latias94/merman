# Resvg-Safe SVG Output Pipeline - Milestones

Status: Active
Last updated: 2026-05-28

## M0 - Scope Freeze

Status: Done

Exit criteria:

- ADR records the public output-pipeline decision.
- Workstream docs define scope, non-goals, and Zed PR evidence.

## M1 - Readable Fallback Correctness

Status: Done

Exit criteria:

- Literal `\n` inside `<foreignObject>` labels becomes separate fallback text lines.
- Focused fallback tests pass.

## M2 - Pipeline Skeleton

Status: Pending

Exit criteria:

- `SvgPipeline` presets exist.
- Default parity rendering is unchanged.
- Existing readable helper can be represented as a pipeline preset.

## M3 - Resvg-Safe Built-ins

Status: Pending

Exit criteria:

- Built-in cleanup covers generic `usvg` / `resvg` hazards observed in downstream integration.
- Regression tests cover unsupported CSS and invalid visual attributes.

## M4 - Host Extension API

Status: Pending

Exit criteria:

- Host applications can append custom SVG postprocessors.
- Ordering and error handling are covered by tests.

## M5 - Verification And Closeout

Status: Pending

Exit criteria:

- Package gates pass.
- Documentation and changelog reflect shipped behavior.
- Remaining product-specific theming work is explicitly deferred or split.
