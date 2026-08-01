# Upgrade from 0.8.0-alpha.3 to 0.8.0-alpha.4

> [!IMPORTANT]
> This guide describes the alpha.4 source contract. Package registries and release channels can
> trail the repository, so verify the installed version before relying on an alpha.4 API or
> capability. Final release benchmarks must be regenerated against the tagged release commit.

Alpha.4 is a broad prerelease upgrade, not a drop-in patch. It expands the Mermaid baseline to
11.16, admits all 35 diagram families, replaces implementation-oriented feature bundles with
observable capabilities, splits the browser SDK into standalone packages, and moves native hosts
to ABI 3.

The practical upgrade rule is:

1. choose the user workflow first;
2. select the package or binary that owns that workflow;
3. enable only the capabilities that workflow needs; and
4. query the installed artifact when optional capability availability matters at runtime.

## Who needs to change something

| If you use... | Required action |
| --- | --- |
| Default `merman` Rust features | Update the version and retest. The default remains the complete SVG product through `complete-svg`. |
| Explicit Cargo features | Replace removed alpha.3 names with alpha.4 capability leaves. There are no compatibility aliases. |
| `merman-cli` root `-i/-o` flags | Existing scripts still route to the compatibility parser, but new scripts should choose `render`, `batch`, or `mmdc` explicitly. |
| `@mermanjs/web/<subpath>` or `@mermanjs/web/pkg/**` | Replace the import with one standalone browser package. Subpaths and raw WASM files are no longer public API. |
| Native C, Flutter, or Android bindings | Rebuild or upgrade the complete host package and migrate from ABI 2 to ABI 3. Reject an ABI mismatch during initialization. |
| Python or Apple bindings | Upgrade the generated UniFFI wrapper and matching native artifact together; do not mix alpha.3 and alpha.4 components. |
| Analysis, editor, or LSP APIs | Review the alpha.4 changelog and surface documentation. These contracts gained substantial typed diagnostics, document, snapshot, and capability changes. |
| Node.js or SSR | Continue to invoke `merman-cli` as a subprocess. No in-process Node package is admitted for alpha.4. |
| Typst | Treat it as an independent release track. The published `@preview/merman:0.2.0` package is not an alpha.4 artifact. |

## Choose the alpha.4 surface

| Workflow | Use | Selection and tradeoff |
| --- | --- | --- |
| Parse Mermaid into Rust models | `merman-core` | Smallest foundational API; no rendering or diagnostics product surface. |
| Lint, diagnose, or scan Markdown/MDX in Rust | `merman-analysis` | Analysis without renderer, layout, export, icon, or network dependencies. |
| Render complete SVG in Rust | `merman` | Use the default or `complete-svg`; includes SVG, Cytoscape, ELK, and math. |
| Render basic deterministic SVG in Rust | `merman` | Disable defaults and select `svg`; optional layout engines and math remain absent. |
| Convert, export, lint, or batch from a shell | `merman-cli` | The release binary is the complete product; source builds can select narrower feature leaves. |
| Run a language server | `merman-lsp` | Use the release binary, or build the explicit `stdio` transport. |
| Render in a browser | `@mermanjs/web-render` | Complete SVG/layout/math without analysis, editor, or ASCII APIs. |
| Analyze in a browser | `@mermanjs/web-analysis` | Diagnostics, facts, and detection without rendering. |
| Provide browser editor intelligence | `@mermanjs/web-editor` | Analysis plus parser-backed editor APIs; intended for a dedicated Worker. |
| Render ASCII in a browser | `@mermanjs/web-ascii` | ASCII/Unicode only; family support is capability-graded. |
| Need all browser capabilities in one realm | `@mermanjs/web` | Full browser SDK; avoid combining it with duplicate slim packages. |
| Embed a prebuilt native SDK | The Python, Flutter, Android, or Apple package | The alpha.4 release contract defines one complete SKU per surface, not a full/slim prebuilt matrix. |
| Embed the C ABI | `merman-ffi` | Build the source crate; there is no downloadable C binary SDK. |
| Render from Node.js or SSR | `merman-cli` subprocess | The private Node candidate is not a supported release surface. |

