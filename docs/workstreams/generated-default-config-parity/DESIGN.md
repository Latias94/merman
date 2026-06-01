# Generated Default Config Parity

Status: Closed
Last updated: 2026-05-31

## Why This Lane Exists

Mermaid 11.15 exposed that `xtask verify-generated` is doing too much under one failure surface.
It verifies generated default config JSON, DOMPurify allowlists, and font metrics in one command.
When an optional reference checkout such as `repo-ref/dompurify` is missing, the command can fail
before the actionable default config drift is visible.

## Relevant Authority

- ADRs:
  - `docs/adr/0019-generated-default-config.md`
  - `docs/adr/0024-dompurify-default-allowlists-and-generation.md`
  - `docs/adr/0041-snapshot-parity-tests.md`
- Existing docs:
  - `docs/rendering/FEARLESS_REFACTORING_SVG_PARITY.md`
  - `docs/rendering/REFACTOR_TODO.md`
- Related workstreams:
  - `docs/workstreams/mermaid-11-15-baseline-upgrade/`

## Problem

The generated artifact verifier has a coarse ownership boundary. Default config parity, DOMPurify
allowlist parity, and render font metrics have different sources, different optional reference
checkout requirements, and different remediation paths.

For default config specifically, the current generator reads schema defaults, while ADR-0019 says
the real source of truth is the schema plus Mermaid `defaultConfig.ts` overlay behavior. That gap
means schema-only regeneration can drift from the committed artifact after 11.15 compatibility
fixes, but the current all-in-one verifier does not make that gap easy to isolate.

## Target State

- `xtask` exposes artifact-specific verification commands:
  - `verify-default-config`
  - `verify-dompurify-defaults`
  - later, if needed, `verify-font-metrics`
- `verify-generated` remains as the umbrella compatibility command and delegates to the specific
  checks.
- The default config generator either models Mermaid's `defaultConfig.ts` overlay semantics or uses
  a small explicit override manifest that is reviewable and parity-tested.
- Docs distinguish required green gates from optional reference-checkout gates.
- The default config artifact can be verified without a DOMPurify checkout.

## In Scope

- Split xtask generated-artifact verification by artifact owner.
- Preserve existing `verify-generated` command behavior as an umbrella command.
- Improve generated default config parity for Mermaid 11.15 defaults.
- Update ADRs and rendering refactor docs so future maintainers know which gate to run.
- Record fresh command evidence for each slice.

## Out Of Scope

- New Mermaid diagram family implementation.
- Full DOMPurify behavior or `dompurifyConfig` parity.
- Changing sanitizer runtime behavior unless a later task explicitly proves it is required.
- Root viewport override generation tooling.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| `repo-ref/dompurify` is optional local reference material, not a runtime dependency. | High | ADR-0024 and current missing-checkout failure. | The DOMPurify verifier may need a bootstrap task instead of optional-gate docs. |
| Default config parity should be verifiable without Node or Vite at runtime. | High | ADR-0019. | We would need a broader architecture decision before changing the generator contract. |
| The 11.15 default config artifact contains intentional overlay values beyond schema defaults. | High | 11.15 workstream closeout and current generator drift. | GDC-030 may shrink to documentation only. |
| `verify-generated` should remain available for existing workflows. | High | Release docs and ADR references use it. | We can replace instead of delegate, but that is a larger compatibility change. |

## Architecture Direction

Generated artifacts should have narrow verification functions with artifact-specific source paths,
temporary output paths, and error messages. The umbrella command should compose those functions
without hiding which artifact failed.

Default config should remain a committed JSON artifact loaded by `merman-core`; runtime code should
not execute JavaScript. Any JavaScript-derived behavior must be captured in deterministic xtask
generation or in a small reviewed override layer with tests.

## Closeout Condition

This lane can close when:

- artifact-specific xtask verification commands exist and are documented,
- `verify-default-config` is green for the Mermaid 11.15 baseline,
- DOMPurify verification has an explicit reference-checkout policy,
- the relevant ADRs name the artifact-specific gates,
- and remaining generated-artifact work is either completed or split into a follow-on.

Closeout status: complete as of GDC-050. The remaining Mermaid 11.15 work is no longer
generated-artifact verification debt; it belongs in follow-on lanes for Pie config behavior,
deferred diagram families, or sanitizer runtime parity.
