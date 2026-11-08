import {
  BINDING_OPTIONS_SCHEMA_VERSION,
  type HostTextMeasurerSvgBindingOptions,
  type ThemePresetCatalogEntry,
  type ThemeCatalog,
  type ThemeAppearance,
} from "@mermanjs/web";

import type { DiagramFont } from "../lib/diagram-font.ts";
import { buildMermaidOperationInput, type InjectedMermaidConfigSpan, type MermaidConfigObject } from "../lib/mermaid-config.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "../lib/workspace-snapshot.ts";
import type { RealmViewport } from "./realm/channel-protocol.ts";
import { projectError, type ErrorProjection } from "./error-projection.ts";
import { resolveThemeSelection, type ThemeAppearancePreference, type ThemeAppearanceSupport } from "../lib/theme-appearance.ts";

export const MERMAN_SVG_PIPELINES = [
  "parity",
  "readable",
  "resvg-safe",
] as const;
export type MermanSvgPipeline = (typeof MERMAN_SVG_PIPELINES)[number];
export type MermanTextMeasurementMode = "browser" | "headless";

export interface MermanLayoutEnvironment {
  readonly containerHeight: number;
  readonly containerWidth: number;
  readonly screenAvailableWidth?: number;
}

export interface MermanOperationOptions {
  readonly diagramFont?: DiagramFont;
  readonly layoutEnvironment?: MermanLayoutEnvironment;
  readonly themePresetId?: string | null;
  readonly themeRecipeJson?: string | null;
  readonly themeAppearance?: ThemeAppearance | null;
  readonly svgPipeline?: MermanSvgPipeline;
  readonly textMeasurementMode?: MermanTextMeasurementMode;
}

export function isMermanSvgPipeline(
  value: unknown,
): value is MermanSvgPipeline {
  return (
    typeof value === "string" &&
    MERMAN_SVG_PIPELINES.some((pipeline) => pipeline === value)
  );
}

export interface ConfiguredMermanOperationInput {
  readonly bindingOptions: Readonly<HostTextMeasurerSvgBindingOptions>;
  readonly bindingOptionsJson: string | null;
  readonly configurationError: ErrorProjection | null;
  readonly configuredSource: string;
  readonly injectedConfigSpan: Readonly<InjectedMermaidConfigSpan> | null;
  readonly source: string;
  readonly textMeasurementMode: MermanTextMeasurementMode;
}

export interface RenderOperationVersions {
  readonly merman: string;
  readonly mermaid: string;
}

export interface FrozenRenderOperation
  extends ConfiguredMermanOperationInput {
  readonly asciiEnabled: boolean;
  readonly compareEnabled: boolean;
  readonly configJson: string;
  readonly diagnosticsEnabled: boolean;
  readonly diagramFont: DiagramFont;
  readonly layoutEnvironment: Readonly<MermanLayoutEnvironment>;
  readonly themePresetId: string | null;
  readonly themeRecipeJson: string | null;
  readonly unavailableThemePresetId: string | null;
  readonly svgPipeline: MermanSvgPipeline;
  readonly theme: WorkspaceSnapshot["diagramTheme"];
  readonly mermaidTheme: WorkspaceSnapshot["diagramTheme"];
  readonly themeAppearancePreference: ThemeAppearancePreference;
  readonly resolvedThemeAppearance: ThemeAppearance | null;
  readonly supportedThemeAppearances: readonly ThemeAppearance[];
  readonly defaultThemeAppearance: ThemeAppearance | null;
  readonly versions: Readonly<RenderOperationVersions>;
  readonly viewport: Readonly<RealmViewport> | null;
}

