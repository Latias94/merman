# HPD-080 - Binding SVG Fallback Options

Date: 2026-06-02
Task: HPD-080 visible rendering defect triage

## Trigger

The Zed PR 57967 audit showed that fallback labels are part of the public integration contract for
hosts that consume `SvgPipeline::resvg_safe()`. Rust users can already compose
`DropNativeDuplicateFallbacksPostprocessor`, but the shared `options_json` surface used by Web,
WASM, C ABI, UniFFI, Android, Apple, Flutter, and Python only exposed `svg.pipeline`.

That made non-Rust consumers choose between default `resvg-safe` output and private downstream
postprocessing for duplicate native/fallback labels.

## Change

- Added `svg.drop_native_duplicate_fallbacks` to the shared binding options JSON.
- The option defaults to `false`, preserving the existing `parity`, `readable`, and `resvg-safe`
  preset contracts.
- When enabled, bindings append `DropNativeDuplicateFallbacksPostprocessor` to the selected SVG
  pipeline. The pass removes only fallback groups whose text duplicates native SVG `<text>` and
  preserves fallback-only labels.
- Updated `@merman/web` TypeScript options so browser consumers can pass the same flag without raw
  JSON strings.
- Documented the field in `docs/bindings/OPTIONS_JSON.md`.

## Validation Plan

- Binding unit coverage parses the shared JSON option, verifies default `resvg-safe` keeps both
  fallback groups, and verifies the opt-in pipeline drops only the native duplicate.
- TypeScript build should refresh `platforms/web/dist/index.d.ts`.

## Residual

This closes the generic duplicate-fallback toggle for binding consumers. It does not add host
palette replacement, root background replacement, scoped CSS injection, or `!important` stripping to
the JSON surface; those need a more explicit security/cascade design before becoming binding
options.
