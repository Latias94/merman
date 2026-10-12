# Current Same-Binary Explicit-Font CLI Probe

This harness measures deterministic layout against active explicit-font layout
using **one executable**, the checked-in Cousine-Regular font and the same
configuration. It is a new current-code experiment, not a reconstruction of the
[historical compiled/inactive/explicit checkpoint](../cli-native-default-2026-10-10/README.md).
No timing results are claimed by this harness alone.

Inputs are the saved user Flowchart, ER and Sequence sources under
`docs/assets/native-font-measurement/`. Their content is unchanged, but this
experiment uses Cousine, **not** the Fira Code visual comparison's font.
`config.json` selects Cousine for general and Sequence labels and disables
HTML labels. Native mode loads only
`crates/merman-export/tests/fonts/Cousine-Regular.ttf`; system discovery is off.
Deterministic mode explicitly opts out of automatic measurement-time discovery.
Mermaid-to-SVG does not rasterize or embed fonts; native mode changes layout
metrics and output geometry, rather than performing an additional painting step.

## Run

Use PowerShell 7 and an executable with `svg,native-fonts,diagram-flowchart,diagram-er,diagram-sequence`.
Run without concurrent builds or other benchmarks. The default run launches
540 measured fresh processes: three families, two lanes, three repetitions,
30 samples per repetition. Each family/lane first runs three warmups (18 extra
processes). Samples alternate AB/BA within each repetition. Families execute
in separate blocks, so cross-family timing differences are not paired controls.
Filesystem caches are not flushed.

```powershell
pwsh -NoProfile -File docs/performance/evidence/cli-explicit-font-current-2026-10-10/measure.ps1 -Cli target/release/merman-cli.exe -OutputPath target/bench/cli-explicit-font-current-samples.json
```

The output parent directory must already exist. `-RepoRoot` is optional; by
default it is inferred from the script's repository location. `-Samples` and
`-Repetitions` default to 30 and 3. Keep the generated JSON with the exact
binary/build identity when publishing a result; source snapshots and build
recipes still need to be recorded by the experiment owner.

## Evidence and Interpretation

Timing includes process startup, input/config acquisition, explicit font
acquisition in native mode, layout/render and stdout/stderr draining. Hashing
stdout occurs after the timer stops. This is **whole CLI active-versus-
deterministic cost**, not isolated measurement/shaping throughput, compiled
feature footprint, or default system-discovery overhead. Native output is
allowed to differ from deterministic output because geometry changes.

The script drains stdout's raw `BaseStream` to memory concurrently with stderr,
rejects nonzero exits and empty output, and requires byte-identical output across
warmups and measured samples for each family/lane. Failure aborts without writing
a successful sample document. It does not require equality between policies.

JSON records executable size/SHA-256, font/config/harness/lockfile/input SHA-256 identities,
each sample's timestamp, exit status, output byte count/hash, family, lane,
repetition, index and elapsed milliseconds. It also records warmup/order/cache
policy and an aggregate summary for each family/lane. Summary median uses the
middle observation or mean of the two middle observations; p95 uses nearest
rank `ceil(0.95 * sample_count)`. Per-repetition values can be independently
recomputed from the rows. Descriptive timings and tails do not establish a
portable performance guarantee or statistical optimization admission.
