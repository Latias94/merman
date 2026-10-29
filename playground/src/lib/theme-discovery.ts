import type {
  DiagramDetectionFacts,
  DiagramFamilyCapability,
  SvgRenderMetadata,
  ThemeExecutionEvidenceV1,
  ThemePresetCatalogEntry,
  ThemePresetFamilyDesign,
} from "@mermanjs/web";
import type { CompletedRenderBatch, RenderCoordinatorState } from "../runtime/render-coordinator.ts";
import type { FrozenRenderOperation } from "../runtime/merman-operation-input.ts";
import type { WorkspaceSnapshot } from "./workspace-snapshot.ts";
import type { ThemeEditSession } from "./theme-edit-session.ts";

export type ThemeFamilyTreatment = "dedicated" | "base_only" | "unreviewed";

export function themeFamilyTreatment(
  preset: Pick<ThemePresetCatalogEntry, "family_designs">,
  familyId: string | null,
): ThemeFamilyTreatment {
  const treatment = preset.family_designs.find((design) => design.family_id === familyId)?.treatment;
  return treatment === "dedicated" || treatment === "base_only" ? treatment : "unreviewed";
}

export function reviewedThemeFamilies(
  preset: Pick<ThemePresetCatalogEntry, "family_designs">,
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

/** Diagram identity stays usable across theme drafts when its own detection inputs are unchanged. */
export function currentDiagramDetection(
  state: RenderCoordinatorState,
  workspace: Readonly<WorkspaceSnapshot>,
): DiagramDetectionFacts | null {
  const batch = state.status === "updating" ? state.previous
    : state.status === "success" || state.status === "partial" || state.status === "failed" ? state : null;
  return batch && matchesDiagramDetectionInputs(batch.snapshot.operation, workspace) ? batch.detection : null;
}

export type CurrentThemeRender =
  | { readonly status: "waiting"; readonly reason: "render" | "importing" | "validating" | "invalid"; readonly batch: null }
  | { readonly status: "failed" | "ready"; readonly batch: CompletedRenderBatch };

/** Check inputs as well as pending state: React can render before the coordinator bridge's effect. */
export function currentThemeRender(
  state: RenderCoordinatorState,
  workspace: Readonly<WorkspaceSnapshot>,
  session: ThemeEditSession | null,
): CurrentThemeRender {
  if (session?.importing) return { status: "waiting", reason: "importing", batch: null };
  if (session?.validation === "pending") return { status: "waiting", reason: "validating", batch: null };
  if (session?.validation === "invalid") return { status: "waiting", reason: "invalid", batch: null };
  if (state.status !== "success" && state.status !== "partial" && state.status !== "failed") {
    return { status: "waiting", reason: "render", batch: null };
  }
  if (!matchesThemeRenderInputs(state.snapshot.operation, workspace)) {
    return { status: "waiting", reason: "render", batch: null };
  }
  return { status: state.merman.status === "failure" ? "failed" : "ready", batch: state };
}

function matchesThemeRenderInputs(operation: FrozenRenderOperation, workspace: Readonly<WorkspaceSnapshot>): boolean {
  return matchesDiagramDetectionInputs(operation, workspace) && operation.themePresetId === workspace.themePresetId &&
    operation.themeRecipeJson === workspace.themeRecipeJson && operation.svgPipeline === workspace.svgPipeline &&
    operation.textMeasurementMode === workspace.textMeasurementMode;
}

function matchesDiagramDetectionInputs(operation: FrozenRenderOperation, workspace: Readonly<WorkspaceSnapshot>): boolean {
  return operation.source === workspace.code && operation.configJson === workspace.mermaidConfig &&
    operation.theme === workspace.diagramTheme && operation.diagramFont === workspace.diagramFont;
}

export function readThemeExecutionEvidence(metadata: SvgRenderMetadata | null): ThemeExecutionEvidenceV1 | null {
  const evidence = metadata?.theme_execution_evidence;
  if (evidence?.version !== 1 || typeof evidence.family_id !== "string" ||
    typeof evidence.theme_status !== "string" || typeof evidence.target_status !== "string" ||
    typeof evidence.target_kind !== "string" || typeof evidence.font_source !== "string" ||
    typeof evidence.output_mutated !== "boolean" || !Array.isArray(evidence.target_reason_ids)) return null;
  return evidence as ThemeExecutionEvidenceV1;
}

export function themeEvidenceOutcome(evidence: ThemeExecutionEvidenceV1 | null) {
  switch (evidence?.theme_status) {
    case "verified": return "applied";
    case "residual": return "residual";
    case "incomplete": return "incomplete";
    case "not_applicable": return "not_present";
    default: return "unavailable";
  }
}

export function themeTargetOutcome(evidence: ThemeExecutionEvidenceV1 | null) {
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
