# Historical Explicit-Font CLI Evidence

These records preserve the October 10, 2026 exploratory checkpoint reported in
[Native Font Measurement Optimization Checkpoint](../../native_font_measurement_2026-10-10.md).
They were moved from ignored `target/bench/experiments/cli-native-default-2026-10-10/`.
No new timing run was performed during evidence archival.

- [experiment.yaml](experiment.yaml) is the original experiment ledger, including
  baseline/candidate executable, lockfile and Cousine-Regular font SHA-256 identities.
- [process-latency.json](process-latency.json) retains the original data values and
  ordering: two inputs, three lanes, 30 samples per lane (180 observations),
  input/output hashes and paired median deltas. Its original CRLF line endings
  were normalized to LF by the patch writer; the JSON data are unchanged.

Lane A excludes the compiled native-font capability. Lane B compiles it but does
not activate a font context. Lane C uses the same candidate executable as B with
explicit Cousine-Regular and system discovery off. This historical CLI did not
automatically acquire system fonts. The later default-system-discovery benchmark
compares two already-native implementations and cannot estimate the overhead of
native versus deterministic measurement.

The reported process costs include startup, explicit acquisition where active,
layout/render and output capture. They are not isolated font-measurement costs.
A/B output hashes match for each input; C intentionally changes geometry.

## Recomputed Tail Values

Nearest-rank p95 is the 29th value after sorting each 30-sample lane. No original
statistics, samples or experiment metadata were rewritten to add these values.

| Input | A absent, ms | B inactive, ms | C explicit, ms |
| --- | ---: | ---: | ---: |
| small | 50.4682 | 48.8466 | 55.4298 |
| medium | 49.2087 | 50.0502 | 57.6930 |

## Reconstruction Limits

The original timing script and exact source inputs were not preserved. Input
hashes establish identity but cannot recover their bytes. The ledger describes
stdin/stdout, SVG output and a fixed SVG ID without retaining its complete
command line. Executable hashes identify the historical binaries but do not
recover the uncommitted source snapshots or their complete build inputs.
No replacement historical script or exact-reproduction command is invented here.

The JSON records one successful-output digest per lane; it does not retain every
process's exit status, stdout, stderr or timestamp. Success and within-lane output
stability rely on the retained ledger and original report. Three warmups and
alternating ABC/CBA order are documented metadata, not independently verifiable
from the grouped timing arrays. There is one run per input on one Windows host,
with warm filesystem caches and a single explicit font; tail values are
descriptive and do not establish portable overhead or statistical admission.
