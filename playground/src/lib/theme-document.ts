import type {
  ThemeAuthoringTypographyV1,
  ThemePaint,
  ThemeRecipeV1,
  ThemeRuleEntry,
  ThemeStyleEntry,
  ThemeStylePatch,
  ThemeStrokePatch,
  ThemeTextStylePatch,
  ThemeTokensV1,
} from "@mermanjs/web";

export interface ValidatedThemeDocument {
  readonly document: ThemeRecipeV1;
  readonly recipeJson: string;
}

export function validateThemeDocument(
  rawJson: string,
  validate: (rawJson: string) => string,
): ValidatedThemeDocument {
  const recipeJson = validate(rawJson);
  return { document: JSON.parse(rawJson) as ThemeRecipeV1, recipeJson };
}

export function setDefinitionToken<
  Token extends Exclude<keyof ThemeTokensV1, "typography">,
>(
  recipe: ThemeRecipeV1,
  token: Token,
  value: ThemeTokensV1[Token],
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  if (edited.kind !== "definition") throw new Error("This recipe has no definition tokens.");
  if (value === undefined) delete edited.definition.tokens[token];
  else edited.definition.tokens[token] = structuredClone(value);
  return edited;
}

export function setDefinitionTypography<
  Property extends keyof ThemeAuthoringTypographyV1,
>(
  recipe: ThemeRecipeV1,
  property: Property,
  value: ThemeAuthoringTypographyV1[Property],
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  if (edited.kind !== "definition") throw new Error("This recipe has no definition tokens.");
  const typography = edited.definition.tokens.typography;
  if (value === undefined) {
    if (typography) delete typography[property];
  } else {
    const next = typography ?? {};
    next[property] = structuredClone(value);
    edited.definition.tokens.typography = next;
  }
  return edited;
}

export function setThemeCanvasBase(
  recipe: ThemeRecipeV1,
  value: ThemePaint | undefined,
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  if (edited.kind !== "complete_spec") throw new Error("This recipe has no complete canvas.");
  const canvas = edited.complete_spec.canvas;
  if (value === undefined) {
    if (canvas) delete canvas.base;
  } else {
    edited.complete_spec.canvas = { ...canvas, base: structuredClone(value) };
  }
  return edited;
}

export function getThemeStyleEntries(recipe: ThemeRecipeV1): readonly ThemeStyleEntry[] {
  return (recipe.kind === "definition" ? recipe.definition : recipe.complete_spec).styles ?? [];
}

export function appendThemeRule(
  recipe: ThemeRecipeV1,
  selector: Omit<ThemeRuleEntry, "kind" | "style">,
  style: ThemeStylePatch,
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  const owner = edited.kind === "definition" ? edited.definition : edited.complete_spec;
  owner.styles = [...owner.styles ?? [], {
    ...structuredClone(selector), kind: "rule", style: structuredClone(style),
  }];
  return edited;
}

export interface ThemeRuleFacetValues {
  fill: ThemeStylePatch["fill"];
  opacity: ThemeStylePatch["opacity"];
  fill_opacity: ThemeStylePatch["fill_opacity"];
  radius: ThemeStylePatch["radius"];
  padding: ThemeStylePatch["padding"];
  effect: ThemeStylePatch["effect"];
  "stroke.paint": ThemeStrokePatch["paint"];
  "stroke.width": ThemeStrokePatch["width"];
  "stroke.dasharray": ThemeStrokePatch["dasharray"];
  "stroke.linecap": ThemeStrokePatch["linecap"];
  "stroke.linejoin": ThemeStrokePatch["linejoin"];
  "stroke.opacity": ThemeStrokePatch["opacity"];
  "typography.font_size_px": ThemeTextStylePatch["font_size_px"];
  "typography.font_weight": ThemeTextStylePatch["font_weight"];
  "typography.font_stack": ThemeTextStylePatch["font_stack"];
}

export function setThemeRuleFacet<Facet extends keyof ThemeRuleFacetValues>(
  recipe: ThemeRecipeV1,
  ruleIndex: number,
  facet: Facet,
  value: ThemeRuleFacetValues[Facet],
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  const rule = getThemeStyleEntries(edited)[ruleIndex];
  if (!rule || rule.kind !== "rule") throw new Error("Select an explicit rule to edit.");
  const [group, property] = facet.split(".");
  const style = rule.style as Record<string, unknown>;
  if (property) {
    const current = style[group] as Record<string, unknown> | undefined;
    if (value === undefined) {
      if (current) delete current[property];
    } else {
      const nested = current ?? {};
      nested[property] = structuredClone(value);
      style[group] = nested;
    }
  } else if (value === undefined) {
    delete style[group];
  } else {
    style[group] = structuredClone(value);
  }
  return edited;
}
