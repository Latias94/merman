# Scoped preset qualification

The workspace-only Rust runner executes the exact Brutalist, Spotless, and Cyberpunk catalog
recipes on the declared `native-state-system-fonts-v1` State SVG/PNG profile. It checks terminal
styles, raster coverage and label ink, admission reasons, residuals, and artifact/resource identity.
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

The record contains:

- The clean source commit and lockfile digest.
- The actual executable digest identified by Cargo's artifact output, build command, Rust toolchain,
  and host identity. The source commit and binary bind compiler, renderer, writer, target-receipt
  encoding, and qualification code together without maintaining another source dependency graph.
- Each preset's qualification profile/schema and exact recipe/resource fingerprints.
- Each declared source, family, target, admission status/reasons, font source, residual counts, and
  document/artifact/target-evidence/target-receipt digests produced by the Rust runner.

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

`versions-and-packages` in Release Preflight runs both commands against the immutable source SHA
selected by input validation. Its Ubuntu host installs DejaVu system fonts for the declared native
profile; those files are runner prerequisites, not library or preset assets. A failed execution or
freshness check fails the job. The successful record is uploaded as
`preset-qualification-<source_sha>-linux`, preserving the source/host scope beside other release
artifacts. Qualification does not edit the source catalog or embed its own binary digest.

This job owns a native Linux host observation. It does not qualify Web, Flutter, another host font
environment, or every shipped artifact profile. Those consumers still use final target admission.

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

Constructing a Rust cell is only constructing metadata, not issuing proof. Public scopes remain
empty until generated catalog promotion verifies fresh evidence for the declared profile and the
actual artifact build. Flowchart retains bridge residuals; Sequence retains unsupported generic-text
requests only where completed role writers do not cover their fills, or other requested facets
remain unsupported. The seven retained compatibility recipes and the fixed 18-cell C6a ledger keep
their existing dispositions. The positive qualification profile remains State SVG/PNG only.
