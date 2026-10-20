---
type: "Session Handoff"
title: "Final host-theme repairs and v0.9.2 candidate"
description: "Final local verification and release-readiness evidence for host-theme repairs on main and v0.9.2 maintenance."
timestamp: 2026-10-20T21:17:50Z
record_id: "3b0f86f73122423e907971e9a3fd5c88"
producer_id: "codex-windows"
run_id: "host-theme-228-232-20261020-final"
related_plan: "docs/plans/2026-10-20-2244-fix-host-theme-backports-plan.md"
git_branch: "fix/host-theme-228-232-20261020"
git_commit: "d5097b8d0"
verified_by: "main and maintenance owner gates, cargo-deny, legal projections, independent review"
supersedes: "4efef5ba500f4745a352e3a45971d78c"
---

# Summary

The #228–#232 host-theme repairs are complete on main and on the `v0.9.1` maintenance line.
Main implementation ends at `1f01c31b7`; the maintenance implementation ends at `31a8add27`.
The maintenance candidate metadata/security commit is `7d13eff7d`; the main lock/legal security
refresh is `d5097b8d0`. No tag, push, registry write, or external message was performed.

# Verified State

- Main issue delta, owner suites, bindings, and current-head native outputs are green. Main full
  SVG structure/parity/parity-root receipts are `main-svg-structure-final2.log` and
  `main-svg-parity-final.log`; all-target warnings-denied Clippy and the 45-case feature matrix pass.
- Maintenance core/render/Presentation owner coverage is 3,443/3,443; bindings are 220/220; font
  coverage is 73 passing with nine documented platform skips; final all-target Clippy is green.
  Feature semver checks for core, render, and export each pass 223 checks against `v0.9.1`.
- Maintenance full SVG structure and parity/parity-root receipts pass after installing the pinned
  196-package Node reference environment, including all six math fixtures. The feature matrix is
  42/42 builds plus 16 isolated consumers. CLI assets and the Playground catalog regenerate and
  verify at 0.9.2.
- Both branches lock `rustls-webpki 0.103.16` and `webpki-root-certs 1.0.10`; cargo-deny advisories,
  bans, licenses, and sources pass. Full workspace all-feature cargo checks pass after the update.
  Legal reports and projections are regenerated from the exact locks.
- The maintenance Presentation API, Options JSON/FFI versions, Mermaid 12.1 pin, and old opaque
  `MermaidConfig` public contract remain intact. Three fresh public-consumer lanes compile and render
  an EditorDark Sequence SVG successfully.
- Current-head native probes produce 54 SVG/PNG/PDF artifacts per branch, with 18 inspected triples
  and one-page PDFs. Linux Fontconfig process integration remains the only environment-specific gap;
  deterministic symbol classification and the executable Linux owner test are present.

# Open Threads

- Immutable remote release preflight, tag creation, publication, and registry verification remain
  future maintainer operations. No local implementation gate is pending.
- The report deliberately records main's existing math-fixture deferral and the Windows-only
  Fontconfig gap; neither is presented as a green result.

# Next Action

Review `docs/release/V092_BACKPORT_READINESS.md`, then run the maintainer-owned immutable preflight
from the exact final commit when publication is authorized. Do not move a tag or publish from this
local preparation without that authorization.

# Citations

- `docs/plans/2026-10-20-2244-fix-host-theme-backports-plan.md`
- `docs/release/V092_BACKPORT_READINESS.md`
- `target/diagnostics/issues-228-232-20261020/` (ignored local receipts)
- `7d13eff7d` maintenance candidate metadata/security assets
- `d5097b8d0` main certificate-root lock/legal refresh
