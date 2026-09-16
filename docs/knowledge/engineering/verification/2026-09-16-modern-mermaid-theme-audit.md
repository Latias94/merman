---
type: Audit Baseline
title: Modern Mermaid theme capability audit baseline
timestamp: 2026-09-16
related_plan: docs/plans/2026-09-15-theme-c7a-c7b-replan.md
git_branch: refactor/presentation-theme-model
git_commit: 2e3cb995cb5a2c3a67db370adf10d136d727b65c
tags: theme,preset,modern-mermaid,audit
---

# Corrected reference inventory

Reference: `repo-ref/modern_mermaid` at
`a021cbce37fc0b07a9f4791c28e983101ea06f2d`.
`src/utils/themes.ts` SHA-256:
`9f1baf22457de1fbc5217437dc103840a14f15f2213eeaba45f91e306f6db92d`.
The checkout has an unrelated untracked `pnpm-workspace.yaml`; it was not used or modified.

The earlier indentation-based scan was wrong: there are **24 themes**, not 22. `handDrawn`
and `grafana` have different indentation and were omitted; they are not nested configurations.
The earlier 225-variable, 223,377-byte and selector-category counts are withdrawn. They also
counted JavaScript string characters rather than UTF-8 bytes in some CSS strings.

A read-only TypeScript AST traversal of the exported `themes` object's properties finds
24 `mermaidConfig` objects, 244 `themeVariables` properties, and 240,692 UTF-8 bytes of
`themeCSS` string contents. CSS sizes range from 1,333 to 18,646 bytes. This measures source
configuration size only, not rendered output, capability coverage, or Merman package cost.

The existing [capability corpus](../../../alignment/MODERN_MERMAID_THEME_CAPABILITY_CORPUS.md)
already owns the reviewed 24-theme mechanism and value-level mapping. Its snapshot uses the
same revision and source hash. Reuse that corpus instead of inventing another keyword-based
classification or claiming that CSS filters and backgrounds are inherently unsupported.
Corpus portability is an intended semantic contract, not proof of public preset implementation.

The 24 names are:

```text
linearLight, linearDark, notion, cyberpunk, monochrome, ghibli, spotless, brutalist,
glassmorphism, softPop, darkMinimal, wireframe, handDrawn, grafana, memphis, noir,
material, aurora, win95, doodle, organic, hightech, kawaii, geometricCollage
```

Current Merman has ten catalog entries. Only Brutalist, Spotless and Cyberpunk share reference
names; matching names do not establish equivalent recipes. Font stacks remain host/consumer
inputs. No Inter asset was added. Interactive annotation colors remain application-owned,
consistent with the existing corpus.

# Reference examples

`MERMAID_EXAMPLES.md` contains 34 fenced Mermaid blocks across 11 principal families, including
multilingual and advanced examples. SHA-256:
`c21f39ee68a9537c4651c3e2a54fc491c03a99f178806f0564e97c061d21981f`.

The earlier 88-run smoke used handwritten minimal diagrams; its description as examples
extracted from this file was incorrect. It establishes only successful execution of those
synthetic inputs. The corrected literal-input matrix and parser comparison are recorded in
[the impact audit](2026-09-16-theme-capability-impact-audit.md).

# Size and timing observations retained with limits

Previously measured local files, in bytes (not identical source/profile claims):

| Artifact | Bytes |
| --- | ---: |
| macOS ARM64 CLI executable | 51,379,344 |
| CLI tar.xz | 13,502,080 |
| LSP executable | 17,807,584 |
| LSP tar.xz | 4,050,224 |
| Node N-API darwin-arm64 | 24,548,080 |
| Node WASM | 20,084,758 |
| Web analysis WASM | 3,674,858 |
| Web ASCII WASM | 5,201,971 |
| Web editor WASM | 3,785,997 |
| Web full WASM | 15,937,667 |
| Web render WASM | 14,036,578 |

51,379,344 bytes is about **49.00 MiB**, not the previously reported 51.4 MiB.
Raw files and stripped/compressed metrics must remain separate. Without matched baseline
artifacts and feature profiles, these observations do not establish a size regression.

The earlier `time -p` samples are insufficient for latency conclusions. They mixed launch/font
initialization with rendering and used coarse timing and too few samples. They cannot establish
cold-start cost, throughput, memory, or a speedup. Formal comparison will use the existing
benchmark owners and matched inputs/profiles; no budget was widened from these samples.

# Public Cyberpunk consumer boundary review

The September 16 follow-up inspected current source `81ee5cad0` and the pinned reference recipe.
The external browser comparison used a reused CLI, Mermaid 11.12.1 and no font download; it corroborates missing visible mechanisms but is not an exact-HEAD or all-target qualification run.

The public catalog's recipe builder still returns `ThemeDefinitionV1`, then the existing shared `materialize_spec_wire` path serves both compilation and complete-spec export.
That shared seam already exists; the missing capability is composition of a full built-in recipe there, not a second compiler/export pipeline.
The current Cyberpunk recipe omits the reference's ordered glow and layered grid/radial background despite those concepts being present in the complete model.

The global effect binding matrix admits State/State only.
The nonempty State writer and native filter receipt recognize a single SourceGraphic zero-spread shadow; the exporter also assumes one reference per filter.
State has additional typed Clear routes, so this is not a claim that every other State effect facet is absent.
Sequence width/radius, Flowchart label weight and XY per-series stroke/opacity consumers also need work for the actual reference scenes.
Updating only the effect graph emitter would not close these drawing and native-admission boundaries.

The reference background includes preview-container composition, with two-direction 40px grid and radial Screen layer.
Full-spec root canvas support already exists, but the public recipe does not use it.
C6's hand-built Cyberpunk scenes remain useful mechanism tests and cannot qualify a different public recipe or establish reference reproduction.

The [current product boundary plan](../../../plans/2026-09-16-1618-refactor-theme-product-boundaries-plan.md) makes three public-entrypoint scenes—Flowchart, Sequence and XY Chart—required before final C7a freeze, while retaining honest limits for the other themes and families.
No production code or runtime tests were changed or run during this planning review.
