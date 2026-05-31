# Generated Default Config Parity - Milestones

Status: Active
Last updated: 2026-05-31

## M0 - Scope And Evidence Freeze

Exit criteria:

- Problem and target state are explicit.
- Non-goals are explicit.
- Relevant ADRs/docs/workstreams are linked.
- First implementation slice is chosen.

Primary evidence:

- `docs/workstreams/generated-default-config-parity/DESIGN.md`
- `docs/workstreams/generated-default-config-parity/TODO.md`

Status: complete.

## M1 - Artifact-Specific Verification Surface

Exit criteria:

- `verify-default-config` exists and runs independently of DOMPurify source checkout state.
- `verify-dompurify-defaults` exists and owns only DOMPurify allowlist regeneration checks.
- `verify-generated` remains an umbrella command that delegates to the split checks.
- Docs and ADR references name the new commands.

Primary gates:

- `cargo nextest run -p xtask`
- `cargo run -p xtask -- verify-default-config`
- `cargo run -p xtask -- verify-dompurify-defaults`
- `cargo fmt --check`
- `git diff --check`

Status: complete with concern. The split commands exist and are independently runnable, but
`verify-default-config` is red until GDC-030 models the Mermaid defaultConfig overlay.

## M2 - Default Config Overlay Parity

Exit criteria:

- `verify-default-config` is green for the Mermaid 11.15 baseline.
- The generator contract explains schema defaults versus Mermaid `defaultConfig.ts` overlay defaults.
- Any explicit override layer is reviewable, deterministic, and covered by Rust tests.

Primary gates:

- `cargo run -p xtask -- verify-default-config`
- `cargo nextest run -p merman-core config`
- `cargo nextest run -p merman-render`
- `cargo fmt --check`

Status: complete. `verify-default-config` is green through an explicit override manifest applied by
`gen-default-config`.

## M3 - DOMPurify Source Policy

Exit criteria:

- DOMPurify verification has clear remediation when `repo-ref/dompurify` is absent.
- ADR-0024 matches the current Mermaid baseline and xtask command names.
- The team has decided whether DOMPurify remains in `verify-generated` or moves to an optional
  bootstrap gate.

Primary gates:

- `cargo run -p xtask -- verify-dompurify-defaults`
- `cargo fmt --check`

## M4 - Closeout

Exit criteria:

- Gate set is recorded.
- Remaining work is completed, deferred, or split into a follow-on.
- `WORKSTREAM.json` status is updated.
