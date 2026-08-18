import type { SvgBindingOptions } from "@mermanjs/web";

import type { DiagramFont } from "../lib/diagram-font.ts";
import { sourceWithConfig } from "../lib/mermaid-config.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "../lib/workspace-snapshot.ts";
import type { RealmViewport } from "./realm/channel-protocol.ts";
import { projectError, type ErrorProjection } from "./error-projection.ts";

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
  readonly presentationProfileId?: string | null;
  readonly presentationThemePresetId?: string | null;
  readonly svgPipeline?: MermanSvgPipeline;
  readonly textMeasurementMode?: MermanTextMeasurementMode;
}

export function isMermanSvgPipeline(
  value: unknown
): value is MermanSvgPipeline {
  return (
    typeof value === "string" &&
    MERMAN_SVG_PIPELINES.some((pipeline) => pipeline === value)
  );
}

export interface ConfiguredMermanOperationInput {
  readonly bindingOptions: Readonly<SvgBindingOptions>;
  readonly configurationError: ErrorProjection | null;
  readonly configuredSource: string;
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
  readonly presentationProfileId: string | null;
  readonly presentationThemePresetId: string | null;
  readonly svgPipeline: MermanSvgPipeline;
  readonly theme: WorkspaceSnapshot["diagramTheme"];
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
}

export function configuredMermanOperationInput(
  source: string,
  theme: string,
  configJson: string,
  options: MermanOperationOptions | undefined
): ConfiguredMermanOperationInput {
  return freezeConfiguredInput(source, theme, configJson, {
    diagramFont: options?.diagramFont ?? DEFAULT_WORKSPACE_SNAPSHOT.diagramFont,
    layoutEnvironment: options?.layoutEnvironment,
    presentationProfileId:
      options?.presentationProfileId ??
      DEFAULT_WORKSPACE_SNAPSHOT.presentationProfileId,
    presentationThemePresetId:
      options?.presentationThemePresetId ??
      DEFAULT_WORKSPACE_SNAPSHOT.presentationThemePresetId,
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
}: FreezeRenderOperationInput): FrozenRenderOperation {
  const frozenLayout = freezeLayoutEnvironment(layoutEnvironment);
  const configured = freezeConfiguredInput(
    workspace.code,
    workspace.diagramTheme,
    workspace.mermaidConfig,
    {
      diagramFont: workspace.diagramFont,
      layoutEnvironment: frozenLayout,
      presentationProfileId: workspace.presentationProfileId,
      presentationThemePresetId: workspace.presentationThemePresetId,
      svgPipeline: workspace.svgPipeline,
      textMeasurementMode: workspace.textMeasurementMode,
    }
  );
  return Object.freeze({
    ...configured,
    asciiEnabled,
    compareEnabled,
    configJson: workspace.mermaidConfig,
    diagnosticsEnabled,
    diagramFont: workspace.diagramFont,
    layoutEnvironment: frozenLayout,
    presentationProfileId: workspace.presentationProfileId,
    presentationThemePresetId: workspace.presentationThemePresetId,
    svgPipeline: workspace.svgPipeline,
    theme: workspace.diagramTheme,
    versions: Object.freeze({ ...versions }),
    viewport: viewport ? Object.freeze({ ...viewport }) : null,
  });
}

export function renderOperationWithSvgPipeline(
  operation: FrozenRenderOperation,
  svgPipeline: MermanSvgPipeline
): FrozenRenderOperation {
  if (operation.svgPipeline === svgPipeline) return operation;
  const { svg: _svg, ...bindingOptions } = operation.bindingOptions;
  return Object.freeze({
    ...operation,
    bindingOptions: Object.freeze({
      ...bindingOptions,
      ...(svgPipeline === "parity"
        ? {}
        : { svg: Object.freeze({ pipeline: svgPipeline }) }),
    }),
    svgPipeline,
  });
}

export function sameRenderOperation(
  left: FrozenRenderOperation,
  right: FrozenRenderOperation
): boolean {
  return (
    left.source === right.source &&
    left.theme === right.theme &&
    left.configJson === right.configJson &&
    left.presentationProfileId === right.presentationProfileId &&
    left.presentationThemePresetId === right.presentationThemePresetId &&
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
  readonly presentationProfileId: string | null;
  readonly presentationThemePresetId: string | null;
  readonly svgPipeline: MermanSvgPipeline;
  readonly textMeasurementMode: MermanTextMeasurementMode;
}

function freezeConfiguredInput(
  source: string,
  theme: string,
  configJson: string,
  options: NormalizedMermanOptions
): ConfiguredMermanOperationInput {
  let configuredSource = source;
  let configurationError: ErrorProjection | null = null;
  try {
    configuredSource = sourceWithConfig(source, theme, configJson, {
      diagramFont: options.diagramFont,
    });
  } catch (error) {
    configurationError = projectError(error);
  }
  return Object.freeze({
    bindingOptions: bindingOptionsForRender(options),
    configurationError,
    configuredSource,
    source,
    textMeasurementMode: options.textMeasurementMode,
  });
}

function bindingOptionsForRender(
  options: NormalizedMermanOptions
): Readonly<SvgBindingOptions> {
  const presentation =
    options.presentationProfileId || options.presentationThemePresetId
      ? Object.freeze({
          ...(options.presentationProfileId
            ? { profile: options.presentationProfileId }
            : {}),
          ...(options.presentationThemePresetId
            ? {
                theme: Object.freeze({
                  preset: options.presentationThemePresetId,
                }),
              }
            : {}),
        })
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
    version: 2,
    ...(presentation ? { presentation } : {}),
    ...(svg ? { svg } : {}),
    ...(layout ? { layout } : {}),
  });
}

function freezeLayoutEnvironment(
  value: MermanLayoutEnvironment
): Readonly<MermanLayoutEnvironment> {
  return Object.freeze({
    containerHeight: value.containerHeight,
    containerWidth: value.containerWidth,
    ...(value.screenAvailableWidth === undefined
      ? {}
      : { screenAvailableWidth: value.screenAvailableWidth }),
  });
}
