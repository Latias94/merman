# Run the theme tool

Use Node.js 22+ and an engine module exporting `createNodeEngine()`. The helper calls existing
Merman operations; Rust owns admission, reference inspection, support queries, and rendering.
Pass an assembled source package entry with `--engine`, or use the default `@mermanjs/node` when
that matching package is installed and resolvable from the skill's location. Explicit Node-WASM
is also supported. Browser packages are not Node engines.

## Source checkout setup

From the Merman root, the existing Node package recipe builds and assembles an explicit Node-WASM
package. Run Cargo builds serially and reuse the existing target directory:

```text
node platforms/node/scripts/build-candidate.mjs --candidate node-wasm
node platforms/node/scripts/assemble-packages.mjs --wasm
```

The resulting engine entry is `./platforms/node/dist-packages/node-wasm/dist/index.mjs`.
These commands prepare local artifacts; they do not publish packages. Do not substitute an older
binary because its package version happens to match: the current design contract is unreleased.

If the local Binaryen optimizer fails, the same Rust engine can be built without the size pass
for local evaluation. Keep that artifact separate from assembled/release artifacts:

```text
wasm-pack build crates/merman-node --target nodejs --release --no-pack --no-opt --out-dir ../../.build/theme-skill/wasm -- --locked -j1 --no-default-features --features all-diagrams,transport-wasm,svg,layout-cytoscape,layout-elk
```

Set `CARGO_TARGET_DIR` to the repository's existing absolute `target` path if the shell environment
would otherwise use the standalone crate's target directory. In `.build/theme-skill/wasm/package.json`
write `{"type":"commonjs"}`. Create `.build/theme-skill/engine.mjs` with:

```js
import { createNodeEngine as create } from "../../platforms/node/src/engine.mjs";
import { loadNodeWasmTransport } from "../../platforms/node/src/candidates/wasm.mjs";
import { fileURLToPath } from "node:url";

export function createNodeEngine(options) {
  return create(options, {
    loadTransport: (json) => loadNodeWasmTransport(json, {
      modulePath: fileURLToPath(new URL("./wasm/merman_node.js", import.meta.url)),
    }),
  });
}
```

This uses the existing workspace engine and Node transport, including their normal identity and
contract validation. It is a development adapter, not a published package or release receipt.

## Commands

Paths below are relative to the Merman root. Replace `ENGINE` with the selected module entry;
create the output directory first. Quote paths containing spaces according to the active shell.

```text
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs catalog --engine ENGINE
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs export cyberpunk --engine ENGINE --out output/cyberpunk.theme.json
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs materialize output/theme.design.json --engine ENGINE --out output/theme.materialized.json
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs support output/support-query.json --engine ENGINE
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs render output/theme.json output/flowchart.mmd --engine ENGINE --options output/render-options.json --out output/flowchart.svg
```

- `export` creates a complete editable recipe with the preset's declared relationships.
- `materialize` accepts the raw `design` payload, not the outer recipe. Extract it from a known
  valid authored object when inspection is needed; preserve the original recipe for rendering.
- `support` forwards the versioned query from [authoring.md](authoring.md).
- `render` validates the exact recipe text, including duplicate-key errors, and saves SVG plus
  `<name>.metadata.json` from one `svg` operation. Source and options are separate inputs. It does
  not rewrite them or force a different look. Read the metadata sidecar after every changed render.

Render options use Options JSON 3 and must omit `theme`, which comes from its own file. For a new
design explicitly intended for Classic Flowchart shadows, an example is:

```json
{
  "version": 3,
  "site_config": { "htmlLabels": false, "flowchart": { "look": "classic" } },
  "svg": { "pipeline": "parity" }
}
```

Leave the original options unchanged for comparisons and user-supplied diagrams. Recipe `mermaid`
holds only `theme`, `dark_mode`, and scalar `variables`; `flowchart.look` belongs to these separate
options or source frontmatter. Deliver required options and explain them in the design note.

The helper writes fresh files by default. Use new revision names or `--force` when overwriting
outputs is intentional; inputs are protected even with `--force`. It returns structured operation
errors on stderr and a nonzero exit code. A render may succeed with residual theme or rejected
target evidence under BestEffort; the sidecar, not the process exit alone, explains that outcome.

## Use the result

Import the standalone recipe through Playground's Theme → Import JSON, preview it, then Apply.
Native CLI can render it with:

```text
merman-cli render diagram.mmd --theme-file theme.json -o diagram.svg
```

The CLI currently renders recipes but does not expose the theme export/materialize/support
subcommands or the same-render metadata sidecar; the helper uses the Node engine for those jobs.
Native binary export must be tested separately when PNG/PDF is required.
