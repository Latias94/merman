import type {
  DiagramDetectionFacts,
  PresentationAspectCatalogEntry,
  PresentationCatalog,
  SvgPlanResult,
} from "@mermanjs/web";

export type PresentationProfileStatusKind =
  | "fully-available"
  | "missing-but-inapplicable"
  | "inactive-by-explicit-renderer"
  | "blocked-for-current-operation"
  | "unknown-future-id";

export interface PresentationProfileStatus {
  readonly kind: PresentationProfileStatusKind;
  readonly missingCapabilityIds: readonly string[];
}

export interface PresentationProfileStatusInput {
  readonly catalog: PresentationCatalog;
  readonly detection: DiagramDetectionFacts;
  readonly plan: SvgPlanResult | null;
  readonly selectedProfileId: string | null;
}

export function presentationProfileStatus({
  catalog,
  detection,
  plan,
  selectedProfileId,
}: PresentationProfileStatusInput): PresentationProfileStatus | null {
  if (!selectedProfileId) return null;

  const profile = catalog.profiles.find(
    (candidate) => candidate.id === selectedProfileId,
  );
  if (!profile) {
    return status("unknown-future-id", []);
  }

  const unavailableAspects = profile.aspects.filter(
    (aspect) => !aspect.available || aspect.missing_capability_ids.length > 0,
  );
  if (profile.fully_available || unavailableAspects.length === 0) {
    return status(
      "fully-available",
      profile.missing_capability_ids,
    );
  }

  const currentPlan =
    plan?.presentation_profile_id === selectedProfileId ? plan : null;
  const plannedAspects = new Map(
    currentPlan?.presentation_aspects.map((aspect) => [aspect.id, aspect]),
  );
  if (
    unavailableAspects.some(
      (aspect) => plannedAspects.get(aspect.id)?.state === "blocked",
    )
  ) {
    return status(
      "blocked-for-current-operation",
      currentPlan?.missing_capability_ids ?? profile.missing_capability_ids,
    );
  }

  const applicableAspects = unavailableAspects.filter((aspect) =>
    aspectAppliesToDetection(aspect, detection),
  );
  if (applicableAspects.length === 0) {
    return status(
      "missing-but-inapplicable",
      profile.missing_capability_ids,
    );
  }

  if (
    currentPlan &&
    applicableAspects.every(
      (aspect) => plannedAspects.get(aspect.id)?.state === "inactive",
    )
  ) {
    return status(
      "inactive-by-explicit-renderer",
      profile.missing_capability_ids,
    );
  }

  return status(
    "blocked-for-current-operation",
    currentPlan?.missing_capability_ids ?? profile.missing_capability_ids,
  );
}

function aspectAppliesToDetection(
  aspect: PresentationAspectCatalogEntry,
  detection: DiagramDetectionFacts,
): boolean {
  if (aspect.applicability.kind === "all-diagrams") return true;
  if (aspect.applicability.kind !== "family") return true;
  if (detection.status !== "available") return true;
  return aspect.applicability.family_id === detection.diagramType;
}

function status(
  kind: PresentationProfileStatusKind,
  missingCapabilityIds: readonly string[],
): PresentationProfileStatus {
  return {
    kind,
    missingCapabilityIds: [...missingCapabilityIds],
  };
}
