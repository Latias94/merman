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

## Outcome

No renderer or manatee change was made. The remaining `junction_fork_join_026` root tail should be
treated as a generated-baseline / seed-lattice audit candidate until a fresh reproducible Mermaid
11.15 browser render proves the stored upstream SVG is still authoritative. Do not tune manatee
against this stored SVG row while the saved browser probe and local output already agree.
