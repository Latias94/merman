import risoRecipe from "./recipes/riso.theme.json?raw";
import readingRecipe from "./recipes/reading.theme.json?raw";
import type { ThemeAppearance } from "@mermanjs/web";
import phosphorRecipe from "./recipes/phosphor.theme.json?raw";
import field_notesRecipe from "./recipes/field-notes.theme.json?raw";
import blueprintRecipe from "./recipes/blueprint.theme.json?raw";
import popArtRecipe from "./recipes/pop-art.theme.json?raw";
import lacquerRecipe from "./recipes/lacquer.theme.json?raw";
import paperCutRecipe from "./recipes/paper-cut.theme.json?raw";
import auroraRecipe from "./recipes/aurora.theme.json?raw";
import metroRecipe from "./recipes/metro.theme.json?raw";
import carbonCopyRecipe from "./recipes/carbon-copy.theme.json?raw";
import signalRecipe from "./recipes/signal.theme.json?raw";
import type { ThemeSampleId } from "./samples";
import atlasRecipe from "./recipes/atlas.theme.json?raw";
import nightfallRecipe from "./recipes/nightfall.theme.json?raw";
import letterpressRecipe from "./recipes/letterpress.theme.json?raw";
import groveRecipe from "./recipes/grove.theme.json?raw";
import porcelainRecipe from "./recipes/porcelain.theme.json?raw";
import chalkRecipe from "./recipes/chalk.theme.json?raw";
import prismRecipe from "./recipes/prism.theme.json?raw";
import monographRecipe from "./recipes/monograph.theme.json?raw";
import { CORE_THEME_SAMPLE_IDS, THEME_SAMPLE_IDS, type ThemeStyleId } from "./families";

const previewAssets = import.meta.glob<string>("./previews/*.svg", { query: "?url", import: "default", eager: true });

function previewsFor(themeId: string): Partial<Record<ThemeSampleId, string>> {
  const previews: Partial<Record<ThemeSampleId, string>> = {};
  for (const family of THEME_SAMPLE_IDS) {
    const url = previewAssets[`./previews/${themeId}-${family}.svg`];
    if (url) previews[family] = url;
  }
  return previews;
}

export interface ExplorationTheme {
  readonly id: string;
  readonly name: string;
  readonly appearance: "light" | "dark";
  readonly colors: readonly string[];
  readonly recipeJson: string;
  readonly previews: Readonly<Partial<Record<ThemeSampleId, string>>> | null;
  readonly families: readonly ThemeSampleId[];
  readonly featuredSample?: ThemeSampleId;
  readonly sampleNotes?: readonly {
    readonly familyId: ThemeSampleId;
    readonly appearance: ThemeAppearance;
    readonly messageKey: string;
  }[];
  readonly styles: readonly ThemeStyleId[];
  readonly appearances?: readonly {
    readonly appearance: ThemeAppearance;
    readonly colors: readonly string[];
    readonly previews?: Readonly<Partial<Record<ThemeSampleId, string>>>;
  }[];
}

export function explorationAppearance(theme: ExplorationTheme, appearance: ThemeAppearance) {
  return theme.appearances?.find((entry) => entry.appearance === appearance) ??
    { appearance: theme.appearance, colors: theme.colors };
}

