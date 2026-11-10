import { UNAVAILABLE_DIAGRAM_DETECTION } from "@mermanjs/web";
import type {
  DiagramDetectionFacts,
  DiagramFamilyCapability,
  SvgRenderMetadata,
  ThemeExecutionEvidenceV2,
  ThemePresetCatalogEntry,
  ThemePresetFamilyDesign,
  ThemeAppearance,
} from "@mermanjs/web";
import type { CompletedRenderBatch, RenderCoordinatorState } from "../runtime/render-coordinator.ts";
import type { FrozenRenderOperation } from "../runtime/merman-operation-input.ts";
import type { MermanDomainFacade } from "../runtime/merman-core.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";
import { readMermaidAppearanceOverrides } from "./mermaid-appearance.ts";
import { configuredMermanOperationInput, matchesWorkspaceAppearance } from "../runtime/merman-operation-input.ts";
import { inspectThemeRecipe, resolveThemeSelection } from "./theme-appearance.ts";

type ThemeFamilyDesignOwner = Pick<ThemePresetCatalogEntry["appearances"][number], "family_designs">;

export type ThemeFamilyTreatment = "dedicated" | "base_only" | "unreviewed";

export function themeFamilyTreatment(
  preset: ThemeFamilyDesignOwner,
  familyId: string | null,
): ThemeFamilyTreatment {
  const treatment = preset.family_designs.find((design) => design.family_id === familyId)?.treatment;
  return treatment === "dedicated" || treatment === "base_only" ? treatment : "unreviewed";
}

export function reviewedThemeFamilies(
  preset: ThemeFamilyDesignOwner,
): ThemePresetFamilyDesign[] {
  return preset.family_designs.filter((design) => design.treatment === "dedicated" || design.treatment === "base_only");
}

export function detectedThemeFamily(
  detection: DiagramDetectionFacts | null,
  capabilities: readonly Pick<DiagramFamilyCapability, "diagram_type" | "family_id">[],
): string | null {
  if (detection?.status !== "available") return null;
  return capabilities.find((entry) => entry.diagram_type === detection.syntaxId)?.family_id ?? null;
}

export function themeFamilyMetadataId(
  familyId: string,
  capabilities: readonly Pick<DiagramFamilyCapability, "family_id" | "metadata_id">[],
): string | null {
  return capabilities.find((entry) => entry.family_id === familyId && entry.metadata_id !== null)?.metadata_id ?? null;
}

type DiagramDetectionFacade = Pick<MermanDomainFacade, "detectDiagram" | "themeCatalog" | "materializeTheme">;

export interface WorkspaceDiagramDetection {
  readonly facade: DiagramDetectionFacade;
  readonly workspace: Readonly<WorkspaceSnapshot>;
  readonly hostAppearance: ThemeAppearance;
  readonly detection: DiagramDetectionFacts;
}

/** Detect independently of preview visibility without publishing a render or changing its state. */
export function detectWorkspaceDiagram(
  facade: DiagramDetectionFacade,
  workspace: Readonly<WorkspaceSnapshot>,
  hostAppearance: ThemeAppearance,
): WorkspaceDiagramDetection {
  let detection: DiagramDetectionFacts = UNAVAILABLE_DIAGRAM_DETECTION;
  try {
    let catalog: ReturnType<DiagramDetectionFacade["themeCatalog"]> | null = null;
    try { catalog = facade.themeCatalog(); } catch { /* Detection also works without a theme catalog. */ }
    const inspection = workspace.themeRecipeJson === null ? null : inspectThemeRecipe(facade, workspace.themeRecipeJson);
    const selection = resolveThemeSelection(workspace, hostAppearance, catalog, inspection);
    if (!selection.error) {
      const input = configuredMermanOperationInput(workspace.code, selection.mermaidTheme, workspace.mermaidConfig, {
        diagramFont: workspace.diagramFont,
        themePresetId: workspace.themePresetId,
        themeRecipeJson: workspace.themeRecipeJson,
        themeAppearance: selection.appearance,
        svgPipeline: workspace.svgPipeline,
        textMeasurementMode: workspace.textMeasurementMode,
      });
      if (!input.configurationError) detection = facade.detectDiagram(input);
    }
  } catch { /* Invalid or unavailable inputs must not reuse an earlier diagram identity. */ }
  return { facade, workspace, hostAppearance, detection };
}

/** A local detection belongs only to the exact inputs and runtime that produced it. */
export function currentWorkspaceDiagramDetection(
  result: WorkspaceDiagramDetection | null,
  facade: DiagramDetectionFacade | null,
  workspace: Readonly<WorkspaceSnapshot>,
  hostAppearance: ThemeAppearance,
): DiagramDetectionFacts | null {
  return result?.facade === facade && result.workspace === workspace && result.hostAppearance === hostAppearance
    ? result.detection : null;
}

