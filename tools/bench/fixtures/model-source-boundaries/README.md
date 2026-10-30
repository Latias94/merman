# Model/source boundary consumers

These six fixed M0 consumers freeze consumed behavior at
`a5c73612fb2288e5effdbb07b3e1c3e683968603`. They extend
`tools/bench/measure_diagram_selection.py`; they are artifact/dependency observations, not timing
benchmarks. Commit this fixture before running the base, then retain those receipts before editing
production code. The driver reads committed source and fixture revisions, never dirty fixture bytes.

The original base fixture is pinned at `2e71bfa333b5e9ae6dcee41bdad2be81c162e444`.
The current fixture migrates model imports to `merman-model` while retaining the same runtime inputs
and consumed fields. Use that original fixture revision for the pre-extraction base; use a committed
current fixture for the candidate. The base has no `merman-model` package.

| Recipe | Fixture feature | Consumed output |
| --- | --- | --- |
| `boundary-model` | `model` | Constructed Flowchart/Class/State node identities, labels, edges, direction and State flat-document count. |
| `boundary-model-ascii` | `model-ascii` | All three constructed models rendered as primary ASCII and complete width-12 fallback, including output text and report metadata. |
| `boundary-source-ascii` | `source-ascii` | All three runtime Mermaid sources parsed to canonical models, then rendered as primary ASCII and complete width-12 fallback. |
| `boundary-presentation-svg` | `presentation-svg` | Flowchart source through the current core presentation API, Render preparation and SVG emission with a runtime diagram ID. |
| `boundary-analysis-diagnostics` | `analysis-diagnostics` | One retained Markdown analysis generation, source identity, sorted parser-backed node IDs and diagnostic payload with fix DTOs omitted. |
| `boundary-analysis-fixes` | `analysis-fixes` | The same Markdown capture/projection path with actual YAML frontmatter migration edits and the full original-document text after applying that fix. |

`alpha.json` and `beta.json` provide different runtime identities, labels and edge text. Beta's
Markdown has CRLF and an emoji before the fence to exercise original-document byte/UTF-16 mapping.
The program receives the input file at runtime. These files are fixture inputs, not a new library
model import protocol. Flowchart/Class use existing typed serde; State uses public graph fields.

## Freeze the base

From the repository root, after the parent commits the harness, run these PowerShell commands.
`--features` is omitted because each boundary recipe selects its exact fixture feature. All library
dependencies disable default features. These invocations deliberately reuse the maintainer's shared
target; do not run another Cargo operation concurrently.

```powershell
$fixtureRevision = '2e71bfa333b5e9ae6dcee41bdad2be81c162e444'
$baseRevision = 'a5c73612fb2288e5effdbb07b3e1c3e683968603'
$boundaryRecipes = @(
    'boundary-model',
    'boundary-model-ascii',
    'boundary-source-ascii',
    'boundary-presentation-svg',
    'boundary-analysis-diagnostics',
    'boundary-analysis-fixes'
)
foreach ($recipe in $boundaryRecipes) {
    python tools/bench/measure_diagram_selection.py --recipe $recipe --revision $baseRevision --fixture-revision $fixtureRevision --label "$recipe-base" --output "target/bench/experiments/model-source-boundaries/base/$recipe" --target-dir 'F:/SourceCodes/Rust/merman/target'
    if ($LASTEXITCODE -ne 0) { throw "Boundary baseline failed: $recipe" }
}
```

Keep `$fixtureRevision` pinned to the actual committed harness. Evidence directories must be new.
Omit `--target-dir` to retain the existing runner's isolated `OUTPUT/cargo-target` behavior. The
runner records the resolved target directory and never cleans it. Both existing legacy recipes keep
their previous defaults. The selected Rustup toolchain needs `llvm-tools`, and the locked packages
must already be available locally: builds are offline, release, one job, size optimization, thin
LTO, one codegen unit, abort-on-panic, no debug information and no incremental compilation.

## Compare a committed candidate

After implementation, use the same inputs and harness where APIs permit. If imports or source
adapters must migrate, commit and retain the revised harness as a separate fixture revision; retain
the original base fixture as well. Do not change the consumed operation or input/output contract to
obtain a smaller artifact.

