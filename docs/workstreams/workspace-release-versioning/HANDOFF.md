# Workspace Release Versioning — Handoff

Status: Closed
Last updated: 2026-05-30

## Current State

This lane is closed. The binding ABI work is not the blocker; release packaging now has a concrete
`0.7.0` baseline and publish order.

Confirmed:

- `docs/release/PUBLISH_ORDER.md` selects `0.7.0` as the next release target.
- Workspace package version and internal dependency requirements are aligned to `0.7.0`.
- `docs/release/PUBLISH_ORDER.md` records the package gate matrix.
- `dugong-graphlib`, `manatee`, and `merman-core` fully package-verify at `0.7.0`.
- Downstream full package verification is blocked until upstream `0.7.0` crates are published to
  crates.io in order.

## Remaining Work

No remaining task belongs to this lane. The release operator should follow
`docs/release/PUBLISH_ORDER.md` and publish crates in dependency order. Platform packaging lanes can
start from the `0.7.0` baseline.

## Guardrails

- Do not weaken FFI or UniFFI feature gates to force package verification.
- Do not touch ASCII work.
- Do not publish crates from Codex without explicit user instruction.
- Keep platform packaging lanes separate.
