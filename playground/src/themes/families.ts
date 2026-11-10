export const CORE_THEME_SAMPLE_IDS = [
  "flowchart", "sequence", "class", "xychart", "pie", "state", "er", "mindmap",
] as const;

export const THEME_SAMPLE_IDS = [
  ...CORE_THEME_SAMPLE_IDS,
  "gitGraph", "gantt", "kanban", "timeline", "architecture",
  "radar", "quadrantChart", "sankey", "treemap", "requirement", "treeView",
] as const;

export type ThemeSampleId = (typeof THEME_SAMPLE_IDS)[number];

export const THEME_STYLE_IDS = ["reading", "print", "technical", "expressive", "material"] as const;
export type ThemeStyleId = (typeof THEME_STYLE_IDS)[number];

export function isThemeSampleId(value: string | null): value is ThemeSampleId {
  return THEME_SAMPLE_IDS.some((id) => id === value);
}
