# Two theme designs

These examples use the built-in recipes in the matching source runtime. They illustrate
design intent rather than pixel parity with a browser, host-font portability, or output
qualification. The installed catalog and each render's metadata describe those separate facts.

| Design | Reviewed example | Settings |
| --- | --- | --- |
| Brutalist | [Flowchart source](brutalist.mmd): repeating yellow/teal/orange accents, bold labels, black outlines and hard shadows | [Classic configuration](brutalist.config.json); basic supported node shapes |
| Ghibli | [Pie source](ghibli.mmd): warm paper texture, muted slices, distinct title/percentage/legend sizes | Default Pie configuration; no Mermaid scalar styling overrides |

![Brutalist Flowchart with bold labels, ordinal accents and hard shadows](brutalist.png)

![Ghibli Pie with warm paper, muted slice colors and independently sized text roles](ghibli.png)

These previews were exported as native PNG on macOS from the included sources and settings.
Host font resolution affects the result; the commands below reproduce the workflow with the
matching runtime, rather than guaranteeing identical pixels on every system.

## Playground

Paste a source into the editor, open **Theme**, choose **Brutalist** or **Ghibli**, and inspect
the preview. The **Merman preset** selector at the top lists the current diagram's design
treatment next to each preset. **Browse presets** adds recipe color swatches. For Brutalist,
choose **Use Classic look** to explicitly preview Classic.
This keeps other configuration and the layout intact. **Apply** commits the panel's theme,
font and recommended configuration together; **Cancel** discards these drafts while preserving
independent source/config editor edits. Leave the font at **Inherit from theme / source** to use
the recipe's typography. **None** removes
Merman styling and keeps the selected Mermaid base theme.

Read **Current preview** for this diagram's result. **Designed for this diagram** describes
catalog intent, not a guarantee that every requested effect rendered. An unreviewed family
can still receive the recipe's base styling. Use **Download JSON** for an editable standalone
recipe and a workspace share link to include source, settings and the applied recipe together.

| Earlier panel | Current panel |
| --- | --- |
| ![Earlier panel with an implicit Trebuchet override and expanded verification text](theme-panel-before.png) | ![Current panel with explicit font inheritance, one Apply/Cancel model and optional output details](theme-panel.png) |

The current example requests Ghibli's `Open Sans, Noto Sans SC, sans-serif` stack. A browser may
resolve the generic fallback when the named fonts are absent. The earlier panel always injected
Trebuchet, even when the user had not chosen a font. These are real Playground screenshots;
font installation and rasterization remain host-dependent.

## Native CLI

From this directory, with a matching SVG-capable CLI:

```text
merman-cli capabilities
merman-cli render brutalist.mmd --theme-preset brutalist --config-file brutalist.config.json -o brutalist.svg
merman-cli render ghibli.mmd --theme-preset ghibli -o ghibli.svg
```

A CLI built with PNG or PDF support accepts `-f png -o brutalist.png` or
`-f pdf -o brutalist.pdf` with the same source, preset and configuration. Web supplies SVG;
Playground's PNG/JPEG export uses browser Canvas. Check the loaded artifact rather than
assuming every SDK has native PNG/PDF output.

Font stacks name host fonts and do not embed or download them. Open Sans and Noto Sans SC
are Ghibli's preferred fonts, with a generic sans-serif fallback. Text measurements and font
rendering can differ across hosts. Source/configuration can override individual recipe values.

## Reuse from a SDK

```ts
const recipe = exportThemePreset("ghibli");
const { svg, metadata } = renderSvgResult(source, { version: 3, theme: recipe });
```

Save `recipe` as JSON for CLI `--theme-file` or Playground **Import JSON**. Configuration is
a separate input: the Brutalist recipe alone does not select Classic. The complete workflow
and precedence rules are in [the theme guide](../custom-diagram-themes.md).
