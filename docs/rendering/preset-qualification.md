# Scoped preset qualification

## Current candidate status

`run_preset_qualification` executes the exact Brutalist, Spotless and Cyberpunk catalog recipes
under their declared native profiles. Cyberpunk uses the complete-scene profile below; Brutalist
and Spotless retain their existing bounded checks. Earlier archive records remain tied to their
recorded source, recipe and profile. Public catalog cells remain empty for every preset; only a
fresh execution can issue an artifact-local qualification projection.

`inspect_preset_admission(Cyberpunk)` instead executes the three unchanged complete scenes in
`crates/merman-theme-fixtures/fixtures/public-cyberpunk`: Flowchart, Sequence and XY Chart.
This inventory uses native SVG labels, system fonts, default resource limits and 1x PNG. It
checks current artifact identity and reports admission/residuals without issuing qualification.
The other nine presets retain the small Flowchart/State/Sequence admission sources.
This inventory does not exercise HTML labels, PDF, controlled caller fonts or installed consumers.
The separate qualification runner adds the semantic and pixel checks described below.

The shared native profile explicitly sets `htmlLabels: false`, `flowchart.look: classic`,
`flowchart.layout: dagre`, and `sequence.look: classic`. These family settings preserve the original
qualification conditions after Mermaid 12 changed the defaults to Neo and ELK. The fixed subgraph
pixel checks exercise the vertical Dagre connection, its arrowhead, and the separate label
background. Other families retain their current defaults. These are runner conditions; public
preset recipes do not force a look or layout.

The profile does not qualify the default Neo look for Flowchart or Sequence. In particular,
Sequence Neo can combine preset-owned filters with a built-in filter on the loop label box. That
mixed filter tree is outside the current native typed-filter receipt and remains rejected; choosing
classic for this profile does not upgrade its admission. A separate explicit Neo PNG regression
checks that Spotless note fill survives the built-in scoped filter.

The Rust runner emits its exact `render_config`; CLI replay consumes that configuration rather
than maintaining a separate copy. Earlier records still require fresh execution.

## Brutalist and Spotless profile

The workspace-only Rust runner executes the exact Brutalist and Spotless catalog
recipes on the declared `native-flowchart-state-sequence-system-fonts-v1` profile for
Flowchart/State/Sequence SVG and PNG. It checks terminal styles, raster coverage and label ink,
admission reasons, residuals, and artifact/resource identity.
State PNG uses 1x; Flowchart and Sequence use 4x so thin lines and label paint remain observable.
Flowchart places its labeled edge inside a contrasting subgraph: missing or incorrectly faded
label paint must fail the raster check. Its opaque typed background owns its alpha, so the current
qualifier requires opacity 1 and authored RGB pixels without Mermaid's legacy 0.5 fade.
The first-release qualification schema is 1. Unpublished earlier receipts cannot acquire
Flowchart cells: source, profile, and complete replayed record identities must match. Each observation records its
scenario's `png_scale`; SVG remains in diagram units.
The profile requires working system fonts and reports **HostDependent**. It does not bundle a font,
inject rules, or claim portable output. See [coverage](diagram-theme-coverage.md) for other families.

## Cyberpunk complete-scene profile

`native-cyberpunk-full-scenes-system-fonts-v1` qualifies only the three unchanged public Cyberpunk
fixtures: Flowchart, Sequence and XY Chart, each as SVG and 1x PNG. It uses native SVG labels,
system fonts, classic Flowchart and Sequence looks, Dagre for Flowchart, the original catalog
recipe and default resource limits. Successful observations remain **HostDependent**. State is not
part of this profile.

The existing sealed SVG observer checks canvas paint, gradient/grid geometry and compositing;
family-specific surfaces, borders, typography, series order, markers and label ownership; and
the bound glow primitives, colors and composition chain. Internal filter-result names may change
when their connections remain equivalent. These are fixed writer/scenario checks, not a general
CSS cascade or arbitrary-source validator.

The runner also checks actual PNG contributions by independently removing 34 labels, 62 effects,
five arrowheads and nine canvas layers across the scenes. Glyph checks disable all effects first
so glow cannot replace missing text ink. Every removal must change pixels without changing output
dimensions; the mutation export path must first reproduce the public facade PNG exactly. Recipe
export/import regressions separately require identical SVG and PNG bytes.

This profile replaces Cyberpunk's obsolete palette-only qualification. It does not qualify HTML
labels, PDF, other font environments, exhaustive clipping containment or Mermaid reference-image
equivalence. Browser and PDF probes remain separate evidence; see the
[scene verification record](../knowledge/engineering/verification/2026-09-19-cyberpunk-controlled-scenes.md).

## Record a candidate