export interface FreezeRenderOperationInput {
  readonly asciiEnabled: boolean;
  readonly compareEnabled: boolean;
  readonly diagnosticsEnabled: boolean;
  readonly layoutEnvironment: MermanLayoutEnvironment;
  readonly versions: RenderOperationVersions;
  readonly viewport: RealmViewport | null;
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly themePresets?: readonly Pick<ThemePresetCatalogEntry, "id" | "available">[];
  readonly themeCatalog?: ThemeCatalog | null;
  readonly themeRecipeInspection?: ThemeAppearanceSupport | null;
  readonly themeInspectionError?: ErrorProjection | null;
  readonly hostAppearance?: ThemeAppearance;
}

export function configuredMermanOperationInput(
  source: string,
  theme: string,
  configJson: string,
  options: MermanOperationOptions | undefined,
): ConfiguredMermanOperationInput {
  return freezeConfiguredInput(source, theme, configJson, {
    diagramFont: options?.diagramFont ?? DEFAULT_WORKSPACE_SNAPSHOT.diagramFont,
    layoutEnvironment: options?.layoutEnvironment,
    themePresetId:
      options?.themePresetId ?? DEFAULT_WORKSPACE_SNAPSHOT.themePresetId,
    themeRecipeJson: options?.themeRecipeJson ?? null,
    themeAppearance: options?.themeAppearance ?? null,
    svgPipeline: options?.svgPipeline ?? DEFAULT_WORKSPACE_SNAPSHOT.svgPipeline,
    textMeasurementMode:
      options?.textMeasurementMode ??
      DEFAULT_WORKSPACE_SNAPSHOT.textMeasurementMode,
  });
}

export function freezeRenderOperation({
  asciiEnabled,
  compareEnabled,
  diagnosticsEnabled,
  layoutEnvironment,
  versions,
  viewport,
  workspace,
  themePresets,
  themeCatalog,
  themeRecipeInspection,
  themeInspectionError,
  hostAppearance = "light",
}: FreezeRenderOperationInput): FrozenRenderOperation {
  const frozenLayout = freezeLayoutEnvironment(layoutEnvironment);
  const selectedPreset = workspace.themePresetId;
  const availablePresets = themeCatalog ? themeCatalog.presets.flatMap((preset) => [
    { id: preset.id, available: preset.available },
    ...preset.appearances.map((entry) => ({ id: entry.concrete_preset_id, available: preset.available })),
  ]) : themePresets;
  const unavailableThemePresetId = selectedPreset && workspace.themeRecipeJson === null &&
    availablePresets && !availablePresets.find((preset) => preset.id === selectedPreset.trim())?.available
    ? selectedPreset : null;
  const selection = resolveThemeSelection(workspace, hostAppearance, themeCatalog, themeRecipeInspection);
  const configured = freezeConfiguredInput(
    workspace.code,
    selection.mermaidTheme,
    workspace.mermaidConfig,
    {
      diagramFont: workspace.diagramFont,
      layoutEnvironment: frozenLayout,
      themePresetId: unavailableThemePresetId === null ? workspace.themePresetId : null,
      themeRecipeJson: workspace.themeRecipeJson,
      themeAppearance: unavailableThemePresetId === null ? selection.appearance : null,
      svgPipeline: workspace.svgPipeline,
      textMeasurementMode: workspace.textMeasurementMode,
    },
  );
  return Object.freeze({
    ...configured,
    configurationError: themeInspectionError ?? configured.configurationError ??
      (selection.error ? projectError(selection.error) : null),
    asciiEnabled,
    compareEnabled,
    configJson: workspace.mermaidConfig,
    diagnosticsEnabled,
    diagramFont: workspace.diagramFont,
    layoutEnvironment: frozenLayout,
    themePresetId: workspace.themePresetId,
    themeRecipeJson: workspace.themeRecipeJson,
    unavailableThemePresetId,
    svgPipeline: workspace.svgPipeline,
    theme: workspace.diagramTheme,
    mermaidTheme: selection.mermaidTheme,
    themeAppearancePreference: selection.preference,
    resolvedThemeAppearance: selection.appearance,
    supportedThemeAppearances: Object.freeze([...selection.supported_appearances]),
    defaultThemeAppearance: selection.default_appearance,
    versions: Object.freeze({ ...versions }),
    viewport: viewport ? Object.freeze({ ...viewport }) : null,
  });
}

