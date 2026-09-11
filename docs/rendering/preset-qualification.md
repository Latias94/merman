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

## Remaining publication boundary

These records supply clean-build provenance and executable freshness checks for the scoped runner.
They do not freeze C7a or promote public catalog entries. `qualified_cells` currently contains only
family/output identifiers and cannot express this host-font condition, so it remains empty.
Generated catalog promotion still needs a truthful resource/admission scope and its own freshness
connection. Flowchart/Sequence residuals, the seven retained compatibility recipes, and the fixed
18-cell C6a ledger retain their existing dispositions.
