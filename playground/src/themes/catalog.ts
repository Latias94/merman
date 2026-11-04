import type { ThemeSampleId } from "./samples";
import atlasRecipe from "./recipes/atlas.theme.json?raw";
import nightfallRecipe from "./recipes/nightfall.theme.json?raw";
import letterpressRecipe from "./recipes/letterpress.theme.json?raw";
import groveRecipe from "./recipes/grove.theme.json?raw";
import atlasFlowchart from "./previews/atlas-flowchart.svg?url";
import atlasSequence from "./previews/atlas-sequence.svg?url";
import atlasClass from "./previews/atlas-class.svg?url";
import atlasXychart from "./previews/atlas-xychart.svg?url";
import atlasPie from "./previews/atlas-pie.svg?url";
import nightfallFlowchart from "./previews/nightfall-flowchart.svg?url";
import nightfallSequence from "./previews/nightfall-sequence.svg?url";
import nightfallClass from "./previews/nightfall-class.svg?url";
import nightfallXychart from "./previews/nightfall-xychart.svg?url";
import nightfallPie from "./previews/nightfall-pie.svg?url";
import letterpressFlowchart from "./previews/letterpress-flowchart.svg?url";
import letterpressSequence from "./previews/letterpress-sequence.svg?url";
import letterpressClass from "./previews/letterpress-class.svg?url";
import letterpressXychart from "./previews/letterpress-xychart.svg?url";
import letterpressPie from "./previews/letterpress-pie.svg?url";
import groveFlowchart from "./previews/grove-flowchart.svg?url";
import groveSequence from "./previews/grove-sequence.svg?url";
import groveClass from "./previews/grove-class.svg?url";
import groveXychart from "./previews/grove-xychart.svg?url";
import grovePie from "./previews/grove-pie.svg?url";

export interface ExplorationTheme {
  readonly id: string;
  readonly name: string;
  readonly appearance: "light" | "dark";
  readonly colors: readonly string[];
  readonly recipeJson: string;
  readonly previews: Readonly<Record<ThemeSampleId, string>>;
}

export const EXPLORATION_THEMES: readonly ExplorationTheme[] = [
  {
    id: "atlas",
    name: "Atlas",
    appearance: "light",
    colors: ["#F8FAFD", "#FFFFFF", "#1F2A3D", "#3067A0", "#287E78", "#996827"],
    recipeJson: atlasRecipe,
    previews: {
      flowchart: atlasFlowchart,
      sequence: atlasSequence,
      class: atlasClass,
      xychart: atlasXychart,
      pie: atlasPie,
    },
  },
  {
    id: "nightfall",
    name: "Nightfall",
    appearance: "dark",
    colors: ["#171D27", "#242D3B", "#E7ECF4", "#96B9D8", "#93BCAF", "#D4B486"],
    recipeJson: nightfallRecipe,
    previews: {
      flowchart: nightfallFlowchart,
      sequence: nightfallSequence,
      class: nightfallClass,
      xychart: nightfallXychart,
      pie: nightfallPie,
    },
  },
  {
    id: "letterpress",
    name: "Letterpress",
    appearance: "light",
    colors: ["#F7F2E8", "#FFFCF6", "#312C2A", "#9A3D2C", "#335A67", "#6A6240"],
    recipeJson: letterpressRecipe,
    previews: {
      flowchart: letterpressFlowchart,
      sequence: letterpressSequence,
      class: letterpressClass,
      xychart: letterpressXychart,
      pie: letterpressPie,
    },
  },
  {
    id: "grove",
    name: "Grove",
    appearance: "light",
    colors: ["#F5F4EA", "#FFFEF8", "#26392F", "#46735C", "#926B30", "#537B7B"],
    recipeJson: groveRecipe,
    previews: {
      flowchart: groveFlowchart,
      sequence: groveSequence,
      class: groveClass,
      xychart: groveXychart,
      pie: grovePie,
    },
  },
];