export function renderOperationWithSvgPipeline(
  operation: FrozenRenderOperation,
  svgPipeline: MermanSvgPipeline,
): FrozenRenderOperation {
  if (operation.svgPipeline === svgPipeline) return operation;
  const { svg: _svg, ...bindingOptions } = operation.bindingOptions;
  const nextOptions = Object.freeze({
    ...bindingOptions,
    ...(svgPipeline === "parity"
      ? {}
      : { svg: Object.freeze({ pipeline: svgPipeline }) }),
  });
  return Object.freeze({
    ...operation,
    bindingOptions: nextOptions,
    bindingOptionsJson: operation.themeRecipeJson
      ? themeRecipeOptionsJson(nextOptions, operation.themeRecipeJson, operation.resolvedThemeAppearance)
      : null,
    svgPipeline,
  });
}

export function sameRenderOperation(
  left: FrozenRenderOperation,
  right: FrozenRenderOperation,
): boolean {
  return (
    left.source === right.source &&
    left.theme === right.theme &&
    left.mermaidTheme === right.mermaidTheme &&
    left.themeAppearancePreference === right.themeAppearancePreference &&
    left.resolvedThemeAppearance === right.resolvedThemeAppearance &&
    left.configJson === right.configJson &&
    left.themePresetId === right.themePresetId &&
    left.themeRecipeJson === right.themeRecipeJson &&
    left.unavailableThemePresetId === right.unavailableThemePresetId &&
    left.textMeasurementMode === right.textMeasurementMode &&
    left.diagramFont === right.diagramFont &&
    left.layoutEnvironment.containerWidth ===
      right.layoutEnvironment.containerWidth &&
    left.layoutEnvironment.containerHeight ===
      right.layoutEnvironment.containerHeight &&
    (left.layoutEnvironment.screenAvailableWidth ?? null) ===
      (right.layoutEnvironment.screenAvailableWidth ?? null) &&
    left.svgPipeline === right.svgPipeline &&
    left.asciiEnabled === right.asciiEnabled &&
    left.compareEnabled === right.compareEnabled &&
    left.diagnosticsEnabled === right.diagnosticsEnabled &&
    (left.viewport?.width ?? null) === (right.viewport?.width ?? null) &&
    (left.viewport?.height ?? null) === (right.viewport?.height ?? null) &&
    left.versions.merman === right.versions.merman &&
    left.versions.mermaid === right.versions.mermaid
  );
}

interface NormalizedMermanOptions {
  readonly diagramFont: DiagramFont;
  readonly layoutEnvironment?: Readonly<MermanLayoutEnvironment>;
  readonly themePresetId: string | null;
  readonly themeRecipeJson: string | null;
  readonly themeAppearance: ThemeAppearance | null;
  readonly svgPipeline: MermanSvgPipeline;
  readonly textMeasurementMode: MermanTextMeasurementMode;
}

function freezeConfiguredInput(
  source: string,
  theme: string,
  configJson: string,
  options: NormalizedMermanOptions,
): ConfiguredMermanOperationInput {
  let configuredSource = source;
  let configurationError: ErrorProjection | null = null;
  let bindingOptionsJson: string | null = null;
  let initializationConfig: Readonly<MermaidConfigObject> | undefined;
  let injectedConfigSpan: Readonly<InjectedMermaidConfigSpan> | null = null;
  try {
    ({ configuredSource, initializationConfig, injectedConfigSpan } = buildMermaidOperationInput(
      source, theme, configJson, { diagramFont: options.diagramFont },
    ));
  } catch (error) {
    configurationError = projectError(error);
  }
  const bindingOptions = bindingOptionsForRender(options, initializationConfig);
  try {
    if (options.themeRecipeJson !== null) {
      if (options.themePresetId !== null) {
        throw new Error("Select a Merman preset or a custom theme, not both.");
      }
      bindingOptionsJson = themeRecipeOptionsJson(bindingOptions, options.themeRecipeJson, options.themeAppearance);
    }
  } catch (error) {
    configurationError = projectError(error);
  }
  return Object.freeze({
    bindingOptions,
    bindingOptionsJson,
    configurationError,
    configuredSource,
    injectedConfigSpan,
    source,
    textMeasurementMode: options.textMeasurementMode,
  });
}

