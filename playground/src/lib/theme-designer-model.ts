import type {
  ThemeDesignReferenceUse,
  ThemeDesignTokenV1,
  ThemeDesignV1,
} from "@mermanjs/web";

interface VisualDestination {
  readonly path: string;
  readonly segments: readonly string[];
}

export type ThemeDesignerDestination =
  | (VisualDestination & { readonly kind: "token"; readonly tokenId: string })
  | (VisualDestination & { readonly kind: "style"; readonly target: string })
  | (VisualDestination & {
      readonly kind: "rule";
      readonly index: number;
      readonly target: string;
    })
  | (VisualDestination & { readonly kind: "canvas"; readonly index?: number })
  | (VisualDestination & { readonly kind: "typography" })
  | {
      readonly kind: "json";
      readonly path: string;
      readonly segments: readonly string[] | null;
      readonly reason: "invalid-pointer" | "missing" | "unsupported";
    };

export interface ThemeBindingUse {
  readonly path: string;
  /** The token named by inspection, rather than the owner of the authored path. */
  readonly token: string;
  readonly ownerToken?: string;
  /** Intermediate shared values on one deterministic shortest route. */
  readonly via: readonly string[];
  readonly destination: ThemeDesignerDestination;
}

export interface ThemeSettingUses {
  readonly direct: readonly ThemeBindingUse[];
  readonly dependencies: readonly ThemeBindingUse[];
  readonly downstream: readonly ThemeBindingUse[];
}

export interface ThemeDesignerSetting {
  readonly id: string;
  readonly label: string;
  readonly type: ThemeDesignTokenV1["type"];
  readonly group?: string;
  readonly description?: string;
  /** The original authored value, never a resolved copy. */
  readonly token: ThemeDesignTokenV1;
  /** Undefined means inspection is unavailable; empty lists mean no declared uses. */
  readonly uses?: ThemeSettingUses;
}

export interface ThemeDesignerGroup {
  /** Null represents absent metadata, distinct from an explicitly empty group. */
  readonly id: string | null;
  readonly label?: string;
  readonly settings: readonly ThemeDesignerSetting[];
}

export interface ThemeDesignerModel {
  readonly settings: readonly ThemeDesignerSetting[];
  readonly groups: readonly ThemeDesignerGroup[];
  readonly inspectionAvailable: boolean;
}

/** Decode a JSON Pointer without accepting malformed escapes or URI fragments. */
export function parseThemeDesignPointer(path: string): string[] | null {
  if (path === "") return [];
  if (!path.startsWith("/") || /~(?![01])/.test(path)) return null;
  return path
    .slice(1)
    .split("/")
    .map((part) =>
      part.replace(/~[01]/g, (escape) => (escape === "~1" ? "/" : "~")),
    );
}

/** Select an existing inspector without following a reference or changing its scope. */
export function classifyThemeDesignDestination(
  design: ThemeDesignV1,
  path: string,
): ThemeDesignerDestination {
  const segments = parseThemeDesignPointer(path);
  if (!segments)
    return { kind: "json", path, segments, reason: "invalid-pointer" };
  const json = (reason: "missing" | "unsupported"): ThemeDesignerDestination => ({
    kind: "json",
    path,
    segments,
    reason,
  });
  let value: unknown = design;
  for (const segment of segments) {
    if (
      value === null ||
      typeof value !== "object" ||
      !Object.hasOwn(value, segment)
    )
      return json("missing");
    // Effect graphs have no nested visual controls. The exact authored location remains available in JSON.
    if (
      Object.hasOwn(value, "kind") &&
      (value as { kind?: unknown }).kind === "graph"
    )
      return json("unsupported");
    value = (value as Record<string, unknown>)[segment];
  }
  const [root, owner, field, item] = segments;
  const base = { path, segments };
  if (root === "tokens" && segments.length >= 3 && field === "value")
    return { ...base, kind: "token", tokenId: owner };
  if (root === "styles" && segments.length >= 3)
    return { ...base, kind: "style", target: owner };
  if (root === "rules" && segments.length >= 4 && field === "style") {
    const index = Number(owner);
    return {
      ...base,
      kind: "rule",
      index,
      target: design.rules![index].target,
    };
  }
  if (root === "canvas") {
    if (owner === "base" || owner === "bleed")
      return { ...base, kind: "canvas" };
    if (
      owner === "layers" &&
      segments.length >= 4 &&
      ["paint", "opacity", "offset_x", "offset_y", "blend_mode"].includes(item)
    )
      return { ...base, kind: "canvas", index: Number(field) };
  }
  if (root === "typography" && owner === "default")
    return { ...base, kind: "typography" };
  return json("unsupported");
}

