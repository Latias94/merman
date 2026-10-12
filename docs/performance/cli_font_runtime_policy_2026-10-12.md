# CLI Font Runtime Policy Checkpoint

Date: 2026-10-12, Asia/Shanghai. Windows 11, Intel i7-11700 (8 cores / 16
logical processors), Rust 1.95.0, x86_64-pc-windows-msvc. Both executables use
`dist` and the exact descriptor-owned `cli-release` feature recipe. Native and
system font support remain compiled into the candidate; only runtime defaults
change. Diagram text fonts are not bundled.

Ordinary Mermaid graphical output now uses deterministic measurement. Explicit
font files/directories enable native measurement with a closed set. Without such
sources, `--font-measurement native` or `--font-system on` explicitly selects
system discovery. PNG/JPEG/PDF painting retains its own legacy font acquisition;
this checkpoint measures SVG output only.

## Exploratory Complete-Process Observations

| Flowchart input | Previous default, ms | Candidate default, ms | Candidate deterministic, ms | Candidate explicit font, ms |
| --- | ---: | ---: | ---: | ---: |
| Two nodes | 1454.30 | 43.37 | 43.75 | 45.60 |
| 25 nodes | 1470.88 | 45.74 | 46.08 | 52.84 |

Cells are medians of nine fresh-process observations per policy and input,
following one warmup per lane. Lane order rotates inside each executable run;
the previous executable was measured before the candidate, rather than in paired
alternating executable order. Timing includes process startup, input, font
acquisition, SVG generation and stdout capture. Filesystem caches were warm and
were not flushed. Both inputs explicitly request Cousine with HTML labels off;
the explicit lane uses the checked-in test-only Cousine-Regular.ttf.

The candidate default and explicit deterministic lane have identical SVG hashes.
Deterministic and explicit-font SVG hashes are unchanged between executables.
The previous default intentionally uses system fonts and has different layout.
Every observation succeeded and each input/policy has a stable SVG hash.

The result supports removing whole-system discovery from the ordinary SVG path.
It does not establish a universal font overhead, a cold-I/O result, a formal
cross-host speedup, a binary-export cost or a final-head benchmark. The earlier
[90-observation explicit-font report](cli_explicit_font_current_2026-10-10.md)
and [system-discovery report](native_system_discovery_2026-10-10.md) retain their
own sources, profiles and limitations.

## Source Identity And Reproduction

The previous executable was built from `7ba994e22cee89f02f7b1b75b467b96ebfba45f8`.
The candidate is the reviewed working-tree policy change based on that commit;
its relevant source, descriptor, lockfile, binary and font SHA-256 identities are
recorded in [candidate raw observations](evidence/cli-font-policy-2026-10-12/after-default.json).
[Previous raw observations](evidence/cli-font-policy-2026-10-12/before-default.json)
retain all original timings and input/output identities. The artifacts themselves
are local build products and are not committed. These are exploratory receipts,
not a guarantee of reproducing identical timings.

Build from a reviewed checkout and run the [saved probe](evidence/cli-font-policy-2026-10-12/probe.py)
from the repository root:

```powershell
$fontProbeFeatures = python scripts/artifact_profile_recipe.py cli-release
cargo build --locked -p merman-cli --profile dist --no-default-features --features $fontProbeFeatures
python docs/performance/evidence/cli-font-policy-2026-10-12/probe.py target/dist/merman-cli.exe target/font-policy-observations.json
```

The probe stores complete source inputs, raw process timings and SVG hashes.
It does not build executables, change font policy or publish diagram output files.
