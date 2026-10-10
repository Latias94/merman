/**
 * Customize explicit color roles in an exported complete recipe.
 *
 * This example changes the canvas, Node borders, and Class Node fill. Actor, edge,
 * marker, and chart-series colors remain independent. It is not a global palette recolorer.
 * Source-owned styles still take precedence over these theme rules.
 *
 * @param {import('../src/public-types.js').ThemeRecipeV1} exported
 * @param {{ background: string, nodeBorder: string, classFill?: string | null }} colors
 * @returns {import('../src/public-types.js').ThemeRecipeV1}
 */
export function customizeNodeColors(exported, colors) {
  if (exported.kind !== "complete_spec") {
    throw new Error("This example expects an exported complete specification.");
  }
  const recipe = structuredClone(exported);
  const spec = recipe.complete_spec;
  // Retain layers and bleed when changing the base of a future layered preset.
  spec.canvas = { ...spec.canvas, base: colors.background };
  spec.styles ??= [];
  spec.styles.push(
    { kind: "rule", target: "node", style: { stroke: { paint: colors.nodeBorder } } },
    {
      kind: "rule", target: "node", family: "class",
      style: { fill: colors.classFill },
    },
  );
  return recipe;
}
