import type {
  ThemeDesignV1,
  ThemeDesignTokenV1,
  ThemeDesignReferenceUse,
  ThemeDesignValueDomain,
  ThemeCanvasLayer,
  ThemePaint,
  ThemeRecipeV1,
  ThemeRuleEntry,
  ThemeStyleEntry,
  ThemeStylePatch,
  ThemeStrokePatch,
  ThemeTextStylePatch,
  ThemeTextStyle,
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

/** Edit one authored field without expanding references or changing other bindings. */
export function setThemeDesignPath(
  design: ThemeDesignV1,
  path: readonly (string | number)[],
  value: unknown,
): ThemeDesignV1 {
  if (!path.length) throw new Error("A design property path is required.");
  const edited = structuredClone(design);
  let owner: Record<string | number, unknown> = edited as Record<
    string,
    unknown
  >;
  for (let index = 0; index < path.length - 1; index++) {
    const key = path[index];
    const child = Object.hasOwn(owner, key) ? owner[key] : undefined;
    if (child === undefined)
      Object.defineProperty(owner, key, {
        value: typeof path[index + 1] === "number" ? [] : {},
        enumerable: true,
        configurable: true,
        writable: true,
      });
    else if (
      child === null ||
      typeof child !== "object" ||
      isThemeReference(child)
    )
      throw new Error("Use a local copy before editing a bound value.");
    owner = owner[key] as Record<string | number, unknown>;
  }
  const key = path[path.length - 1];
  if (value === undefined) delete owner[key];
  else
    Object.defineProperty(owner, key, {
      value: structuredClone(value),
      enumerable: true,
      configurable: true,
      writable: true,
    });
  return edited;
}

export function isThemeReference(value: unknown): value is { ref: string } {
  return (
    value !== null &&
    typeof value === "object" &&
    Object.keys(value).length === 1 &&
    Object.hasOwn(value, "ref") &&
    typeof (value as Record<string, unknown>).ref === "string"
  );
}

/** Rename an authored ID and its explicit bindings in one document edit. */
export function renameThemeToken(
  design: ThemeDesignV1,
  from: string,
  to: string,
): ThemeDesignV1 {
  if (!to.length || new TextEncoder().encode(to).length > 128)
    throw new Error("invalid-id");
  if (!design.tokens || !Object.hasOwn(design.tokens, from))
    throw new Error("missing-id");
  if (from === to) return design;
  if (Object.hasOwn(design.tokens, to)) throw new Error("duplicate-id");
  const next = structuredClone(design);
  next.tokens = Object.fromEntries(
    Object.entries(next.tokens!).map(([id, value]) => [
      id === from ? to : id,
      value,
    ]),
  );
  const rewrite = (value: unknown): void => {
    if (isThemeReference(value)) {
      if (value.ref === from) value.ref = to;
    } else if (value !== null && typeof value === "object")
      Object.values(value).forEach(rewrite);
  };
  for (const field of [
    "tokens",
    "styles",
    "rules",
    "palettes",
    "canvas",
    "typography",
  ] as const)
    rewrite(next[field]);
  return next;
}

/** Copy an authored resource; nested bindings intentionally remain bindings. */
export function copyThemeReferenceValue(
  design: ThemeDesignV1,
  token: string,
): unknown {
  return structuredClone(readThemeReferenceValue(design, token));
}

function readThemeReferenceValue(
  design: ThemeDesignV1,
  token: string,
): unknown {
  const visited = new Set<string>();
  let resource = design.tokens?.[token];
  while (resource) {
    if (visited.has(token)) throw new Error("Circular theme reference.");
    visited.add(token);
    if (!isThemeReference(resource.value)) return resource.value;
    token = resource.value.ref;
    resource = design.tokens?.[token];
  }
  throw new Error(`Unknown theme value: ${token}`);
}

export function themeDesignPointer(path: readonly (string | number)[]): string {
  return `/${path.map((part) => String(part).replaceAll("~", "~0").replaceAll("/", "~1")).join("/")}`;
}

/** Consumer constraints originate in Rust; the editor only intersects reported domains. */
export function themeTokenDomain(
  token: string,
  type: ThemeDesignTokenV1["type"],
  references: readonly ThemeDesignReferenceUse[],
): ThemeDesignValueDomain {
  const domains = references
    .filter((use) => use.token === token)
    .map((use) => use.domain);
  const intersection = (key: "choices" | "units") => {
    const lists = domains.flatMap((domain) =>
      domain[key] ? [domain[key]] : [],
    );
    return lists.length
      ? lists[0].filter((value) => lists.every((list) => list.includes(value)))
      : undefined;
  };
  const minima = domains.flatMap((domain) =>
    domain.min === undefined ? [] : [domain.min],
  );
  const maxima = domains.flatMap((domain) =>
    domain.max === undefined ? [] : [domain.max],
  );
  return {
    value_type: type,
    choices: intersection("choices"),
    units: intersection("units"),
    ...(minima.length ? { min: Math.max(...minima) } : {}),
    ...(maxima.length ? { max: Math.min(...maxima) } : {}),
  };
}

/** Filter simple incompatible values using the consumer's reported constraints. */
export function themeTokenFitsDomain(
  design: ThemeDesignV1,
  token: string,
  type: ThemeDesignTokenV1["type"],
  domain?: ThemeDesignValueDomain,
): boolean {
  if (design.tokens?.[token]?.type !== type) return false;
  let value: unknown;
  try {
    value = readThemeReferenceValue(design, token);
  } catch {
    return false;
  }
  if (!domain) return true;
  if (
    domain.choices?.length &&
    typeof value === "string" &&
    !domain.choices.includes(value)
  )
    return false;
  const number =
    typeof value === "number"
      ? value
      : value !== null &&
          typeof value === "object" &&
          "px" in value &&
          typeof value.px === "number"
        ? value.px
        : undefined;
  if (
    number !== undefined &&
    ((domain.min !== undefined && number < domain.min) ||
      (domain.max !== undefined && number > domain.max))
  )
    return false;
  if (
    domain.units?.length &&
    type === "length" &&
    value !== null &&
    typeof value === "object" &&
    "percent" in value &&
    !domain.units.includes("percent")
  )
    return false;
  return true;
}

export function setCompleteTypography<Property extends keyof ThemeTextStyle>(
  recipe: ThemeRecipeV1,
  property: Property,
  value: ThemeTextStyle[Property] | undefined,
): ThemeRecipeV1 {
  if (recipe.kind !== "complete_spec")
    throw new Error("This recipe has no complete typography spec.");
  const edited = structuredClone(recipe);
  const typography = edited.complete_spec.typography ?? {};
  const defaultStyle = typography.default ?? {};
  if (value === undefined) delete defaultStyle[property];
  else defaultStyle[property] = structuredClone(value);
  if (Object.keys(defaultStyle).length) typography.default = defaultStyle;
  else delete typography.default;
  if (Object.keys(typography).length)
    edited.complete_spec.typography = typography;
  else delete edited.complete_spec.typography;
  return edited;
}

export function setThemeCanvasBase(
  recipe: ThemeRecipeV1,
  value: ThemePaint | undefined,
): ThemeRecipeV1 {
  if (recipe.kind === "design")
    return {
      ...recipe,
      design: setThemeDesignPath(recipe.design, ["canvas", "base"], value),
    };
  const edited = structuredClone(recipe);
  const canvas = edited.complete_spec.canvas;
  if (value === undefined) {
    if (canvas) delete canvas.base;
  } else {
    edited.complete_spec.canvas = { ...canvas, base: structuredClone(value) };
  }
  return edited;
}

export function setThemeCanvasLayers(
  recipe: ThemeRecipeV1,
  layers: readonly ThemeCanvasLayer[],
): ThemeRecipeV1 {
  if (recipe.kind !== "complete_spec")
    throw new Error("Canvas layers require a complete theme spec.");
  const edited = structuredClone(recipe);
  if (layers.length)
    edited.complete_spec.canvas = {
      ...edited.complete_spec.canvas,
      layers: structuredClone([...layers]),
    };
  else if (edited.complete_spec.canvas)
    delete edited.complete_spec.canvas.layers;
  return edited;
}

export function removeThemeCanvasBackground(
  recipe: ThemeRecipeV1,
): ThemeRecipeV1 {
  const edited = setThemeCanvasBase(recipe, "transparent");
  const canvas =
    edited.kind === "design"
      ? edited.design.canvas
      : edited.complete_spec.canvas;
  if (canvas) delete canvas.layers;
  return edited;
}

export function restoreThemeCanvas(
  recipe: ThemeRecipeV1,
  previous: ThemeRecipeV1,
): ThemeRecipeV1 {
  if (recipe.kind === "design" && previous.kind === "design")
    return {
      ...recipe,
      design: setThemeDesignPath(
        recipe.design,
        ["canvas"],
        previous.design.canvas,
      ),
    };
  if (recipe.kind !== "complete_spec" || previous.kind !== "complete_spec") {
    throw new Error("Canvas restoration requires the same recipe kind.");
  }
  const edited = structuredClone(recipe);
  if (previous.complete_spec.canvas === undefined)
    delete edited.complete_spec.canvas;
  else
    edited.complete_spec.canvas = structuredClone(
      previous.complete_spec.canvas,
    );
  return edited;
}

export function getThemeStyleEntries(
  recipe: ThemeRecipeV1,
): readonly ThemeStyleEntry[] {
  return recipe.kind === "complete_spec"
    ? (recipe.complete_spec.styles ?? [])
    : [];
}

export function appendThemeRule(
  recipe: ThemeRecipeV1,
  selector: Omit<ThemeRuleEntry, "kind" | "style">,
  style: ThemeStylePatch,
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  if (edited.kind !== "complete_spec")
    throw new Error("Complete rule editing requires a complete spec.");
  const owner = edited.complete_spec;
  owner.styles = [
    ...(owner.styles ?? []),
    {
      ...structuredClone(selector),
      kind: "rule",
      style: structuredClone(style),
    },
  ];
  return edited;
}

export function moveThemeRule(
  recipe: ThemeRecipeV1,
  ruleIndex: number,
  direction: "up" | "down",
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  if (edited.kind !== "complete_spec")
    throw new Error("Complete rule editing requires a complete spec.");
  const owner = edited.complete_spec;
  const styles = owner.styles ? [...owner.styles] : [];
  const targetIndex = direction === "up" ? ruleIndex - 1 : ruleIndex + 1;
  if (!styles[ruleIndex] || targetIndex < 0 || targetIndex >= styles.length)
    return recipe;
  [styles[ruleIndex], styles[targetIndex]] = [
    styles[targetIndex],
    styles[ruleIndex],
  ];
  owner.styles = styles;
  return edited;
}

export function removeThemeRule(
  recipe: ThemeRecipeV1,
  ruleIndex: number,
): ThemeRecipeV1 {
  const edited = structuredClone(recipe);
  if (edited.kind !== "complete_spec")
    throw new Error("Complete rule editing requires a complete spec.");
  const owner = edited.complete_spec;
  if (!owner.styles?.[ruleIndex] || owner.styles[ruleIndex].kind !== "rule")
    return recipe;
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
  if (!rule || rule.kind !== "rule")
    throw new Error("Select an explicit rule to edit.");
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
