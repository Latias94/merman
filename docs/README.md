# Documentation

This directory is the entry point for Merman's maintained documentation. Markdown explains the
project's contracts and history; it is not a second machine database. Exact capability, package,
fixture, dependency, and upstream-reference facts live with their structured owners.

## Lifecycle Classes

| Class | Purpose | Representative entries |
| --- | --- | --- |
| Current authority | Defines current architecture, policy, or supported behavior. | [`adr/`](adr/), [`alignment/STATUS.md`](alignment/STATUS.md), [`workstreams/PARITY_BOUNDARY.md`](workstreams/PARITY_BOUNDARY.md) |
| Operator guide | Gives a maintained procedure for contributors or release operators. | [`release/RELEASING.md`](release/RELEASING.md), [`release/MERMAID_UPGRADE_PLAYBOOK.md`](release/MERMAID_UPGRADE_PLAYBOOK.md), [`rendering/UPSTREAM_SVG_BASELINES.md`](rendering/UPSTREAM_SVG_BASELINES.md) |
| Machine input | Is consumed as structured data by code or automation at an owner path. | [`../capabilities/feature-surface-v1.json`](../capabilities/feature-surface-v1.json), [`../capabilities/artifact-profiles-v1.json`](../capabilities/artifact-profiles-v1.json), [`../tools/upstreams/REPOS.lock.json`](../tools/upstreams/REPOS.lock.json) |
| Active workstream | Records implementation work that has an explicit active owner and status. | [`plans/2026-08-11-001-refactor-verification-ci-and-release-ownership-plan.md`](plans/2026-08-11-001-refactor-verification-ci-and-release-ownership-plan.md), [`plans/2026-08-02-001-refactor-presentation-theme-architecture-plan.md`](plans/2026-08-02-001-refactor-presentation-theme-architecture-plan.md) |
| Historical report | Preserves a dated measurement, release checkpoint, or investigation without claiming to be current guidance. | [`release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md`](release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md), [`performance/`](performance/), [`research/`](research/) |
| Archived history | Preserves completed or superseded context for durable links; removed journals remain available in Git history. | [`ARCHIVE.md`](ARCHIVE.md), [`plans/`](plans/), [`workstreams/`](workstreams/) |

A directory name alone does not make every file current. Use the file's explicit status and the
indexes above. CE plans remain in the repository as decision artifacts even after implementation;
progress is recorded in Git rather than by rewriting those plans.

## Start Here

For users and integrators:

- [Project overview and examples](../README.md)
- [Capabilities and artifact profiles](FEATURES.md)
- [Diagram alignment and parity status](alignment/STATUS.md)
- [Package and delivery surfaces](release/PACKAGE_SURFACES.md)
- [Integrations and editor workflows](integrations/README.md)
- [Rendering security](security/RENDERING_SECURITY.md)

For maintainers:

- [Alignment authority map](alignment/README.md)
- [Release operator guide](release/RELEASING.md)
- [Mermaid upgrade playbook](release/MERMAID_UPGRADE_PLAYBOOK.md)
- [Documentation archive index](ARCHIVE.md)

Machine gates may validate structured inputs, executable examples, manifests, fixtures, and
generated projections. They must not depend on ordinary prose wording, Markdown file pairing,
backtick paths, or historical document identifiers.
