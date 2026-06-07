# HPD-050 - ClassDB Snapshot Gate Follow-up

Task: HPD-050 release-boundary evidence alignment

## Context

The earlier ClassDB member parser cleanup replaced the local method regex with a source-shaped
scanner. That scanner preserves the source-authored space after a visibility marker, matching the
input in `fixtures/zed_issues/zed_50558_class_inheritance.mmd`:

```mermaid
+ move() void
+ eat() void
+ bark() void
```

The core snapshot gate still expected compact display text such as `+move() : void`, so
`merman-core --test snapshots` reported a stale golden.

## Changes

- Refreshed `fixtures/zed_issues/zed_50558_class_inheritance.golden.json` with
  `cargo +1.95 run -p xtask -- update-snapshots --diagram all --filter zed_50558_class_inheritance`.
- Preserved method ids and display text with a leading source space after the `+` marker:
  ` move`, ` eat`, ` bark`, and `+ move() : void` style display text.
- Left production parser, layout, render, sanitizer, config, and root-viewport code unchanged.

## Verification

- `cargo +1.95 run -p xtask -- update-snapshots --diagram all --filter zed_50558_class_inheritance` -
  passed and changed only the ClassDB zed issue golden.
- `cargo +1.95 nextest run -p merman-core --test snapshots` - passed, `1` test run.
- `cargo +1.95 nextest run -p merman-core class` - passed, `49` tests run.
- `git diff --check` - passed.
- `docs/workstreams/headless-parity-deepening/CONTEXT.jsonl` JSONL parse check - passed, `858`
  lines parsed.

## Boundary

This is a snapshot evidence alignment follow-up. It does not change Class parser behavior, layout,
renderer SVG output, namespace semantics, common sanitizer policy, Gantt date parsing, retained
config projection, SVG baselines, root viewport formulas, or Architecture residual classification.
