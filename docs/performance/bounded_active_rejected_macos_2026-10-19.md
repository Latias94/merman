# Bounded SVG active-state experiment — October 19, 2026

The isolated active-state/cold-error candidate was rejected by its preregistered diagnostic
screen. No production optimization was integrated. Two public end-to-end AB/BA pairs suggest
small improvements, but neither primary row clears both the registered 10% and 1 us screen.
These observations have no A/A calibration and establish no confirmed latency gain.

| Fixture | Baseline ns | Candidate ns | Candidate/baseline movement | Delta ns |
| --- | ---: | ---: | ---: | ---: |
| Packet medium, primary | 10,353 | 10,027.5 | −3.15% | −325.5 |
| Kanban medium, primary | 56,680.5 | 53,857 | −4.98% | −2,823.5 |
| Pie medium, control | 27,765 | 27,350.5 | −1.49% | −414.5 |
| Class tiny, control | 142,810 | 139,255 | −2.49% | −3,555 |

The clean baseline is `f40f1c5f7`; provisional candidate `2ff665431` remains on the isolated
`perf/bounded-active-state-20261019` branch. It adds only `#[inline]` to the state error
discriminator and `#[cold] #[inline(never)]` to failure copying. The existing eighteen output
contract tests, formatting and diff/path checks pass. Source review finds unchanged function
bodies, fragment boundaries, work credit, byte admission, cancellation positions and first-error
semantics. Wider candidate gates were not run after rejection.

Both executables use Rust 1.95.0, locked bench builds, default features plus
`all-diagrams,svg,layout-cytoscape,layout-elk,math`, on Apple M4 Pro/macOS aarch64. The baseline
uses an independent target directory; the candidate reuses the production target. Builds,
discovery and sixteen timed calls run serially. Each call uses thirty Criterion samples,
two seconds of warmup and three seconds of measurement. Each row has one AB and one BA pair;
the table uses the arithmetic mean of its two independent point estimates. All pre/postflight
SVG identities match. The initial anchored-filter invocation failed before building or timing
and is retained separately; the corrected selector selects the original four rows.

Frozen machine code confirms the narrow hypothesis: the candidate removes the successful
path's out-of-line `State::error` calls. The baseline establishes a 224-byte frame and saves four
register pairs before checking the state; the candidate performs direct state checks and calls
cold conversion only for failure. This is explanatory evidence, not an alternative admission
gate. The earlier read-only estimate of five saved register pairs was incorrect.

The ignored ledger, complete reports, frozen executables and assembly review are under
`target/bench/experiments/bounded-active-state-20261019/`. Formal confirmation was not started;
the thresholds and selected rows were not relaxed after observing the result.

## Remaining release-comparison coverage

The all-family goal remains open. The existing release matrix has nineteen exact-SVG rendering
rows and fifty-seven changed-SVG rows, with three preprocessing-only fixtures kept separate.
The older source audit covers only twenty-three cross-family scenes, leaves six families'
specific changes unlocalized and does not reconcile all thirty-four other stress/variant rows.
It also lacks paired raw SVG files; digests alone cannot explain changed output.

The next evidence task captures all seventy-six public rendering outputs from tag and current
production code using the same benchmark request, then verifies every raw file against the
already frozen side-specific preflight identity. Differences must be explained per family and
fixture, without dropping styles, IDs, geometry or other semantic data through normalization.

The 418 new typed-preset cells prove adjacent output/report and memory behavior. v0.9.0 has no
matching preset API, so they do not prove release latency convergence. Historical common legacy
configuration, a strictly proven exported Mermaid subset and complete new preset behavior need
distinct operation contracts. Newly consumed functionality requires its own cost attribution;
it cannot be assumed equivalent to a legacy base/dark theme.
