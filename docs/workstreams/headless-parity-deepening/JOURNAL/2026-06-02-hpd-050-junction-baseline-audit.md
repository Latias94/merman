# HPD-050 - Junction Fork/Join Baseline Audit

Date: 2026-06-02

## Context

After the Architecture edge-label bounds fix, `stress_architecture_junction_fork_join_026` became
the largest remaining Architecture root residual at `+13.976px`. Earlier M15RV-089 work had
already aligned the source-backed inputs for this fixture: junction parents come only from
`junction.in`, and Mermaid's duplicate queued-position BFS behavior is preserved for relative
constraints.

## Evidence

- `cargo run -p xtask -- compare-architecture-svgs --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3 --report-root-all --out target/compare/architecture_junction_fork_join_hpd050_debug.md`
- Local debug still reports 9 relative constraints, including duplicate `join -> db` and
  `join -> cache` rows.
- Comparing the saved Mermaid browser probe
  `target/compare/arch_junction_fork_join_probe_m15rv089.json` against the current local SVG shows
  service-position deltas at floating-point noise level.
- Comparing the same probe against the stored upstream SVG shows concrete drift: `auth`/`cache` X
  differ by about `10.376px`, `ingress`/`db` X by about `6.988px`, and `api`/`db` Y by about
  `12.358px`.

## Superseding Check

A follow-up `check-upstream-svgs` run used Edge as `PUPPETEER_EXECUTABLE_PATH` because the default
Puppeteer Chrome was not installed locally:

- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram architecture --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity-root --dom-decimals 3`
- `$env:PUPPETEER_EXECUTABLE_PATH='C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'; cargo run -p xtask -- check-upstream-svgs --diagram architecture --filter stress_architecture_junction_fork_join_026 --check-dom --dom-mode parity --dom-decimals 3`

Both checks passed. The generated SVG in
`target/upstream-svgs-check/architecture/stress_architecture_junction_fork_join_026.svg` matches
the stored fixture's root facts exactly:

- `max-width: 2808.126708984375px`
- `viewBox="-1362.063232421875 -1213.2674560546875 2808.126708984375 2557.534912109375"`

The debug probe still matches local service positions exactly, but differs from the fixture/CLI
baseline by offsets such as `auth.x=+10.376px`, `cache.x=+10.376px`, `api.y=-12.358px`, and
`db.y=-12.358px`.

## Outcome

No renderer or manatee change was made. The earlier "stored upstream baseline drift" reading is
superseded: the stored fixture is reproducible by the current CLI/Edge upstream path. The remaining
`junction_fork_join_026` root tail should be treated as a debug-probe harness / CLI-harness
divergence plus solver/phase residual candidate. Do not tune manatee against the saved debug probe
alone.
