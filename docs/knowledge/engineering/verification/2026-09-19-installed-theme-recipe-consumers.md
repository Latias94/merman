---
type: Verification
title: Installed Python and Node recipe consumers
timestamp: 2026-09-19
git_commit: 0c1b1047a7453b5668e371da38019b20b9c66815
related_plan: docs/plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md
---

# Scope

The [2026-09-21 refresh](2026-09-21-installed-theme-consumers.md) supersedes the local Python
and Node installed artifacts below with immutable source `f3a783c29`. This record preserves
the earlier package identities and verification history.

The Python UniFFI wheel, Node Darwin ARM64 packages and Node WASM package were built from clean
source `0c1b1047a7453b5668e371da38019b20b9c66815`, using their existing package owners and exact
artifact recipes. The checkout stayed clean through collection. Cargo builds ran sequentially
with one build job and reused the workspace target directory. The host used Rust 1.95.0,
Python 3.14.7, Node 24.21.0, npm 12.0.2 and wasm-pack 0.15.0 on macOS ARM64.

The enhanced installed-consumer checks are committed as `ff9b9991b`. That commit changes only
the two smoke owners, not package implementation or build inputs. This is a local installed
SVG/authoring witness. It does not freeze C7a, publish packages or qualify new preset cells.

# Packaging input repairs

The first clean Node build at `a4a04ef1b` failed before compilation because its independent
`crates/merman-node/Cargo.lock` no longer matched the current manifests. Cargo's offline metadata
refresh removed 12 obsolete packages from the former font shaping/decoding closure without
upgrading packages or changing the intended Node capability recipe. Locked metadata and both
real transport builds then passed. The `--locked` requirement was retained.

The license owner also found stale input digests in 13 source reports. Regeneration changed
only their root Cargo.lock digest. Synchronizing release copies additionally removed 11 obsolete
dependencies from Android's previously stale packaged report; it now equals the Android source
report exactly. No license text or generated report was hand-edited.

The generator's final `--check` passed for all 13 reports; release projection checks passed for
382 files; the third-party contract and legal material checks for 24 governed Cargo packages
passed. The representative dependency-closure check passed for its 34 checked profiles. These are current
manifest/report checks, not builds or execution on all declared targets.

An initial Python wheel at `a4a04ef1b` passed runtime checks but carried the pre-refresh legal
input digest. It remains diagnostic history. The final wheel below was rebuilt at `0c1b1047a`
and installed into another fresh environment; its embedded report contains the actual root
lockfile digest `d2ed8f1b2ed6040b28a9006b89278257b82db6642f6d45ae622ca2990027b081`.

# Installed artifacts

| Artifact | Packed bytes | SHA-256 |
| --- | ---: | --- |
| `merman-0.8.0a7-py3-none-macosx_11_0_arm64.whl` | 9042331 | `8a18798e14cd55d8e18f0c55789b7f1d12187411382a3bc5e3f989828c78b799` |
| `mermanjs-node-0.8.0-alpha.7.tgz` | 113927 | `923b9f857a249638c23d2267c0203644710965518297f0d62735fe3d147a3fcb` |
| `mermanjs-node-darwin-arm64-0.8.0-alpha.7.tgz` | 10541620 | `c0b3f9546e23c564e6a0b1333ce31efd6d8f11e2feceee941a07205184ffbff6` |
| `mermanjs-node-wasm-0.8.0-alpha.7.tgz` | 7032865 | `98f7ec0939d31d17ef62e44ccbcf789a1282064f10244a9775441d6a61cced2b` |

The wheel's dylib is 21,883,936 bytes, the N-API binary 24,483,152 bytes and the Node WASM binary
19,714,138 bytes. These are artifact observations, not matched alpha.6 performance/size deltas or
changed budgets. Node build receipts bind both artifacts to the same source digest
`sha256:4f0d1ca3cb9edca3f950755239ef5b26654aabf4375015b9b28b8388788123c6`.

Python installed the wheel with `--no-index --no-deps`; a separate isolated import checked that
the module resolves inside the new virtual environment. Its complete checked-in smoke ran with
`python -E` from that consumer directory. Node packages passed assembly and packed-ownership
verification, then installed offline with lifecycle scripts disabled into two separate projects.
Each smoke resolved the installed public package entry point; no source-tree runtime replaced it.

# Authoring and recipe checks