function bindingOptionsForRender(
  options: NormalizedMermanOptions,
  initializationConfig: Readonly<MermaidConfigObject> | undefined,
): Readonly<HostTextMeasurerSvgBindingOptions> {
  const theme = options.themePresetId
    ? Object.freeze({ preset: options.themePresetId, ...(options.themeAppearance ? { appearance: options.themeAppearance } : {}) })
    : undefined;
  const svg =
    options.svgPipeline === "parity"
      ? undefined
      : Object.freeze({ pipeline: options.svgPipeline });
  const layout = options.layoutEnvironment
    ? Object.freeze({
        container_width: options.layoutEnvironment.containerWidth,
        container_height: options.layoutEnvironment.containerHeight,
        ...(options.layoutEnvironment.screenAvailableWidth === undefined
          ? {}
          : {
              screen_available_width:
                options.layoutEnvironment.screenAvailableWidth,
            }),
      })
    : undefined;
  return Object.freeze({
    version: BINDING_OPTIONS_SCHEMA_VERSION,
    ...(theme ? { theme } : {}),
    ...(initializationConfig ? { site_config: initializationConfig } : {}),
    ...(svg ? { svg } : {}),
    ...(layout ? { layout } : {}),
  });
}

function freezeLayoutEnvironment(
  value: MermanLayoutEnvironment,
): Readonly<MermanLayoutEnvironment> {
  return Object.freeze({
    containerHeight: value.containerHeight,
    containerWidth: value.containerWidth,
    ...(value.screenAvailableWidth === undefined
      ? {}
      : { screenAvailableWidth: value.screenAvailableWidth }),
  });
}

/** Keep authored JSON intact so Rust can reject duplicate fields and unknown schema members. */
export function themeRecipeOptionsJson(
  options: Readonly<HostTextMeasurerSvgBindingOptions>,
  recipeJson: string,
  appearance: ThemeAppearance | null = null,
): string {
  const recipe: unknown = JSON.parse(recipeJson);
  if (
    !recipe || typeof recipe !== "object" || Array.isArray(recipe) ||
    !("schema_version" in recipe) || recipe.schema_version !== 1 ||
    !("kind" in recipe) || (recipe.kind !== "design" && recipe.kind !== "complete_spec")
  ) {
    throw new Error("Expected a version 1 theme recipe (design or complete_spec).");
  }
  // JSON.parse above verifies this is one complete value before inserting it into the envelope.
  const { theme: _theme, ...base } = options;
  const encodedBase = JSON.stringify(base);
  const prefix = encodedBase === "{}" ? "{" : `${encodedBase.slice(0, -1)},`;
  return `${prefix}"theme":${appearance ? `{"recipe":${recipeJson},"appearance":${JSON.stringify(appearance)}}` : recipeJson}}`;
}

export function matchesWorkspaceAppearance(
  operation: FrozenRenderOperation,
  workspace: Readonly<WorkspaceSnapshot>,
  hostAppearance?: ThemeAppearance,
): boolean {
  if (operation.themeAppearancePreference !== (workspace.themeAppearance ?? "default")) return false;
  if (operation.themeAppearancePreference !== "follow" || !hostAppearance) return true;
  const expected = operation.supportedThemeAppearances.includes(hostAppearance)
    ? hostAppearance : operation.defaultThemeAppearance;
  return operation.resolvedThemeAppearance === expected;
}