Use a clean checkout, including untracked files, with the project's Rust toolchain and Python 3.11
or later. The script does not clean or modify source files. Its Cargo invocation uses the existing
target directory, two build jobs, Release, and the acceptance crate's explicit `png,layout-cytoscape` feature profile.

```console
python3 scripts/qualify_theme_presets.py --output target/preset-qualification.json
python3 scripts/qualify_theme_presets.py --check target/preset-qualification.json
```

Add `--cli path/to/merman-cli` to both commands to qualify an existing production CLI on this
host. The checker sends each Rust-owned source through `render --theme-preset`, uses the declared
PNG scale and `resvg-safe` SVG pipeline, and requires exact output bytes. It reuses the bounded
release-process runner; it does not interpret SVG/CSS or reconstruct renderer receipts. A missing
observation, changed source/output, or executable replacement rejects qualification.

The schema-1 record contains:

- The clean source commit and lockfile digest.
- The actual executable digest identified by Cargo's artifact output, build command, Rust toolchain,
  and host identity. The source commit and binary bind compiler, renderer, writer, target-receipt
  encoding, and qualification code together without maintaining another source dependency graph.
- Each preset's qualification profile/schema and exact recipe/resource fingerprints.
- Each declared source text/digest, family, target, admission status/reasons, font source, residual counts, and
  document/artifact/target-evidence/target-receipt digests produced by the Rust runner.
- When `--cli` is supplied, its executable digest, render configuration, SVG pipeline, and count
  of outputs that matched the qualified bytes. `cli: null` makes the absence of this check explicit.

`--check` rebuilds and reruns the same production qualification before comparing the full record.
A changed commit, executable, toolchain, host, font-dependent artifact, or nested receipt rejects
replay. Source, lockfile, and executable identity are also checked across each execution. Run this
in a dedicated checkout without concurrent source edits. The record has no signature and is not
accepted as authority by deserializing it: a successful check requires a fresh execution.

A record is specific to the checkout/build path and host where it was made; debug paths or linker
metadata may change the binary digest in another checkout. Generate that candidate's own record
rather than weakening the comparison. Store records as CI artifacts or under ignored `target/`,
not as self-updating checked-in source fixtures.

## Release preflight

The `x86_64-unknown-linux-gnu` row of `cli-and-lsp-archives` in Release Preflight qualifies
the CLI extracted from its verified cargo-dist archive. Its Ubuntu host installs DejaVu
system fonts for the native profile. The ARM64 row executes normal CLI/LSP archive checks
without claiming the x86_64 qualification profile. Archive
checksum, tracked package contents, runtime capabilities, and normal CLI smoke checks must pass
before `--preset-qualification-output target/preset-qualification.json` runs qualification. The
record additionally binds `cli_archive.sha256`, `target`, and `version` to that exact archive.

A second archive verification uses `--preset-qualification-check` to rebuild and reexecute the
Rust and extracted-CLI paths and compare the complete record. Both flags require `--execute` and
the existing host-target check. Use these archive flags to replay an archive record; standalone
`--check --cli` deliberately cannot reproduce its archive provenance. The job archives the result
as `preset-qualification-<source_sha>-x86_64-unknown-linux-gnu`, including both the execution
record and the archive-bound catalog companion. Generation, replay, and missing-file upload
failures fail the job. Qualification does not edit the source catalog or embed a binary's
own digest into that binary.

This job owns a native Linux host observation. It does not qualify Web, Flutter, another host font
environment, or every shipped artifact profile. Those consumers still use final target admission.

## Final release archive

The formal `Release` workflow also qualifies the final `x86_64-unknown-linux-gnu` CLI archive in
`verify-release-archives-native`, after central bundle verification. It checks out the pinned
release source, installs the same DejaVu system-font dependency, and runs the existing archive
verifier with `--execute` plus qualification output and replay. Downloaded assets and the record
stay under ignored `target/` so the qualification runner can enforce a clean source tree.

The job uploads `preset-qualification-<source_sha>-x86_64-unknown-linux-gnu` only after replay
succeeds. A qualification or upload failure fails the native verification job and blocks the
existing release gate, attestation, registry candidates, and GitHub Release creation. The record
stays in the workflow artifact. The release verification gate then joins the public companion to
an immutable `publication-release-assets` bundle as
`merman-cli-x86_64-unknown-linux-gnu.preset-catalog.json`. It validates the source, target, version
and archive digest against the already verified archive, checks the companion against the native
record projection, and adds its digest and size to `release-verification.json`. Archive bytes and
installer checksums remain unchanged. Other native rows retain their product smoke checks without
claiming this profile.

