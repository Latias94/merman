import {
  BINDING_OPTIONS_SCHEMA_VERSION,
  type HostTextMeasurerSvgBindingOptions,
  type ShoalLayoutFrame,
  type ThemeAppearance,
  type ThemeCatalog,
} from "@mermanjs/web";

import type { DiagramFont } from "../lib/diagram-font.ts";
import { buildMermaidOperationInput, type InjectedMermaidConfigSpan, type MermaidConfigObject } from "../lib/mermaid-config.ts";
import { exceedsUtf8ByteBudget } from "../lib/utf8.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "../lib/workspace-snapshot.ts";
import type { RealmViewport } from "./realm/channel-protocol.ts";
import { projectError, type ErrorProjection } from "./error-projection.ts";
import { resolveThemeSelection, type ThemeAppearancePreference } from "../lib/theme-appearance.ts";
import { parseThemeDocument } from "../themes/v2.ts";

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

export interface MermanRuntimeSnapshot {
  readonly unixMillis: number;
  readonly localOffsetMinutes: number;
}

export interface MermanOperationOptions {
  readonly diagramFont?: DiagramFont;
  readonly layoutEnvironment?: MermanLayoutEnvironment;
  readonly runtimeSnapshot?: MermanRuntimeSnapshot;
  readonly themePresetId?: string | null;
  readonly themeDocumentJson?: string | null;
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
  readonly bindingOptionsJson: string;
  readonly configurationError: ErrorProjection | null;
  readonly configuredSource: string;
  readonly injectedConfigSpan: Readonly<InjectedMermaidConfigSpan> | null;
  readonly source: string;
  readonly textMeasurementMode: MermanTextMeasurementMode;
  readonly themeDocumentJson: string | null;
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
  readonly runtimeSnapshot?: MermanRuntimeSnapshot;
  readonly versions: RenderOperationVersions;
  readonly viewport: RealmViewport | null;
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly themeCatalog?: ThemeCatalog | null;
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
    runtimeSnapshot: options?.runtimeSnapshot,
    themePresetId:
      options?.themePresetId ?? DEFAULT_WORKSPACE_SNAPSHOT.themePresetId,
    themeDocumentJson: options?.themeDocumentJson ?? null,
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
  runtimeSnapshot = captureMermanRuntimeSnapshot(),
  versions,
  viewport,
  workspace,
  themeCatalog,
  hostAppearance = "light",
}: FreezeRenderOperationInput): FrozenRenderOperation {
  const frozenLayout = freezeLayoutEnvironment(layoutEnvironment);
  const selectedPreset = workspace.themePresetId?.trim() ?? null;
  const unavailableThemePresetId = selectedPreset && workspace.themeDocumentJson === null &&
    themeCatalog && !themeCatalog.presets.some(({ id }) => id === selectedPreset) ? selectedPreset : null;
  const selection = resolveThemeSelection(workspace, hostAppearance, themeCatalog);
  const configured = freezeConfiguredInput(
    workspace.code,
    selection.mermaidTheme,
    workspace.mermaidConfig,
    {
      diagramFont: workspace.diagramFont,
      layoutEnvironment: frozenLayout,
      runtimeSnapshot,
      themePresetId: unavailableThemePresetId === null ? selectedPreset : null,
      themeDocumentJson: workspace.themeDocumentJson,
      themeAppearance: selection.preference !== "default" ? selection.appearance : null,
      svgPipeline: workspace.svgPipeline,
      textMeasurementMode: workspace.textMeasurementMode,
    },
  );
  return Object.freeze({
    ...configured,
    configurationError: configured.configurationError ?? (selection.error ? projectError(selection.error) : null),
    asciiEnabled,
    compareEnabled,
    configJson: workspace.mermaidConfig,
    diagnosticsEnabled,
    diagramFont: workspace.diagramFont,
    layoutEnvironment: frozenLayout,
    themePresetId: workspace.themePresetId,
    themeDocumentJson: workspace.themeDocumentJson,
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
    bindingOptionsJson: encodeMermanOptions(nextOptions, operation.themeDocumentJson),
    svgPipeline,
  });
}

export function previewRenderWithShoalFrame(
  operation: FrozenRenderOperation,
  previous: ShoalLayoutFrame | undefined,
  optionsByteLimit: number,
): ConfiguredMermanOperationInput {
  for (const frame of previous === undefined ? [undefined] : [previous, undefined]) {
    const bindingOptions = Object.freeze({
      ...operation.bindingOptions,
      layout: Object.freeze({
        ...operation.bindingOptions.layout,
        capture_shoal_frame: true,
        ...(frame === undefined ? {} : { previous_shoal_frame: frame }),
      }),
    });
    const bindingOptionsJson = encodeMermanOptions(bindingOptions, operation.themeDocumentJson);
    if (!exceedsUtf8ByteBudget(bindingOptionsJson, optionsByteLimit)) {
      return Object.freeze({ ...operation, bindingOptions, bindingOptionsJson });
    }
  }
  return operation;
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
    left.themeDocumentJson === right.themeDocumentJson &&
    left.unavailableThemePresetId === right.unavailableThemePresetId &&
    left.textMeasurementMode === right.textMeasurementMode &&
    left.bindingOptions.fixed_unix_millis === right.bindingOptions.fixed_unix_millis &&
    left.bindingOptions.fixed_local_offset_minutes === right.bindingOptions.fixed_local_offset_minutes &&
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
  readonly runtimeSnapshot?: MermanRuntimeSnapshot;
  readonly themePresetId: string | null;
  readonly themeDocumentJson: string | null;
  readonly themeAppearance: ThemeAppearance | null;
  readonly svgPipeline: MermanSvgPipeline;
  readonly textMeasurementMode: MermanTextMeasurementMode;
}