See [Package Surfaces](PACKAGE_SURFACES.md) for delivery channels and the exact release evidence
required by each surface.

## Cargo feature migration

Alpha.4 Cargo features name observable results. Features remain additive, so use
`default-features = false` when absence matters and remember that another dependency can re-enable
a leaf through Cargo feature unification.

| Alpha.3 feature | Alpha.4 selection |
| --- | --- |
| `render` | `svg` |
| `cytoscape-layout` | `layout-cytoscape` |
| `elk-layout` | `layout-elk` |
| `ratex-math` | `math` |
| `raster` | Select `png`, `jpeg`, and/or `pdf` independently. |
| `core-host` | Select `system-clock`, `system-timezone`, `system-random`, and/or `system-timing`. |
| `core-full` | No direct replacement. Select only the output and host capabilities the product needs. |
| Historical `full` or `tiny` profiles | Use an exact artifact profile, or disable defaults and select direct leaves. |

A complete Rust renderer can move from the old implementation bundle to the result-named
convenience feature:

```toml
# 0.8.0-alpha.3
merman = { version = "=0.8.0-alpha.3", default-features = false, features = [
  "render",
  "cytoscape-layout",
  "elk-layout",
  "ratex-math",
] }

# 0.8.0-alpha.4
merman = { version = "=0.8.0-alpha.4", default-features = false, features = [
  "complete-svg",
] }
```

A basic SVG-only embedding becomes:

```toml
merman = { version = "=0.8.0-alpha.4", default-features = false, features = ["svg"] }
```

Use [Choosing Merman capabilities](../FEATURES.md) for package-specific examples and the complete
implication table. `complete-svg` is a facade convenience; lower-level crates and release artifact
profiles select direct leaves.

## CLI migration

Alpha.4 gives native and compatibility workflows separate command trees:

| Existing spelling | Preferred alpha.4 spelling |
| --- | --- |
| `merman-cli -i diagram.mmd -o diagram.svg` | `merman-cli mmdc -i diagram.mmd -o diagram.svg` |
| Native one-file rendering through shared flags | `merman-cli render diagram.mmd --output diagram.svg` |
| Native Markdown conversion | `merman-cli batch README.md --output-dir README.merman` |
| `merman-cli render diagram.mmd -e png` | `merman-cli render diagram.mmd -f png` |

Root `-i/-o` invocations remain hidden compatibility aliases. Native `render -e` and `batch -e`
are deprecated aliases for `-f/--format` during `0.8.x` and are scheduled for removal in 0.9.0.
The `mmdc -e/--outputFormat` spelling remains part of the pinned Mermaid CLI compatibility
contract.

Automation should run `merman-cli capabilities --json` and check the reported CLI contract when
it depends on compiled commands, outputs, or optional runtime adapters. See the
[CLI reference](../../crates/merman-cli/README.md) for installation and command details.

## Browser package migration

Alpha.3 published one `@mermanjs/web` package with capability subpaths and raw `pkg/**` exports.
Alpha.4 publishes a lockstep package group. Each package exports only its root and owns exactly one
WASM artifact.

| Alpha.3 import | Alpha.4 choice |
| --- | --- |
| `@mermanjs/web` or `@mermanjs/web/full` | `@mermanjs/web` for the complete browser SDK. |
| `@mermanjs/web/render` or `@mermanjs/web/render-only` | `@mermanjs/web-render` for the supported complete SVG renderer; this is a capability expansion, not an identity-preserving rename. |
| `@mermanjs/web/ascii` | `@mermanjs/web-ascii`. |
| `@mermanjs/web/core` | No identity-preserving replacement. Choose `web-analysis`, `web-editor`, or the full package by workflow. |
| `@mermanjs/web/pkg/**` | No replacement. Import the selected public package root. |

