import assert from "node:assert/strict";
import test from "node:test";

import type {
  DiagramDetectionFacts,
  PresentationCatalog,
  SvgPlanResult,
} from "@mermanjs/web";

import { presentationProfileStatus } from "./presentation-status.ts";

test("reports a fully available profile", () => {
  assert.equal(
    statusKind({ catalog: catalog({ fullyAvailable: true }) }),
    "fully-available",
  );
});

test("reports missing capabilities as inapplicable for another diagram family", () => {
  assert.equal(
    statusKind({
      detection: detection("sequence", "dagre"),
      plan: plan("inactive", false, "sequence"),
    }),
    "missing-but-inapplicable",
  );
});

test("reports an unavailable aspect disabled by an explicit renderer", () => {
  assert.equal(
    statusKind({
      detection: detection("flowchart", "dagre"),
      plan: plan("inactive", true),
    }),
    "inactive-by-explicit-renderer",
  );
});

test("reports an unavailable aspect blocking the current operation", () => {
  assert.equal(
    statusKind({ plan: plan("blocked", false) }),
    "blocked-for-current-operation",
  );
});

test("preserves an unknown future profile ID", () => {
  assert.equal(
    presentationProfileStatus({
      catalog: catalog(),
      detection: detection("flowchart", "elk"),
      plan: null,
      selectedProfileId: "future-profile",
    })?.kind,
    "unknown-future-id",
  );
});

function statusKind({
  catalog: value = catalog(),
  detection: facts = detection("flowchart", "elk"),
  plan: valuePlan = plan("active", true),
}: {
  catalog?: PresentationCatalog;
  detection?: DiagramDetectionFacts;
  plan?: SvgPlanResult;
} = {}) {
  const result = presentationProfileStatus({
    catalog: value,
    detection: facts,
    plan: valuePlan,
    selectedProfileId: "merman-modern",
  });
  assert.ok(result);
  return result.kind;
}

function catalog({
  fullyAvailable = false,
}: {
  fullyAvailable?: boolean;
} = {}): PresentationCatalog {
  return {
    schema_version: 1,
    theme_presets: [],
    profiles: [
      {
        id: "merman-modern",
        fully_available: fullyAvailable,
        missing_capability_ids: fullyAvailable ? [] : ["layout-elk"],
        aspects: [
          {
            id: "global-defaults",
            applicability: { kind: "all-diagrams", family_id: null },
            required_capability_id: null,
            available: true,
            missing_capability_ids: [],
          },
          {
            id: "flowchart-elk-default",
            applicability: { kind: "family", family_id: "flowchart" },
            required_capability_id: "layout-elk",
            available: fullyAvailable,
            missing_capability_ids: fullyAvailable ? [] : ["layout-elk"],
          },
        ],
      },
    ],
  };
}

function plan(
  state: "active" | "inactive" | "blocked",
  ready: boolean,
  diagramType = "flowchart-v2",
): SvgPlanResult {
  return {
    schema_version: 1,
    planned_operation_id: "svg",
    diagram_type: diagramType,
    presentation_profile_id: "merman-modern",
    presentation_aspects: [
      {
        id: "global-defaults",
        state: "active",
        required_capability_id: null,
      },
      {
        id: "flowchart-elk-default",
        state,
        required_capability_id: "layout-elk",
      },
    ],
    required_capability_ids: state === "blocked" ? ["layout-elk"] : [],
    missing_capability_ids: state === "blocked" ? ["layout-elk"] : [],
    ready,
  };
}

function detection(
  diagramType: "flowchart" | "sequence",
  effectiveLayoutId: string,
): DiagramDetectionFacts {
  return {
    status: "available",
    validity: "valid",
    diagramType,
    syntaxId: `${diagramType}-syntax`,
    effectiveLayoutId,
  };
}
