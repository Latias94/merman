# Documentation

Use this index to move from a Merman task to the maintained document that owns it. Exact
capability, package, fixture, dependency, and upstream-reference facts live with their structured
owners rather than being duplicated across Markdown files.

## Choose a path

| Task | Start here |
| --- | --- |
| Evaluate Merman or run a first example | [Project overview](../README.md) |
| Choose Rust features, outputs, or artifact profiles | [Capability guide](FEATURES.md) |
| Choose a registry package or delivery channel | [Package surface guide](release/PACKAGE_SURFACES.md) |
| Integrate a browser, Node.js, editor, linter, or Rust host | [Integration guide](integrations/README.md) |
| Check diagram coverage and parity evidence | [Alignment dashboard](alignment/STATUS.md) |
| Embed or export SVG safely | [SVG output pipeline](rendering/SVG_OUTPUT_PIPELINE.md) and [rendering security](security/RENDERING_SECURITY.md) |
| Understand architecture decisions | [Architecture decision records](adr/) and [alignment authority map](alignment/README.md) |
| Contribute or understand CI ownership | [CI guide](development/CI.md) |
| Prepare or operate a release | [Release operator guide](release/RELEASING.md) |
| Align to another Mermaid release | [Mermaid upgrade playbook](release/MERMAID_UPGRADE_PLAYBOOK.md) |

## Documentation model

| Class | Meaning | Representative entries |
| --- | --- | --- |
| Current authority | Defines current architecture, policy, or supported behavior. | [`adr/`](adr/), [`alignment/STATUS.md`](alignment/STATUS.md), [`workstreams/PARITY_BOUNDARY.md`](workstreams/PARITY_BOUNDARY.md) |
| Operator guide | Gives a maintained procedure for contributors or release operators. | [`release/RELEASING.md`](release/RELEASING.md), [`release/MERMAID_UPGRADE_PLAYBOOK.md`](release/MERMAID_UPGRADE_PLAYBOOK.md), [`rendering/UPSTREAM_SVG_BASELINES.md`](rendering/UPSTREAM_SVG_BASELINES.md) |
| Machine input | Is consumed as structured data by code or automation at an owner path. | [`../capabilities/feature-surface-v1.json`](../capabilities/feature-surface-v1.json), [`../capabilities/artifact-profiles-v1.json`](../capabilities/artifact-profiles-v1.json), [`../tools/upstreams/REPOS.lock.json`](../tools/upstreams/REPOS.lock.json) |
| Implementation plan or workstream | Records a scoped design or active implementation effort. Check its explicit status before treating it as current guidance. | [`plans/`](plans/), [`workstreams/`](workstreams/) |
| Historical report | Preserves a dated measurement, release checkpoint, or investigation without claiming to be current guidance. | [`release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md`](release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md), [`performance/`](performance/), [`research/`](research/) |
| Archived history | Preserves completed or superseded context for durable links; removed journals remain available in Git history. | [`ARCHIVE.md`](ARCHIVE.md) |

A directory name alone does not make every file current. Read the document's stated status and use
the current authority or operator guide for decisions. Implementation plans remain useful decision
records after their work is complete, but Git history—not prose rewritten after the fact—records
their execution.

Machine gates may validate structured inputs, executable examples, manifests, fixtures, and
generated projections. They must not depend on ordinary prose wording, Markdown file pairing,
backtick paths, or historical document identifiers.
