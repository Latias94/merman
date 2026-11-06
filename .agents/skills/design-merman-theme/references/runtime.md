# Run and adopt the theme

## Select a matching runtime

Reuse a source-built CLI or Node engine with the current authoring contract. Record the artifact
path and revision or digest alongside the invocation; a matching package version alone does not
establish matching code. A working CLI is sufficient for render/report work. Use Node for preset
export, materialization, or support queries. Check capabilities before assuming a target exists.

The helper requires Node.js 22+ and a module exporting `createNodeEngine()`. Pass its entry through
`--engine`; the default `@mermanjs/node` must be installed and resolvable from the skill's location.
Explicit Node-WASM is supported; browser packages are not Node engines.

If a matching artifact is missing, follow the current
[Node build owner](../../../../platforms/node/README.md#develop-and-verify). Its source package path is:

```text
node platforms/node/scripts/build-candidate.mjs --candidate node-wasm
node platforms/node/scripts/assemble-packages.mjs --wasm
```

The assembled entry is `./platforms/node/dist-packages/node-wasm/dist/index.mjs`. Build serially,
reuse the repository target directory, and coordinate with other workers before starting Cargo.
Use the build owner's optimizer/toolchain requirements rather than a private engine adapter or a
weaker release build. These commands prepare artifacts; they do not publish packages.

## Invoke the existing helper

Paths below start at the Merman root. Replace `ENGINE` with the selected entry and create the
ignored `target/theme-design` output directory first. Use fresh output names for revisions.

```text
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs catalog --engine ENGINE
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs export cyberpunk --engine ENGINE --out target/theme-design/cyberpunk.theme.json
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs materialize target/theme-design/design.json --engine ENGINE --out target/theme-design/materialized.json
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs support target/theme-design/support-query.json --engine ENGINE
node .agents/skills/design-merman-theme/scripts/theme-tool.mjs render .agents/skills/design-merman-theme/assets/starter.theme.json .agents/skills/design-merman-theme/assets/flowchart.mmd --engine ENGINE --options target/theme-design/options.json --out target/theme-design/starter.svg
```

`export` retains the preset's declared relationships. `materialize` takes the raw `design` payload,
not the outer recipe, and returns an inspection result. `support` takes the versioned query in
[authoring.md](authoring.md). `render` validates original recipe text, including duplicate keys,
and writes SVG plus `<name>.metadata.json` from one operation. Read that sidecar after each render.
Inputs are protected even with `--force`; use it only for an intentional output replacement.

The `options.json` above is Options JSON 3 without `theme`, which has its own file:

```json
{
  "version": 3,
  "site_config": { "htmlLabels": false },
  "svg": { "pipeline": "parity" }
}
```

For library scenes, read the saved family `.mmd` and `.config.json` together. CLI `--config-file`
takes that Mermaid configuration directly. The Node helper takes an Options JSON envelope, so a
scene without source init can place that same configuration under `site_config`. For a captured
workspace, preserve source init/frontmatter and site configuration separately: moving values
between them changes precedence. `flowchart.look` and layout settings belong to those owners,
not `design.mermaid`.

## Native rendering and fonts

An existing CLI can render the seed without a Node package or build:

```text
target/debug/merman-cli render .agents/skills/design-merman-theme/assets/flowchart.mmd --theme-file .agents/skills/design-merman-theme/assets/starter.theme.json --theme-report target/theme-design/starter.report.json --svg-pipeline parity --font-measurement deterministic --font-system off -o target/theme-design/starter.svg
```

For a font-sensitive Class preview, explicitly load files matching the recipe's actual text roles.
This Riso example uses checked macOS fonts; verify the paths on this host and choose corresponding
installed fonts on another system. Recheck after any substitution.

```text
target/debug/merman-cli render playground/src/themes/samples/class.mmd --config-file playground/src/themes/samples/class.config.json --theme-file playground/src/themes/recipes/riso.theme.json --theme-report target/theme-design/riso-class.report.json --font-measurement native --font-file /System/Library/Fonts/Avenir.ttc --font-file /System/Library/Fonts/Menlo.ttc --font-system off -o target/theme-design/riso-class.svg
```

`--font-file` is repeatable and enables native measurement. Keep the selected order and actual
files with the render conditions. Do not combine explicit fonts with deterministic measurement.
Native SVG text is not proof of native measurement, and font names are not embedded fonts. The
CLI report may still describe host-dependent text and an unverified SVG target. Read
[review.md](review.md) before interpreting it. PNG/PDF require separate actual exports through a
capable artifact; `resvg-safe` alone does not qualify either target.

## Adopt the recipe

| Input | Responsibility | How it is consumed |
| --- | --- | --- |
| `theme.json` | Visual design and retained editable relationships | Playground Import JSON; CLI `--theme-file`; Rust `compile_recipe`; Web `theme: recipe`. |
| Mermaid source and optional sample configuration | Content, look, layout, and authored initialization | Use your own diagram to adopt only the design. Keep example options only to reproduce that scene. |
| Rust, CLI, or Web integration | Loads the chosen inputs and invokes a matching runtime | Pick the integration for the application; they are alternative adoption routes. |

Use Playground **Use in project** for generated, current adoption code. Rust **Single file**
embeds the raw inputs in `main.rs`; use its listed Cargo dependencies. **Project files** includes
`Cargo.toml` and `src/main.rs`; `include_str!("../theme.json")` resolves from `src/main.rs`, with
theme, source, and site configuration at the project root. Keep those paths when copying files.
Generated sample projects already consume their example configuration; applying it again is
unnecessary. Web `options.json` already contains its recipe.

The [theme guide](../../../../docs/rendering/custom-diagram-themes.md) owns current API and
precedence details. A library name is display metadata, not a registered preset ID. The JSON must
remain usable independently of this skill, a private generator, or the saved preview SVG.