Previously, the two smoke owners extracted `complete_spec` from preset export and rebuilt the
selection envelope. They now pass the complete versioned export directly as `options.theme`,
including after JSON save/load. Brutalist, Spotless and Cyberpunk retain exact preset/import SVG
equality. Cyberpunk additionally exercises the unchanged complete Flowchart, Sequence and XY Chart
fixtures. Missing version, versions 0 and 2, and recipes mixed with `preset` or `spec` are rejected
as `MERMAN_OPTIONS_JSON_ERROR`.

Each installed Node package passed:

- Two shared authoring vectors and two exact catalog comparisons, including family design metadata.
- Three family-isolation checks, a scoped rule override and a cold complete-spec render.
- Three direct preset imports, three complete Cyberpunk scene imports and five recipe rejections.
- 44 support queries, six shared authoring diagnostics and two resource-limit rejections.
- 23 JSON operations and 29 successful SVG renders.

The final Python wheel passed its full native smoke, shared authoring vectors and catalog golden,
three-family isolation, scoped rule, cold spec, three direct imports, three complete scenes and
five recipe rejections. It exercised all 22 shared support vectors and all three budgeted authoring
operations through one-shot and reusable consumers. Its broader smoke also covers the profile's
analysis, ASCII, icon and text-measurement services; it does not add binary exporters.

The Node contract suite passed 109/109 under the pinned Node/npm environment. An initial run with
npm 11 failed the existing npm-pack metadata contract; selecting the required npm 12 fixed the
environment without relaxing that contract. Focused Python owner tests passed 27/27. Python
syntax, workspace formatting and diff checks passed.

# Cross-process file exchange

Separate installed processes exported Cyberpunk to actual JSON files. Both Node transports read
the unchanged Python file directly; the final Python process read both Node exports directly.
The exported objects agreed without rebuilding envelopes or removing canvas/effect fields.
The final Python export also exactly reproduced the initial file bytes, SHA-256
`4577d372676360c724e6ae568046ad9a691adeda99c2310e598f92e9df0b8cfb`.

All three consumers rendered identical SVG bytes for each complete scene with
`site_config.htmlLabels:false` and a shared per-family diagram ID:

| Scene | SVG bytes | SHA-256 |
| --- | ---: | --- |
| Flowchart | 32969 | `4af0678d75393c6f204803d47b0a0ca51de3d8e5ff85c174767a39477613f86d` |
| Sequence | 48266 | `51a657e052f0051cd603d2c817762a3710c7d7b286d6648d1fbe69f1cd38de30` |
| XY Chart | 33427 | `6da8363df3a8e2784ab8d47d3c779be8ef4289684ac9e1bdc22ec23b07238bd4` |

No SVG normalization or pixel tolerance was used. This demonstrates recipe exchange and transport
output agreement for the fixed inputs. It does not replace the separate
[native scene qualification](2026-09-19-cyberpunk-controlled-scenes.md), browser pixel probes or
PDF raster checks.

# Reproduction and remaining boundaries

Use `scripts/build-python-uniffi-wheel.py --wheel-dir <new-directory>`, install the wheel into a
fresh virtual environment and run `platforms/python/merman/examples/smoke.py`. For each Node
transport, use the release owner's `build-candidate.mjs`, `assemble-packages.mjs`,
`verify-packages.mjs`, npm pack/install and `smoke-installed-package.mjs` sequence from
`.github/workflows/release-node.yml`. Use the checked-in smoke owners from `ff9b9991b` or later.

Artifacts, installed consumers, build receipts, recipe files, SVGs, smoke JSON and logs remain
under `target/bench/experiments/installed-theme-consumers-a4a04ef1b/`. The directory name records
the initial attempt; `artifact-identities.json` records the final source and artifacts above.
Its SHA-256 is `d612f4b76c7d173364eda32171dd440c9f2db1bc752e3c1cdf28bdc9c0d5d773`.
The retained clean checkout and tool paths are in `/tmp/merman-installed-theme-context.json`.
Final Python evidence uses the `python-final-*` files, not the initial wheel. Node evidence uses
`node-{native,wasm}-{smoke,file-exchange,build-receipt}.json` and `node-artifacts.json`.

Linux/Windows native packages, compiler/runtime floors, installed browser/Typst/mobile/native SDK
profiles and final CLI/LSP archives do not inherit these observations. PNG/JPEG/PDF are absent
from these selected Python/Node profiles. Missing-font behavior, controlled-font portability,
arbitrary sources and all six public customization/resource journeys are not certified here.
Public qualified cells stay empty. Final same-source release preflight, broader consumer coverage,
matched performance/footprint comparisons and C7a closure remain open; no tag or publication ran.
