import type {
  ThemeAuthoringTypographyV1,
  ThemeCanvasLayer,
  ThemePaint,
  ThemeRecipeV1,
  ThemeRuleEntry,
  ThemeStyleEntry,
  ThemeStylePatch,
  ThemeStrokePatch,
  ThemeTextStylePatch,
  ThemeTextStyle,
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

export function setCompleteTypography<
  Property extends keyof ThemeTextStyle,
>(
  recipe: ThemeRecipeV1,
  property: Property,
  value: ThemeTextStyle[Property] | undefined,
): ThemeRecipeV1 {
  if (recipe.kind !== "complete_spec") throw new Error("This recipe has no complete typography spec.");
  const edited = structuredClone(recipe);
  const typography = edited.complete_spec.typography ?? {};
  const defaultStyle = typography.default ?? {};
  if (value === undefined) delete defaultStyle[property];
  else defaultStyle[property] = structuredClone(value);
  if (Object.keys(defaultStyle).length) typography.default = defaultStyle;
  else delete typography.default;
  if (Object.keys(typography).length) edited.complete_spec.typography = typography;
  else delete edited.complete_spec.typography;
  return edited;
}

export function setThemeCanvasBase(
  recipe: ThemeRecipeV1,
  value: ThemePaint | undefined,
): ThemeRecipeV1 {
  if (recipe.kind === "definition") {
    if (value !== undefined && typeof value !== "string") throw new Error("Definition canvas tokens require a color.");
    return setDefinitionToken(recipe, "canvas", value);
  }
  const edited = structuredClone(recipe);
  const canvas = edited.complete_spec.canvas;
  if (value === undefined) {
    if (canvas) delete canvas.base;
  } else {
    edited.complete_spec.canvas = { ...canvas, base: structuredClone(value) };
  }
  return edited;
}

export function setThemeCanvasLayers(recipe: ThemeRecipeV1, layers: readonly ThemeCanvasLayer[]): ThemeRecipeV1 {
  if (recipe.kind !== "complete_spec") throw new Error("Canvas layers require a complete theme spec.");
  const edited = structuredClone(recipe);
  if (layers.length) edited.complete_spec.canvas = { ...edited.complete_spec.canvas, layers: structuredClone([...layers]) };
  else if (edited.complete_spec.canvas) delete edited.complete_spec.canvas.layers;
  return edited;
}

export function removeThemeCanvasBackground(recipe: ThemeRecipeV1): ThemeRecipeV1 {
  const edited = setThemeCanvasBase(recipe, "transparent");
  if (edited.kind === "complete_spec" && edited.complete_spec.canvas) {
    delete edited.complete_spec.canvas.layers;
  }
  return edited;
}

export function restoreThemeCanvas(recipe: ThemeRecipeV1, previous: ThemeRecipeV1): ThemeRecipeV1 {
  if (recipe.kind === "definition" && previous.kind === "definition") {
    return setDefinitionToken(recipe, "canvas", previous.definition.tokens.canvas);
  }
  if (recipe.kind !== "complete_spec" || previous.kind !== "complete_spec") {
    throw new Error("Canvas restoration requires the same recipe kind.");
  }
  const edited = structuredClone(recipe);
  if (previous.complete_spec.canvas === undefined) delete edited.complete_spec.canvas;
  else edited.complete_spec.canvas = structuredClone(previous.complete_spec.canvas);
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

export function moveThemeRule(
  recipe: ThemeRecipeV1,
  ruleIndex: number,
  direction: "up" | "down",
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  const owner = edited.kind === "definition" ? edited.definition : edited.complete_spec;
  const styles = owner.styles ? [...owner.styles] : [];
  const targetIndex = direction === "up" ? ruleIndex - 1 : ruleIndex + 1;
  if (!styles[ruleIndex] || targetIndex < 0 || targetIndex >= styles.length) return recipe;
  [styles[ruleIndex], styles[targetIndex]] = [styles[targetIndex], styles[ruleIndex]];
  owner.styles = styles;
  return edited;
}

export function removeThemeRule(recipe: ThemeRecipeV1, ruleIndex: number): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  const owner = edited.kind === "definition" ? edited.definition : edited.complete_spec;
  if (!owner.styles?.[ruleIndex] || owner.styles[ruleIndex].kind !== "rule") return recipe;
  owner.styles = owner.styles.filter((_, index) => index !== ruleIndex);
  if (!owner.styles.length) delete owner.styles;
  return edited;
}

export function setThemeRuleSelector(
  recipe: ThemeRecipeV1,
  ruleIndex: number,
  selector: { target?: string; family?: string; variant?: string },
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  const entry = getThemeStyleEntries(edited)[ruleIndex];
  if (!entry || entry.kind !== "rule") return recipe;
  if (selector.target !== undefined) entry.target = selector.target;
  for (const property of ["family", "variant"] as const) {
    if (!(property in selector)) continue;
    const value = selector[property];
    if (value === undefined || value === "") delete entry[property];
    else entry[property] = value;
  }
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