/** Diagram identity stays usable when its own detection inputs are unchanged. */
export function currentDiagramDetection(
  state: RenderCoordinatorState,
  workspace: Readonly<WorkspaceSnapshot>,
  hostAppearance?: ThemeAppearance,
): DiagramDetectionFacts | null {
  const batch = state.status === "updating" ? state.previous
    : state.status === "success" || state.status === "partial" || state.status === "failed" ? state : null;
  return batch && matchesDiagramDetectionInputs(batch.snapshot.operation, workspace, hostAppearance) ? batch.detection : null;
}

export function currentAppearanceOverrides(
  state: RenderCoordinatorState,
  workspace: Readonly<WorkspaceSnapshot>,
  hostAppearance?: ThemeAppearance,
) {
  const batch = state.status === "updating" ? state.previous
    : state.status === "success" || state.status === "partial" || state.status === "failed" ? state : null;
  return batch && matchesDiagramDetectionInputs(batch.snapshot.operation, workspace, hostAppearance)
    ? readMermaidAppearanceOverrides(batch.detection.appearance, batch.snapshot.operation.injectedConfigSpan)
    : null;
}

export type CurrentThemeRender =
  | { readonly status: "waiting"; readonly reason: "render"; readonly batch: null }
  | { readonly status: "failed" | "ready"; readonly batch: CompletedRenderBatch };

/** Check inputs as well as pending state: React can render before the coordinator bridge's effect. */
export function currentThemeRender(
  state: RenderCoordinatorState,
  workspace: Readonly<WorkspaceSnapshot>,
  hostAppearance?: ThemeAppearance,
): CurrentThemeRender {
  if (state.status !== "success" && state.status !== "partial" && state.status !== "failed") {
    return { status: "waiting", reason: "render", batch: null };
  }
  if (!matchesThemeRenderInputs(state.snapshot.operation, workspace, hostAppearance)) {
    return { status: "waiting", reason: "render", batch: null };
  }
  return { status: state.merman.status === "failure" ? "failed" : "ready", batch: state };
}

function matchesThemeRenderInputs(operation: FrozenRenderOperation, workspace: Readonly<WorkspaceSnapshot>, hostAppearance?: ThemeAppearance): boolean {
  return matchesDiagramDetectionInputs(operation, workspace, hostAppearance) && operation.themePresetId === workspace.themePresetId &&
    operation.themeRecipeJson === workspace.themeRecipeJson && operation.svgPipeline === workspace.svgPipeline &&
    operation.textMeasurementMode === workspace.textMeasurementMode;
}

function matchesDiagramDetectionInputs(operation: FrozenRenderOperation, workspace: Readonly<WorkspaceSnapshot>, hostAppearance?: ThemeAppearance): boolean {
  return operation.source === workspace.code && operation.configJson === workspace.mermaidConfig &&
    operation.theme === workspace.diagramTheme && operation.diagramFont === workspace.diagramFont &&
    matchesWorkspaceAppearance(operation, workspace, hostAppearance);
}

export function readThemeExecutionEvidence(metadata: SvgRenderMetadata | null): ThemeExecutionEvidenceV2 | null {
  const evidence = metadata?.theme_execution_evidence;
  if (evidence?.version !== 2 || typeof evidence.family_id !== "string" ||
    typeof evidence.theme_status !== "string" || typeof evidence.target_status !== "string" ||
    typeof evidence.target_kind !== "string" || typeof evidence.font_source !== "string" ||
    typeof evidence.output_mutated !== "boolean" || !Array.isArray(evidence.target_reason_ids)) return null;
  return evidence as ThemeExecutionEvidenceV2;
}

export function themeEvidenceOutcome(evidence: ThemeExecutionEvidenceV2 | null) {
  switch (evidence?.theme_status) {
    case "verified": return "applied";
    case "residual": return "residual";
    case "incomplete": return "incomplete";
    case "not_applicable": return "not_present";
    default: return "unavailable";
  }
}

export function themeTargetOutcome(evidence: ThemeExecutionEvidenceV2 | null) {
  switch (evidence?.target_status) {
    case "portable": return "checked";
    case "host_dependent": return "host_dependent";
    case "rejected": return "rejected";
    case "unverified": return "unverified";
    default: return "unavailable";
  }
}

/** Summarize repeated codes without interpreting rule targets or inferring support. */
export function boundedThemeReasonCodes(codes: readonly string[]): { visible: string[]; remaining: number } {
  const unique = [...new Set(codes)];
  const visible = unique.slice(0, 4);
  return { visible, remaining: unique.length - visible.length };
}