Registry candidates, attestation and GitHub Release publication download that final bundle and
run `verify-bundle --require-preset-catalog`. Removing both the companion file and its manifest
entry still fails this final-publication gate. The public companion is included in the attested
and published asset set; the private execution record is excluded. This workflow wiring does
not mean a candidate has already run or that any current release has published these new assets.

## Remaining qualification boundary

These records supply clean-build provenance and executable freshness checks for the scoped runner.
They do not freeze C7a or promote public catalog entries. Each `qualified_cells` entry now carries
`family_id`, `output_id`, `profile_id`, and `admission_status`. The profile names the tested scenario
and resource conditions; admission is an open ID such as `portable` or `host_dependent`. Unknown
profiles or admission classes must not be interpreted as portable support. A family/output may have
multiple profiles, ordered by `(family_id, output_id, profile_id)`; conflicting statuses under the
same profile are invalid. Rust, Web, and Flutter expose these conditions; other transports preserve the shared JSON.
This unpublished alpha correction stays within theme catalog schema 1. Rust and Dart cell
constructors now require the profile and admission arguments; there is no implicit default.

Constructing a Rust cell is only constructing metadata, not issuing proof. Built-in scopes remain
empty. The archive companion below projects fresh evidence only for its declared profile and actual
artifact build. Flowchart now reports HostDependent without bridge residuals for the fixed
native-candidate admission sources; qualification additionally checks the declared title, subgraph,
node, and labeled-edge scenario. Sequence retains unsupported
generic-text requests only where completed role writers do not cover their fills, or other
requested facets remain unsupported. The seven retained compatibility recipes and the fixed 18-cell C6a ledger keep
their existing dispositions. Each positive profile covers only its declared scenarios (six artifact
cells per native preset). For Brutalist and Spotless, Flowchart checks canvas, node/cluster surfaces and borders, diagram/cluster/node/edge labels,
the directed edge and marker, and the actual authored label background. PNG checks use
separate regions so borders or the edge cannot replace missing label ink. These checks do not
qualify arbitrary Flowchart sources or explicit Marker/ClusterLabel rules.
Sequence checks actor, message, note, loop, and autonumber label assignments plus activation
styles in SVG; PNG independently checks actor/note surfaces and label ink, message strokes, and
unoccluded lifeline strokes. These bounded checks do not qualify arbitrary Sequence sources.

## Artifact catalog companion

Archive qualification writes `preset-qualification.catalog.json` beside the full execution record.
It joins the catalog returned by the exact CLI's `capabilities --json` with fresh runner-owned
SVG/PNG cells: Flowchart/State/Sequence for Brutalist and Spotless, and Flowchart/Sequence/XY Chart
for Cyberpunk. A mismatch in production metadata or output bytes rejects the join.
The three native candidates retain alpha maturity and report `host_dependent` under the declared
system-font profile; the seven retained presets keep empty scope. The companion binds the final
archive and executable hashes, source revision, host, recipe/resource identities, and scenario
conditions. It excludes private target receipts and does not modify the binary or archive.

The archive `--preset-qualification-check` mode reruns qualification and checks both the execution
record and its companion. Missing or stale companions reject verification. A standalone
`qualify_theme_presets.py --cli` run keeps its artifact catalog inside the execution record; only
archive verification emits the archive-bound companion. Qualification records use schema 1.

Production Rust and SDK discovery continues to expose the shared unqualified catalog. A downloaded
JSON file is not itself a qualification authority: consumers need its trusted release provenance
and matching artifact identity. These observations cover the named scenarios and host conditions,
not arbitrary source text or every installation's font environment. Stable promotion and loading
qualified metadata into production discovery remain separate C7a work.

## Consume a released artifact catalog

For releases produced by this workflow, download the CLI archive, `release-verification.json`,
and `merman-cli-x86_64-unknown-linux-gnu.preset-catalog.json` from the same trusted release.
Verify release provenance and the files' digests against the manifest. The companion's source
commit, target, version and archive SHA-256 must match the selected artifact; it also records the
executable SHA-256. Other target archives have no qualification claim from this companion.

Read `catalog.presets[].qualified_cells` together with its `profile_id`, `admission_status`,
`host`, `render_config`, `svg_pipeline`, and `qualifications` scenarios. The current cells are
host-dependent observations of the preset-specific fixed SVG/PNG scenarios above with system fonts.
They do not promise equivalent results on every font installation. Unknown profiles or admission
IDs provide no portable assurance. Empty cells leave a preset selectable as alpha without granting
qualification; the final render/export admission remains authoritative for an actual request.

The CLI's built-in `capabilities --json` and shared SDK metadata continue to describe the
unqualified production catalog. They do not automatically trust a JSON file placed next to a
binary. This public release companion is a separate, explicitly artifact-bound discovery surface.