```powershell
$headRevision = (git rev-parse HEAD).Trim()
$headFixtureRevision = (git rev-parse HEAD).Trim()
foreach ($recipe in $boundaryRecipes) {
    python tools/bench/measure_diagram_selection.py --recipe $recipe --revision $headRevision --fixture-revision $headFixtureRevision --label "$recipe-head" --output "target/bench/experiments/model-source-boundaries/head/$recipe" --target-dir 'F:/SourceCodes/Rust/merman/target' --expect-receipt "target/bench/experiments/model-source-boundaries/base/$recipe/receipt.json"
    if ($LASTEXITCODE -ne 0) { throw "Boundary comparison failed: $recipe" }
}
```

Set `$headFixtureRevision` to the committed migrated fixture when needed. `--expect-receipt` checks
the exact corpus input/output hashes and byte counts, selected families, feature, profile, toolchain
and target. A mismatch writes its failed comparison into the candidate receipt and exits with an
error. Different executable sizes or dependency graphs are observations, not output mismatches.

## Outputs and checks

Each evidence directory retains the archived source, independently staged consumer manifest and
lock, fixture inputs, Cargo metadata, normal/build dependency tree, build log, unstripped release
executable, stripped executable and `receipt.json`. Output files are `alpha.observations.json` and
`beta.observations.json`, or `alpha.svg` and `beta.svg` for the presentation recipe. Raw/stripped
executables must emit identical bytes; a second stripped execution must also match exactly.

The JSON envelope has the runtime `identity` (`boundary-alpha` or `boundary-beta`) and nonempty
`observations`. Model observations must contain the corresponding constructed IDs/labels/edges.
ASCII fixtures require both labels and edge text in each primary/fallback output and require an
actual fallback outcome. SVG requires the runtime diagram ID, both labels and edge text. Analysis
requires one valid diagram, authoring diagnostics and both runtime node IDs. The fix consumer
requires the frontmatter migration fix, validates nonoverlapping UTF-8 edit ranges against the
original Markdown and checks that applying it preserves the diagram labels and removes the init
directive. Full spans and replacements remain in the consumed diagnostic payload.

JSON object keys are sorted for deterministic observation. Array order, ASCII text/report values,
SVG bytes, diagnostic spans, replacements and applied-source line endings are preserved. Output
SHA-256 values are obtained from the executable; this fixture does not invent expected runtime
hashes before the parent runs the base. `consumer_families` describes exercised consumers, not a
source-parser capability catalog.

## Coverage boundaries

- BASE still owns models in `merman-core`; its dependency costs remain visible. The direct-model
  recipe calls no Mermaid parser, but does not claim its Cargo graph is already parser-free.
- BASE State has no public flat-document builder. Its directly constructed model has an empty
  document/record store. The source route also exercises the parser-produced document. These are
  separate contracts; direct-model and source-to-ASCII outputs need not equal each other. The
  candidate keeps that same direct graph to preserve comparison; the new checked document builder
  has separate model/source tests.
- BASE `source-ascii` selects `model-ascii` and therefore compiles the fixture's model construction
  code too; it calls only the source route. The head recipe may separate feature/import ownership
  while preserving the source consumer's observations.
- BASE exposes graphical presentation through a closed core parsed result. This freezes the real
  supported source/presentation/SVG route, not an independently authored presentation constructor.
  The M5 source/ownership adapter needs its own retained revision and final independent consumer.
- BASE eagerly generates fixes. The diagnostics consumer clears fix DTOs only after projection;
  it does not claim to avoid the YAML writer or generation work. The fixes consumer consumes an
  actual YAML rewrite. M4 must separately prove Omit/Generate and serializer exclusions.
- This is a bounded three-family graph fixture and one graphical family, not the complete family,
  feature-union, adversarial, full-context terminal, font/native export or security acceptance suite.
  Existing owners remain responsible for those checks.
- Python unit tests prove staging and receipt behavior without compiling Rust. The parent must
  compile/run these consumers serially and freeze successful base receipts before production edits.
  Artifact bytes and dependency closures do not establish cold-compile or runtime speed changes.

The scoped runner tests require no Cargo:

```console
python -B tools/bench/test_diagram_selection.py
```
