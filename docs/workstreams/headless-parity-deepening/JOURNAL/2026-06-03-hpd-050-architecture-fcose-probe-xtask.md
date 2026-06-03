# HPD-050 - Architecture FCoSE Browser Probe xtask Entry

Date: 2026-06-03
Task: HPD-050 Architecture-first layout engine audit

## Context

The Architecture Cytoscape body/label/union contribution seams are now explicit in renderer-side
debug output, but the browser/Cytoscape reference probe was still a raw Node command with manual
fixture spelling and shell redirection. That made source-backed residual audits harder to repeat
and easier to document inconsistently.

## Outcome

- Added `xtask debug-architecture-fcose-probe`.
- The command resolves exactly one Architecture fixture by `--fixture`, invokes the existing
  `tools/debug/arch_fcose_browser_probe_fixture_025.js` script, validates JSON stdout, and writes a
  stable `<fixture>.fcose-browser-probe.json` artifact.
- Added `--out` / `--out-dir` for artifact directory selection and `--browser-exe` for the existing
  Edge/Chrome-backed Puppeteer workflow.
- Added focused xtask unit coverage for argument parsing and artifact naming.
- Kept renderer, layout, measurement constants, and SVG output behavior unchanged.

## Verification

- `cargo fmt --check` - passed.
- `cargo nextest run -p xtask fcose_probe` - passed, `3` tests run.
- `cargo nextest run -p xtask` - passed, `89` tests run.
- `cargo run -p xtask -- debug-architecture-fcose-probe --fixture stress_architecture_batch5_long_titles_and_punct_076 --out-dir target\compare\architecture-fcose-probe-hpd050 --browser-exe 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'` -
  passed and wrote
  `target\compare\architecture-fcose-probe-hpd050\stress_architecture_batch5_long_titles_and_punct_076.fcose-browser-probe.json`
  with `4` captured stages, `5` final nodes, and `4` final edges.
- `cargo run -p xtask -- compare-architecture-svgs --check-dom --dom-mode parity --dom-decimals 3 --out target\compare\architecture_report_parity_hpd050_fcose_probe_xtask.md` -
  passed.
- `git diff --check` - passed.

## Residual Boundary

This is a reference-harness repeatability seam, not an Architecture root residual closure. Future
Cytoscape bbox audits should prefer the xtask entry over raw `node ... > file` commands so the
browser executable, fixture resolution, and artifact path are all visible in command evidence.
