# Flowchart U4 Adapter Discovery Anchor

Date: 2026-07-29

This receipt binds the completed U4 discovery evidence before confirmation sampling. It extends,
but does not modify, the admission contract in
`flowchart_u4_adapter_preregistration_2026-07-29.md`.

## Discovery evidence

- Report: `target/bench/u4-adapter-candidate/discovery.json`.
- Report SHA-256: `86294ff35f280b43dbfadeabe1dd6e7bf2cac589a1627f9a3fdf0a597022accf`.
- Report schema: `2`.
- Harness schema: `compare-self-v2`.
- Evidence mode: confirmation discovery only.
- Contract errors: none.
- Comparable public lanes: `flowchart_medium`, `flowchart_large`,
  `flowchart_ports_heavy`, and `class_medium`.

## Frozen runners

- Base commit: `6b5f3e0ef2bc1b3162712b5a2de71fe8f887e213`.
- Base tree: `7e9f82864725122c37e7b8931c3bdaaf5f790a4f`.
- Base executable SHA-256:
  `3815342419503cbc5905aba8cce7d3fb3985a8f474a928d2149f3fcd0bb86620`.
- Candidate commit: `234ad437335db899494612b3b9f0be83fe0af954`.
- Candidate tree: `1f416a8512a783d6aa3c1926ff7845299d5d1fd6`.
- Candidate executable SHA-256:
  `2de13fb9ed535afb3ad70b100d27b414f821f0009c1945e1a04369968a478d42`.
- Freeze context: `20260728T203733Z-92407-e24762d295e3`.

Confirmation may reuse these immutable executables only through
`--reuse-discovery-json` together with
`--reuse-discovery-sha256 86294ff35f280b43dbfadeabe1dd6e7bf2cac589a1627f9a3fdf0a597022accf`.
The runner must reject any report, checkout, recipe, toolchain, corpus, fixture, benchmark-list, or
executable drift before sampling.