export const EXPLORATION_THEMES: readonly ExplorationTheme[] = [
  {
    id: "riso", name: "Riso", appearance: "light",
    families: CORE_THEME_SAMPLE_IDS, styles: ["print", "expressive"],
    colors: ["#F6F0E4", "#233F78", "#C64037"],
    recipeJson: risoRecipe,
    previews: previewsFor("riso"),
  },
  {
    id: "pop-art", name: "Pop Art", appearance: "light",
    families: CORE_THEME_SAMPLE_IDS, styles: ["expressive", "print"],
    colors: ["#FFF9E8", "#FFE500", "#F32888", "#31D5E8"],
    recipeJson: popArtRecipe,
    previews: previewsFor("pop-art"),
  },
  {
    id: "metro", name: "Metro", appearance: "dark",
    families: [...CORE_THEME_SAMPLE_IDS, "gitGraph"], featuredSample: "gitGraph", styles: ["technical", "expressive"],
    colors: ["#071B2C", "#FFE347", "#63DCE5", "#F4F3E9"],
    recipeJson: metroRecipe,
    previews: previewsFor("metro"),
  },
  {
    id: "carbon-copy", name: "Carbon Copy", appearance: "light",
    families: [...CORE_THEME_SAMPLE_IDS, "kanban"], featuredSample: "kanban", styles: ["print", "material"],
    colors: ["#F2EDDF", "#4B2C63", "#76508E", "#A53B32"],
    recipeJson: carbonCopyRecipe,
    previews: previewsFor("carbon-copy"),
  },
  {
    id: "signal", name: "Signal", appearance: "light",
    families: [...CORE_THEME_SAMPLE_IDS, "gantt"], featuredSample: "gantt", styles: ["technical", "expressive"],
    colors: ["#F3F0E5", "#22251F", "#F7D938", "#E66B2D"],
    recipeJson: signalRecipe,
    previews: previewsFor("signal"),
  },
  {
    id: "field-notes", name: "Field Notes", appearance: "light",
    families: [...CORE_THEME_SAMPLE_IDS, "timeline"], featuredSample: "timeline", styles: ["print", "material"],
    colors: ["#F8F3E5", "#244B39", "#FFF4D7", "#A13F32"],
    recipeJson: field_notesRecipe,
    previews: previewsFor("field-notes"),
  },
  {
    id: "lacquer", name: "Lacquer", appearance: "dark",
    families: CORE_THEME_SAMPLE_IDS, styles: ["material"],
    colors: ["#110E12", "#3D1824", "#D8BC82", "#F2E9D6"],
    recipeJson: lacquerRecipe,
    previews: previewsFor("lacquer"),
  },
  {
    id: "paper-cut", name: "Paper Cut", appearance: "light",
    families: CORE_THEME_SAMPLE_IDS, styles: ["material", "expressive"],
    colors: ["#FBF8F0", "#E9F2E9", "#EB9C88", "#7EB4AA"],
    recipeJson: paperCutRecipe,
    previews: previewsFor("paper-cut"),
  },
  {
    id: "aurora", name: "Aurora", appearance: "dark",
    families: CORE_THEME_SAMPLE_IDS, styles: ["material", "expressive"],
    colors: ["#0B1028", "#163442", "#78E8CA", "#BB9CFF"],
    recipeJson: auroraRecipe,
    previews: previewsFor("aurora"),
  },
  {
    id: "phosphor", name: "Phosphor", appearance: "dark",
    families: CORE_THEME_SAMPLE_IDS, styles: ["technical", "material"],
    colors: ["#0C1210", "#17211C", "#D8FB8A", "#F1C577"],
    recipeJson: phosphorRecipe,
    previews: previewsFor("phosphor"),
  },
  {
    id: "letterpress",
    families: CORE_THEME_SAMPLE_IDS, styles: ["print", "reading"],
    name: "Letterpress",
    appearance: "light",
    colors: ["#F7F2E8", "#FFFCF6", "#312C2A", "#9A3D2C", "#335A67", "#6A6240"],
    recipeJson: letterpressRecipe,
    previews: previewsFor("letterpress"),
  },
  {
    id: "grove",
    families: CORE_THEME_SAMPLE_IDS, styles: ["reading", "material"],
    name: "Grove",
    appearance: "light",
    colors: ["#F5F4EA", "#FFFEF8", "#26392F", "#46735C", "#926B30", "#537B7B"],
    recipeJson: groveRecipe,
    previews: previewsFor("grove"),
  },
  {
    id: "reading", name: "Reading", appearance: "light",
    families: CORE_THEME_SAMPLE_IDS, styles: ["reading"],
    colors: ["#F6F7F9", "#243246", "#315EAA", "#2F7E73"],
    recipeJson: readingRecipe,
    appearances: [
      { appearance: "light", colors: ["#F6F7F9", "#243246", "#315EAA", "#2F7E73"], previews: previewsFor("reading-light") },
      { appearance: "dark", colors: ["#171C24", "#E3EAF3", "#94B9F2", "#76BDAE"], previews: previewsFor("reading-dark") },
    ],
    previews: null,
  },
  {
    id: "atlas",
    families: CORE_THEME_SAMPLE_IDS, styles: ["reading", "technical"],
    name: "Atlas",
    appearance: "light",
    colors: ["#F8FAFD", "#FFFFFF", "#1F2A3D", "#3067A0", "#287E78", "#996827"],
    recipeJson: atlasRecipe,
    previews: previewsFor("atlas"),
  },
  {
    id: "nightfall",
    families: CORE_THEME_SAMPLE_IDS, styles: ["reading"],
    name: "Nightfall",
    appearance: "dark",
    colors: ["#171D27", "#242D3B", "#E7ECF4", "#96B9D8", "#93BCAF", "#D4B486"],
    recipeJson: nightfallRecipe,
    previews: previewsFor("nightfall"),
  },
  {
    id: "blueprint", name: "Blueprint", appearance: "dark",
    families: [...CORE_THEME_SAMPLE_IDS, "architecture"], featuredSample: "architecture", styles: ["technical", "print"],
    colors: ["#12365B", "#173E66", "#E3F3FA", "#7AE6EC"],
    recipeJson: blueprintRecipe,
    previews: previewsFor("blueprint"),
  },
  {
    id: "porcelain", name: "Porcelain", appearance: "light",
    families: CORE_THEME_SAMPLE_IDS, styles: ["material", "reading"],
    colors: ["#F4F1E9", "#FFFEFA", "#2E5185", "#A5523B", "#477C78"],
    recipeJson: porcelainRecipe, previews: previewsFor("porcelain"),
  },
  {
    id: "chalk", name: "Chalk", appearance: "dark",
    families: CORE_THEME_SAMPLE_IDS, styles: ["reading", "expressive"],
    colors: ["#242C2B", "#F3EEDC", "#A6C7E2", "#F0D88B", "#C8BAE0"],
    recipeJson: chalkRecipe, previews: previewsFor("chalk"),
  },
  {
    id: "prism", name: "Prism", appearance: "light",
    families: [...CORE_THEME_SAMPLE_IDS, "radar", "quadrantChart", "sankey", "treemap"],
    featuredSample: "radar", styles: ["reading", "technical"],
    sampleNotes: [
      { familyId: "radar", appearance: "dark", messageKey: "themeLibrary.sampleNotes.radarDark" },
    ],
    colors: ["#F5F7FB", "#223047", "#2864DC", "#B95D12", "#137F75", "#8455C7"],
    recipeJson: prismRecipe,
    appearances: [
      { appearance: "light", colors: ["#F5F7FB", "#223047", "#2864DC", "#B95D12", "#137F75", "#8455C7"], previews: previewsFor("prism-light") },
      { appearance: "dark", colors: ["#171F2D", "#EAF0F8", "#77ABFF", "#F7AF6A", "#58CFBA", "#B99AF0"], previews: previewsFor("prism-dark") },
    ],
    previews: null,
  },
  {
    id: "monograph", name: "Monograph", appearance: "light",
    families: [...CORE_THEME_SAMPLE_IDS, "requirement", "treeView"],
    featuredSample: "requirement", styles: ["print", "reading", "technical"],
    colors: ["#FFFFFF", "#161616", "#555555", "#303030", "#EFEFEF"],
    recipeJson: monographRecipe, previews: previewsFor("monograph"),
  },
];