For example:

```ts
// 0.8.0-alpha.3
import * as merman from "@mermanjs/web/render";

// 0.8.0-alpha.4
import * as merman from "@mermanjs/web-render";
```

Do not combine the full package with a slim package in the same realm unless the application
deliberately wants two independent WASM runtimes. See the
[browser package guide](../../platforms/web/README.md) for initialization, Worker, and packaging
details.

## Native ABI migration

Alpha.4 C, Flutter, and Android hosts use ABI 3. Python and Apple use generated UniFFI bindings
from the matching native artifact. Upgrade each language package and native artifact together; do
not mix an alpha.3 generated wrapper with an alpha.4 library. ABI 3 hosts must validate the ABI and
generated runtime capability catalog before requesting optional outputs, resources, or host text
measurement.

Follow the [ABI 3 migration guide](../bindings/ABI3_MIGRATION.md) and the surface-specific Python,
Flutter, Android, or Apple documentation. A channel listed in the repository is not proof that the
alpha.4 artifact has already been published there.

## What the refactor changes for users

The alpha.4 candidate expands primary SVG admission from 27 to all 35 Mermaid 11.16 families. It
also makes analysis, editor intelligence, layouts, math, exports, icons, network access, Markdown
parallelism, and system runtime adapters independently selectable where the owning product exposes
them.

The historical checkpoint against alpha.3 found a clear win for analysis-only CLI builds, while
complete products became broader rather than uniformly smaller or faster:

| Historical checkpoint | Alpha.3 | Measured alpha.4 candidate | Interpretation |
| --- | ---: | ---: | --- |
| Primary SVG admission records | 27 | 35 | Broader Mermaid 11.16 coverage. |
| Lint/analysis CLI binary | 25,477,648 bytes | 8,166,352 bytes | 67.95% smaller for the measured lint workflow. |
| Lint normal dependency identities | 333 | 123 | 63.06% fewer resolved normal dependencies. |
| Default CLI binary | 32,194,272 bytes | 36,925,360 bytes | 14.70% larger, but the default capability contract also changed. |
| Minimal same-capability native SVG | baseline | 1.12x median latency | A historical 32-fixture checkpoint, not a universal performance improvement. |

These measurements compare alpha.3 with candidate commit `d2698d0a3` on one Apple M4 Pro. Later
focused work removed duplicate Requirement label measurement, accelerated ordinary Mindmap labels,
and accepted a smaller Kanban label-preparation improvement. Those adjacent fixes do not replace a
fresh alpha.3-versus-release A/B run.

Use the [detailed evidence report](ALPHA3_TO_ALPHA4_REFACTORING_REPORT.md) for recipes and historical
measurements. Use the [performance plan](../performance/PERF_PLAN.md) for the rolling optimization
status.

## What remains unproven before release

- The final alpha.4 target commit is not fixed until the release tag is created.
- Final same-host alpha.3 A/B measurements still need to refresh the complete and minimal SVG
  lanes, including Class, Sequence, Requirement, and Mindmap attribution.
- Browser-WASM throughput has not been compared with browser Mermaid.js under one equivalent
  browser contract.
- The private Node candidate lacks reproducible all-target admission and is not a supported package.
- Package availability must be verified at each registry or GitHub Release; repository manifests
  describe the intended contract, not live publication state.

## Further reading

- [Alpha.3 to Alpha.4 evidence report](ALPHA3_TO_ALPHA4_REFACTORING_REPORT.md)
- [Changelog](../../CHANGELOG.md)
- [Capability guide](../FEATURES.md)
- [Package surfaces](PACKAGE_SURFACES.md)
- [Performance plan](../performance/PERF_PLAN.md)
- [ABI 3 migration](../bindings/ABI3_MIGRATION.md)
