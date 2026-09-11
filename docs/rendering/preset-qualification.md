# Scoped preset qualification

The workspace-only Rust runner executes the exact Brutalist, Spotless, and Cyberpunk catalog
recipes on the declared `native-flowchart-state-sequence-system-fonts-v3` profile for
Flowchart/State/Sequence SVG and PNG. It checks terminal styles, raster coverage and label ink,
admission reasons, residuals, and artifact/resource identity.
State PNG uses 1x; Flowchart and Sequence use 4x so thin lines and label paint remain observable.
Flowchart places its labeled edge inside a contrasting subgraph: a missing translucent label
background must fail the raster check. Qualification schema 3 replaces the previous two-family
profile; an older receipt cannot acquire the new Flowchart cells. Each observation records its
scenario's `png_scale`; SVG remains in diagram units.
The profile requires working system fonts and reports **HostDependent**. It does not bundle a font,
inject rules, or claim portable output. See [coverage](diagram-theme-coverage.md) for other families.

## Record a candidate

Use a clean checkout, including untracked files, with the project's Rust toolchain and Python 3.11
or later. The script does not clean or modify source files. Its Cargo invocation uses the existing
target directory, two build jobs, Release, and the acceptance crate's explicit `png` feature profile.

```console
python3 scripts/qualify_theme_presets.py --output target/preset-qualification.json
python3 scripts/qualify_theme_presets.py --check target/preset-qualification.json
```

Add `--cli path/to/merman-cli` to both commands to qualify an existing production CLI on this
host. The checker sends each Rust-owned source through `render --theme-preset`, uses the declared
PNG scale and `resvg-safe` SVG pipeline, and requires exact output bytes. It reuses the bounded
release-process runner; it does not interpret SVG/CSS or reconstruct renderer receipts. A missing
observation, changed source/output, or executable replacement rejects qualification.

The schema-3 record contains:

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

`versions-and-packages` in Release Preflight qualifies the CLI extracted from its verified host
cargo-dist archive. Its Ubuntu host installs DejaVu system fonts for the native profile. Archive
checksum, tracked package contents, runtime capabilities, and normal CLI smoke checks must pass
before `--preset-qualification-output target/preset-qualification.json` runs qualification. The
record additionally binds `cli_archive.sha256`, `target`, and `version` to that exact archive.

A second archive verification uses `--preset-qualification-check` to rebuild and reexecute the
Rust and extracted-CLI paths and compare the complete record. Both flags require `--execute` and
the existing host-target check. Use these archive flags to replay an archive record; standalone
`--check --cli` deliberately cannot reproduce its archive provenance. The job archives the result
as `preset-qualification-<source_sha>-linux`. Qualification does not edit the source catalog or
embed a binary's own digest into that binary.

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
and its public catalog companion are workflow artifacts; neither is an additional member of the
immutable release bundle. Other native rows retain their product smoke checks without claiming this profile.

## Remaining publication boundary

These records supply clean-build provenance and executable freshness checks for the scoped runner.
They do not freeze C7a or promote public catalog entries. Each `qualified_cells` entry now carries
`family_id`, `output_id`, `profile_id`, and `admission_status`. The profile names the tested scenario
and resource conditions; admission is an open ID such as `portable` or `host_dependent`. Unknown
profiles or admission classes must not be interpreted as portable support. A family/output may have
multiple profiles, ordered by `(family_id, output_id, profile_id)`; conflicting statuses under the
same profile are invalid. Rust, Web, and Flutter expose these conditions; other transports preserve the shared JSON.
This unpublished alpha correction stays within theme catalog schema 3. Rust and Dart cell
constructors now require the profile and admission arguments; there is no implicit default.

Constructing a Rust cell is only constructing metadata, not issuing proof. Built-in scopes remain
empty. The archive companion below projects fresh evidence only for its declared profile and actual
artifact build. Flowchart now reports HostDependent without bridge residuals for the fixed
native-candidate admission sources; qualification additionally checks the declared title, subgraph,
node, and labeled-edge scenario. Sequence retains unsupported
generic-text requests only where completed role writers do not cover their fills, or other
requested facets remain unsupported. The seven retained compatibility recipes and the fixed 18-cell C6a ledger keep
their existing dispositions. The positive qualification profile covers only its declared
Flowchart/State/Sequence SVG/PNG scenarios (six artifact cells per native preset).
Flowchart checks canvas, node/cluster surfaces and borders, diagram/cluster/node/edge labels,
the directed edge and marker, and the actual translucent label background. PNG checks use
separate regions so borders or the edge cannot replace missing label ink. These checks do not
qualify arbitrary Flowchart sources or explicit Marker/ClusterLabel rules.
Sequence checks actor, message, note, loop, and autonumber label assignments plus activation
styles in SVG; PNG independently checks actor/note surfaces and label ink, message strokes, and
unoccluded lifeline strokes. These bounded checks do not qualify arbitrary Sequence sources.

## Artifact catalog companion

Archive qualification writes `preset-qualification.catalog.json` beside the full execution record.
It joins the catalog returned by the exact CLI's `capabilities --json` with fresh runner-owned
Flowchart/State/Sequence SVG/PNG cells. A mismatch in production metadata or output bytes rejects the join.
The three native candidates retain alpha maturity and report `host_dependent` under the declared
system-font profile; the seven retained presets keep empty scope. The companion binds the final
archive and executable hashes, source revision, host, recipe/resource identities, and scenario
conditions. It excludes private target receipts and does not modify the binary or archive.

The archive `--preset-qualification-check` mode reruns qualification and checks both the execution
record and its companion. Missing or stale companions reject verification. A standalone
`qualify_theme_presets.py --cli` run keeps its artifact catalog inside the execution record; only
archive verification emits the archive-bound companion. Qualification records use schema 3.

Production Rust and SDK discovery continues to expose the shared unqualified catalog. A downloaded
JSON file is not itself a qualification authority: consumers need its trusted release provenance
and matching artifact identity. These observations cover the named scenarios and host conditions,
not arbitrary source text or every installation's font environment. Stable promotion and loading
qualified metadata into production discovery remain separate C7a work.
