import risoRecipe from "./recipes/riso.theme.json?raw";
import riso_flowchart from "./previews/riso-flowchart.svg?url";
import riso_sequence from "./previews/riso-sequence.svg?url";
import riso_class from "./previews/riso-class.svg?url";
import riso_xychart from "./previews/riso-xychart.svg?url";
import riso_pie from "./previews/riso-pie.svg?url";
import phosphorRecipe from "./recipes/phosphor.theme.json?raw";
import phosphor_flowchart from "./previews/phosphor-flowchart.svg?url";
import phosphor_sequence from "./previews/phosphor-sequence.svg?url";
import phosphor_class from "./previews/phosphor-class.svg?url";
import phosphor_xychart from "./previews/phosphor-xychart.svg?url";
import phosphor_pie from "./previews/phosphor-pie.svg?url";
import field_notesRecipe from "./recipes/field-notes.theme.json?raw";
import field_notes_flowchart from "./previews/field-notes-flowchart.svg?url";
import field_notes_sequence from "./previews/field-notes-sequence.svg?url";
import field_notes_class from "./previews/field-notes-class.svg?url";
import field_notes_xychart from "./previews/field-notes-xychart.svg?url";
import field_notes_pie from "./previews/field-notes-pie.svg?url";
import blueprintRecipe from "./recipes/blueprint.theme.json?raw";
import blueprintFlowchart from "./previews/blueprint-flowchart.svg?url";
import blueprintSequence from "./previews/blueprint-sequence.svg?url";
import blueprintClass from "./previews/blueprint-class.svg?url";
import blueprintXychart from "./previews/blueprint-xychart.svg?url";
import blueprintPie from "./previews/blueprint-pie.svg?url";
import popArtRecipe from "./recipes/pop-art.theme.json?raw";
import popArtFlowchart from "./previews/pop-art-flowchart.svg?url";
import popArtSequence from "./previews/pop-art-sequence.svg?url";
import popArtClass from "./previews/pop-art-class.svg?url";
import popArtXychart from "./previews/pop-art-xychart.svg?url";
import popArtPie from "./previews/pop-art-pie.svg?url";
import lacquerRecipe from "./recipes/lacquer.theme.json?raw";
import lacquerFlowchart from "./previews/lacquer-flowchart.svg?url";
import lacquerSequence from "./previews/lacquer-sequence.svg?url";
import lacquerClass from "./previews/lacquer-class.svg?url";
import lacquerXychart from "./previews/lacquer-xychart.svg?url";
import lacquerPie from "./previews/lacquer-pie.svg?url";
import paperCutRecipe from "./recipes/paper-cut.theme.json?raw";
import paperCutFlowchart from "./previews/paper-cut-flowchart.svg?url";
import paperCutSequence from "./previews/paper-cut-sequence.svg?url";
import paperCutClass from "./previews/paper-cut-class.svg?url";
import paperCutXychart from "./previews/paper-cut-xychart.svg?url";
import paperCutPie from "./previews/paper-cut-pie.svg?url";
import auroraRecipe from "./recipes/aurora.theme.json?raw";
import auroraFlowchart from "./previews/aurora-flowchart.svg?url";
import auroraSequence from "./previews/aurora-sequence.svg?url";
import auroraClass from "./previews/aurora-class.svg?url";
import auroraXychart from "./previews/aurora-xychart.svg?url";
import auroraPie from "./previews/aurora-pie.svg?url";
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
    id: "riso", name: "Riso", appearance: "light",
    colors: ["#F6F0E4", "#233F78", "#C64037"],
    recipeJson: risoRecipe,
    previews: { flowchart: riso_flowchart, sequence: riso_sequence, class: riso_class, xychart: riso_xychart, pie: riso_pie },
  },
  {
    id: "pop-art", name: "Pop Art", appearance: "light",
    colors: ["#FFF9E8", "#FFE500", "#F32888", "#31D5E8"],
    recipeJson: popArtRecipe,
    previews: { flowchart: popArtFlowchart, sequence: popArtSequence, class: popArtClass, xychart: popArtXychart, pie: popArtPie },
  },
  {
    id: "field-notes", name: "Field Notes", appearance: "light",
    colors: ["#F8F3E5", "#244B39", "#FFF4D7", "#A13F32"],
    recipeJson: field_notesRecipe,
    previews: { flowchart: field_notes_flowchart, sequence: field_notes_sequence, class: field_notes_class, xychart: field_notes_xychart, pie: field_notes_pie },
  },
  {
    id: "lacquer", name: "Lacquer", appearance: "dark",
    colors: ["#110E12", "#3D1824", "#D8BC82", "#F2E9D6"],
    recipeJson: lacquerRecipe,
    previews: { flowchart: lacquerFlowchart, sequence: lacquerSequence, class: lacquerClass, xychart: lacquerXychart, pie: lacquerPie },
  },
  {
    id: "paper-cut", name: "Paper Cut", appearance: "light",
    colors: ["#FBF8F0", "#E9F2E9", "#EB9C88", "#7EB4AA"],
    recipeJson: paperCutRecipe,
    previews: { flowchart: paperCutFlowchart, sequence: paperCutSequence, class: paperCutClass, xychart: paperCutXychart, pie: paperCutPie },
  },
  {
    id: "aurora", name: "Aurora", appearance: "dark",
    colors: ["#0B1028", "#163442", "#78E8CA", "#BB9CFF"],
    recipeJson: auroraRecipe,
    previews: { flowchart: auroraFlowchart, sequence: auroraSequence, class: auroraClass, xychart: auroraXychart, pie: auroraPie },
  },
  {
    id: "phosphor", name: "Phosphor", appearance: "dark",
    colors: ["#0C1210", "#17211C", "#D8FB8A", "#F1C577"],
    recipeJson: phosphorRecipe,
    previews: { flowchart: phosphor_flowchart, sequence: phosphor_sequence, class: phosphor_class, xychart: phosphor_xychart, pie: phosphor_pie },
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
    id: "blueprint", name: "Blueprint", appearance: "dark",
    colors: ["#12365B", "#173E66", "#E3F3FA", "#7AE6EC"],
    recipeJson: blueprintRecipe,
    previews: { flowchart: blueprintFlowchart, sequence: blueprintSequence, class: blueprintClass, xychart: blueprintXychart, pie: blueprintPie },
  },
];