function compare(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function dependencyOwner(path: string): string | undefined {
  const segments = parseThemeDesignPointer(path);
  return segments?.[0] === "tokens" && segments[2] === "value"
    ? segments[1]
    : undefined;
}

/** Project authored settings and Rust inspection only; this neither validates nor materializes. */
export function buildThemeDesignerModel(
  design: ThemeDesignV1,
  references: readonly ThemeDesignReferenceUse[] | undefined,
): ThemeDesignerModel {
  const byToken = new Map<string, ThemeBindingUse[]>();
  const ordered = [...(references ?? [])].sort(
    (left, right) =>
      compare(left.path, right.path) || compare(left.token, right.token),
  );
  for (const use of ordered) {
    const rows = byToken.get(use.token) ?? [];
    if (rows.some((row) => row.path === use.path)) continue;
    rows.push({
      path: use.path,
      token: use.token,
      ownerToken: dependencyOwner(use.path),
      via: [],
      destination: classifyThemeDesignDestination(design, use.path),
    });
    byToken.set(use.token, rows);
  }
  function usesFor(id: string): ThemeSettingUses {
    const immediate = byToken.get(id) ?? [];
    const direct = immediate.filter((row) => row.ownerToken === undefined);
    const dependencies = immediate.filter((row) => row.ownerToken !== undefined);
    const seenPaths = new Set(direct.map((row) => row.path));
    const visited = new Set([id]);
    const pending = [{ id, via: [] as readonly string[] }];
    const downstream: ThemeBindingUse[] = [];
    for (let cursor = 0; cursor < pending.length; cursor++) {
      const current = pending[cursor];
      for (const row of byToken.get(current.id) ?? []) {
        if (row.ownerToken !== undefined) {
          if (
            !visited.has(row.ownerToken) &&
            design.tokens &&
            Object.hasOwn(design.tokens, row.ownerToken)
          ) {
            visited.add(row.ownerToken);
            pending.push({
              id: row.ownerToken,
              via: [...current.via, row.ownerToken],
            });
          }
        } else if (current.id !== id && !seenPaths.has(row.path)) {
          seenPaths.add(row.path);
          downstream.push({ ...row, via: current.via });
        }
      }
    }
    downstream.sort((left, right) => compare(left.path, right.path));
    return { direct, dependencies, downstream };
  }
  const settings = Object.entries(design.tokens ?? {})
    .map(
      ([id, token]): ThemeDesignerSetting => ({
        id,
        token,
        type: token.type,
        label: token.label ?? id,
        group: token.group,
        description: token.description,
        ...(references === undefined ? {} : { uses: usesFor(id) }),
      }),
    )
    .sort(
      (left, right) =>
        compare(left.label, right.label) || compare(left.id, right.id),
    );
  const groups = new Map<string | null, ThemeDesignerSetting[]>();
  for (const setting of settings) {
    const group = setting.group ?? null;
    const items = groups.get(group) ?? [];
    items.push(setting);
    groups.set(group, items);
  }
  return {
    settings,
    groups: [...groups]
      .sort(([left], [right]) =>
        left === null ? 1 : right === null ? -1 : compare(left, right),
      )
      .map(([id, items]) => ({ id, label: id ?? undefined, settings: items })),
    inspectionAvailable: references !== undefined,
  };
}

/** Search all authored settings, including custom groups and values without metadata. */
export function searchThemeSettings(
  model: ThemeDesignerModel,
  query: string,
): readonly ThemeDesignerSetting[] {
  const search = query.trim().toLowerCase();
  if (!search) return model.settings;
  return model.settings.filter((setting) =>
    [
      setting.id,
      setting.label,
      setting.group,
      setting.description,
      setting.type,
    ].some((value) => value?.toLowerCase().includes(search)),
  );
}
