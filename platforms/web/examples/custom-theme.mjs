/**
 * Customize explicit semantic roles in an editable design or complete snapshot.
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
  const recipe = structuredClone(exported);
  const authored = recipe.kind === "design";
  const spec = authored ? recipe.design : recipe.complete_spec;
  // Retain layers and bleed when changing the base of a future layered preset.
  spec.canvas = { ...spec.canvas, base: colors.background };
  const rules = authored ? (spec.rules ??= []) : (spec.styles ??= []);
  const tag = authored ? {} : { kind: "rule" };
  rules.push(
    { ...tag, target: "node", style: { stroke: { paint: colors.nodeBorder } } },
    {
      ...tag, target: "node", family: "class",
      style: { fill: colors.classFill },
    },
  );
  return recipe;
}
