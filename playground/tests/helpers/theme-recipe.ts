import { readFile } from "node:fs/promises";

const CORE_RECIPE_IDS = new Set(["paper-cut", "carbon-copy", "pop-art"]);

export function readSavedThemeRecipe(id: string): Promise<string> {
  const directory = CORE_RECIPE_IDS.has(id)
    ? "../../../crates/merman-render/assets/themes/"
    : "../../src/themes/recipes/";
  return readFile(new URL(`${directory}${id}.theme.json`, import.meta.url), "utf8");
}