function freezeConfiguredInput(
  source: string,
  mermaidTheme: string,
  configJson: string,
  options: NormalizedMermanOptions,
): ConfiguredMermanOperationInput {
  let configuredSource = source;
  let configurationError: ErrorProjection | null = null;
  let initializationConfig: Readonly<MermaidConfigObject> | undefined;
  let injectedConfigSpan: Readonly<InjectedMermaidConfigSpan> | null = null;
  let theme: HostTextMeasurerSvgBindingOptions["theme"];
  try {
    ({ configuredSource, initializationConfig, injectedConfigSpan } = buildMermaidOperationInput(
      source, mermaidTheme, configJson, { diagramFont: options.diagramFont },
    ));
  } catch (error) {
    configurationError = projectError(error);
  }
  try {
    const { themeAppearance: appearance } = options;
    if (options.themeDocumentJson !== null) {
      if (options.themePresetId !== null) {
        throw new Error("Select a Merman preset or a custom theme, not both.");
      }
      const document = parseThemeDocument(options.themeDocumentJson);
      theme = appearance ? Object.freeze({ document, appearance }) : document;
    } else if (options.themePresetId) {
      theme = appearance ? Object.freeze({ preset: options.themePresetId, appearance }) : options.themePresetId;
    }
  } catch (error) {
    configurationError = projectError(error);
  }
  const bindingOptions = bindingOptionsForRender(options, initializationConfig, theme);
  return Object.freeze({
    bindingOptions,
    bindingOptionsJson: encodeMermanOptions(bindingOptions, options.themeDocumentJson),
    configurationError,
    configuredSource,
    injectedConfigSpan,
    source,
    textMeasurementMode: options.textMeasurementMode,
    themeDocumentJson: options.themeDocumentJson,
  });
}

function bindingOptionsForRender(
  options: NormalizedMermanOptions,
  initializationConfig: Readonly<MermaidConfigObject> | undefined,
  theme: HostTextMeasurerSvgBindingOptions["theme"],
): Readonly<HostTextMeasurerSvgBindingOptions> {
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
    ...(options.runtimeSnapshot ? {
      fixed_unix_millis: options.runtimeSnapshot.unixMillis,
      fixed_local_offset_minutes: options.runtimeSnapshot.localOffsetMinutes,
    } : {}),
    ...(theme ? { theme } : {}),
    ...(initializationConfig ? { site_config: initializationConfig } : {}),
    ...(svg ? { svg } : {}),
    ...(layout ? { layout } : {}),
  });
}

export function captureMermanRuntimeSnapshot(): MermanRuntimeSnapshot {
  const instant = new Date();
  return {
    unixMillis: instant.getTime(),
    localOffsetMinutes: -instant.getTimezoneOffset(),
  };
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
export function encodeMermanOptions(
  options: Readonly<HostTextMeasurerSvgBindingOptions>,
  documentJson: string | null,
): string {
  const { theme, ...base } = options;
  if (documentJson === null || typeof theme !== "object" || !theme) return JSON.stringify(options);
  const value = theme.schema_version !== 2 && theme.document
    ? `{"document":${documentJson}${theme.appearance ? `,"appearance":${JSON.stringify(theme.appearance)}` : ""}${
      theme.precedence ? `,"precedence":${JSON.stringify(theme.precedence)}` : ""}}`
    : documentJson;
  return `${JSON.stringify(base).slice(0, -1)},"theme":${value}}`;
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

export function matchesWorkspaceRender(
  operation: FrozenRenderOperation,
  workspace: Readonly<WorkspaceSnapshot>,
  hostAppearance?: ThemeAppearance,
): boolean {
  return operation.source === workspace.code && operation.configJson === workspace.mermaidConfig &&
    operation.theme === workspace.diagramTheme && operation.diagramFont === workspace.diagramFont &&
    operation.themePresetId === workspace.themePresetId && operation.themeDocumentJson === workspace.themeDocumentJson &&
    operation.textMeasurementMode === workspace.textMeasurementMode && operation.svgPipeline === workspace.svgPipeline &&
    matchesWorkspaceAppearance(operation, workspace, hostAppearance);
}
