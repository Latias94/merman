# Documentation Archive Index

This index separates durable history from current guidance without turning documentation cleanup
into a mass path migration. Public migration targets, CE plans, accepted ADRs, and useful
historical evidence remain in the tree. A completed implementation journal may be removed only
when it is explicitly complete, has no current inbound link, and its conclusion already has a
current owner; Git history remains the archive for any such removed journal.

## Current Destinations

| Need | Current destination |
| --- | --- |
| Documentation map | [`README.md`](README.md) |
| Mermaid baseline, family admission, and parity status | [`alignment/STATUS.md`](alignment/STATUS.md) |
| Alignment machine/prose ownership | [`alignment/README.md`](alignment/README.md) |
| Capability and build-profile selection | [`FEATURES.md`](FEATURES.md) |
| Package and channel ownership | [`release/PACKAGE_SURFACES.md`](release/PACKAGE_SURFACES.md) |
| Release operations | [`release/RELEASING.md`](release/RELEASING.md) |
| Presentation themes and output policy | [`rendering/presentation-themes.md`](rendering/presentation-themes.md) |

## Retained Historical Targets

- [`release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md`](release/ALPHA3_TO_ALPHA5_REFACTORING_REPORT.md)
  is a frozen engineering evidence checkpoint. The public migration target remains
  [`release/ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md`](release/ALPHA3_TO_ALPHA5_UPGRADE_GUIDE.md).
- [`plans/`](plans/) retains CE product and implementation decisions. Completed plans are not live
  progress dashboards, but their stable paths remain useful review history.
- [`workstreams/`](workstreams/) retains active lanes and selected completed snapshots. A closed
  workstream's own status or archive note governs how to read terms such as "current."
- [`performance/`](performance/), [`research/`](research/), and
  [`knowledge/engineering/`](knowledge/engineering/) contain dated evidence and engineering
  memory. Current product claims must still resolve to a current authority or machine owner.

## ADR Identity History

Two later ADRs originally reused identifiers already owned by earlier decisions. On 2026-08-11,
the later documents moved to unused identifiers; their content and decision status did not change.

| Historical path | Current path | Identity retained by the earlier ADR |
| --- | --- | --- |
| `docs/adr/0041-dagre-graphlib-dugong.md` | `docs/adr/0080-dagre-graphlib-dugong.md` | `docs/adr/0041-snapshot-parity-tests.md` |
| `docs/adr/0050-release-quality-gates.md` | `docs/adr/0081-release-quality-gates.md` | `docs/adr/0050-svg-viewbox-parity.md` |

Git history is authoritative for deleted paths and pre-rename references. New references should
use the current ADR identities.
