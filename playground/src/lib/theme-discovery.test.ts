import assert from "node:assert/strict";
import test from "node:test";
import type { SvgRenderMetadata, ThemeExecutionEvidenceV1 } from "@mermanjs/web";
import type { CompletedRenderBatch, RenderPublicationId } from "../runtime/render-coordinator.ts";
import type { NavigableInlineSvg } from "../runtime/render-artifact.ts";
import {
  boundedThemeReasonCodes,
  currentDiagramDetection,
  currentThemeRender,
  detectedThemeFamily,
  readThemeExecutionEvidence,
  reviewedThemeFamilies,
  themeEvidenceOutcome,
  themeFamilyTreatment,
  themeFamilyMetadataId,
  themeTargetOutcome,
} from "./theme-discovery.ts";
import { mergeRecommendedMermaidConfig } from "./mermaid-config.ts";
import { beginThemeEditSession, beginThemeImport, completeThemeImport, rejectThemeValidation, themeValidationToken } from "./theme-edit-session.ts";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "./workspace-snapshot.ts";

const workspace = { ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "catalog-preset" };
const evidence: ThemeExecutionEvidenceV1 = {
  version: 1, family_id: "flowchart", theme_status: "verified", output_mutated: true,
  target_kind: "inline_svg", target_status: "host_dependent", target_reason_ids: ["host-font"], font_source: "browser",
};
const metadata: SvgRenderMetadata = {
  version: 1, operation_id: "svg", media_type: "image/svg+xml", runtime_policy: "default", byte_length: 6,
  theme_execution_evidence: evidence,
};
const publicationId = 1 as RenderPublicationId;
const batch: CompletedRenderBatch = {
  status: "success", publishedAt: 1, diagnostics: null, ascii: null, mermaid: null,
  detection: { status: "available", diagramType: "flowchart", syntaxId: "flowchart-v2", effectiveLayoutId: "dagre", validity: "valid" },
  snapshot: { publicationId, operation: {
    source: workspace.code, configuredSource: workspace.code, configurationError: null,
    bindingOptions: { version: 3 }, bindingOptionsJson: null,
    theme: workspace.diagramTheme, configJson: workspace.mermaidConfig,
    themePresetId: workspace.themePresetId, themeRecipeJson: workspace.themeRecipeJson,
    textMeasurementMode: workspace.textMeasurementMode, diagramFont: workspace.diagramFont, svgPipeline: workspace.svgPipeline,
    layoutEnvironment: { containerWidth: 800, containerHeight: 600 }, viewport: { width: 800, height: 600 },
    versions: { merman: "test", mermaid: "test" }, asciiEnabled: false, compareEnabled: false, diagnosticsEnabled: false,
  } },
  merman: { status: "success", engine: "merman", metadata, renderTimeMs: 1, presentedAt: 1,
    artifact: { svg: "<svg/>", kind: "navigable-inline-svg" } as NavigableInlineSvg },
};
const familyCapabilities = [{ diagram_type: "flowchart-v2", family_id: "flowchart", metadata_id: "flowchart" }];

test("recommended settings recursively preserve unrelated Mermaid config without mutating the catalog", () => {
  const original = {
    theme: "forest", layout: "elk", htmlLabels: false,
    flowchart: { look: "handDrawn", layout: "dagre", curve: "linear", nested: { retain: true, replace: 1 } },
    sequence: { actorMargin: 60 }, themeVariables: { primaryColor: "#123456" }, custom: [1, 2],
  };
  const recommended = Object.freeze({ flowchart: Object.freeze({ look: "classic", nested: { replace: 2 } }) });
  const merged = JSON.parse(mergeRecommendedMermaidConfig(JSON.stringify(original), recommended));
  assert.deepEqual(merged, {
    ...original, flowchart: { ...original.flowchart, look: "classic", nested: { retain: true, replace: 2 } },
  });
  assert.equal(original.flowchart.look, "handDrawn");
  assert.deepEqual(recommended.flowchart.nested, { replace: 2 });
});

test("recommended settings replace only named scalar or array keys and handle empty config", () => {
  assert.deepEqual(JSON.parse(mergeRecommendedMermaidConfig("  ", { flowchart: { look: "classic" } })), { flowchart: { look: "classic" } });
  assert.deepEqual(JSON.parse(mergeRecommendedMermaidConfig('{"flowchart":false,"keep":[1]}', { flowchart: { look: "classic" } })), {
    flowchart: { look: "classic" }, keep: [1],
  });
  assert.deepEqual(JSON.parse(mergeRecommendedMermaidConfig('{"a":[1,2],"b":{"keep":true}}', { a: [3] })), { a: [3], b: { keep: true } });
});

test("invalid Mermaid config is rejected before recommended settings can replace it", () => {
  for (const raw of ["{", "[]", "null", "1", '"text"']) {
    assert.throws(() => mergeRecommendedMermaidConfig(raw, { flowchart: { look: "classic" } }));
  }
});

test("recommended merging rejects prototype pollution keys at any depth in either input", () => {
  for (const raw of [
    '{"__proto__":{"themePolluted":true}}',
    '{"flowchart":{"constructor":{"prototype":{"themePolluted":true}}}}',
    '{"custom":[{"prototype":{"themePolluted":true}}]}',
  ]) {
    assert.throws(() => mergeRecommendedMermaidConfig("{}", JSON.parse(raw)), /reserved key/);
    assert.throws(() => mergeRecommendedMermaidConfig(raw, { flowchart: { look: "classic" } }), /reserved key/);
  }
  assert.equal(Object.hasOwn(Object.prototype, "themePolluted"), false);
});

test("only explicit known family treatments are reviewed, independently of preset IDs", () => {
  const preset = { family_designs: [
    { family_id: "pie", treatment: "dedicated" },
    { family_id: "class", treatment: "base_only" },
    { family_id: "packet", treatment: "future-treatment" },
  ] };
  assert.equal(themeFamilyTreatment(preset, "pie"), "dedicated");
  assert.equal(themeFamilyTreatment(preset, "class"), "base_only");
  for (const family of ["packet", "flowchart", "unknown", null]) {
    assert.equal(themeFamilyTreatment(preset, family), "unreviewed");
  }
  assert.deepEqual(reviewedThemeFamilies(preset).map((design) => design.family_id), ["pie", "class"]);
});

test("family identity comes from current detection and the runtime family catalog", () => {
  assert.equal(detectedThemeFamily(batch.detection, familyCapabilities), "flowchart");
  assert.equal(detectedThemeFamily(batch.detection, []), null);
  assert.equal(detectedThemeFamily(null, familyCapabilities), null);
  assert.equal(themeFamilyMetadataId("flowchart", familyCapabilities), "flowchart");
  assert.equal(themeFamilyMetadataId("future-family", familyCapabilities), null);
});

test("Git Graph and Quadrant Chart resolve syntax identity and existing metadata translation keys", () => {
  for (const [syntaxId, metadataId] of [["gitGraph", "gitgraph"], ["quadrantChart", "quadrantchart"]] as const) {
    const detection = { status: "available", validity: "valid", diagramType: metadataId, syntaxId, effectiveLayoutId: "none" } as const;
    const capabilities = [
      { diagram_type: metadataId, family_id: "wrong-family", metadata_id: "wrong-metadata" },
      { diagram_type: syntaxId, family_id: syntaxId, metadata_id: metadataId },
    ];
    const familyId = detectedThemeFamily(detection, capabilities);
    assert.equal(familyId, syntaxId);
    assert.equal(themeFamilyMetadataId(familyId!, capabilities), metadataId);
    assert.equal(themeFamilyMetadataId(syntaxId, [{ family_id: syntaxId, metadata_id: null }]), null);
  }
});

test("pending coordinator states hide previous theme evidence while unchanged detection remains visible", () => {
  const pending = { publicationId: 2 as RenderPublicationId };
  for (const state of [{ status: "empty" }, { status: "pending", snapshot: pending }] as const) {
    const current = currentThemeRender(state, workspace, null);
    assert.deepEqual(current, { status: "waiting", reason: "render", batch: null });
    assert.equal(currentDiagramDetection(state, workspace), null);
  }
  const updating = { status: "updating", previous: batch, snapshot: pending } as const;
  assert.deepEqual(currentThemeRender(updating, workspace, null), { status: "waiting", reason: "render", batch: null });
  assert.equal(currentDiagramDetection(updating, workspace), batch.detection);
  assert.equal(detectedThemeFamily(currentDiagramDetection(updating, workspace), familyCapabilities), "flowchart");
});

test("theme and output changes retain current-family choices but never inherit old theme evidence", () => {
  const updating = { status: "updating", previous: batch, snapshot: { publicationId: 2 as RenderPublicationId } } as const;
  for (const change of [
    { themePresetId: "new-preset" }, { themePresetId: null },
    { themePresetId: null, themeRecipeJson: '{"schema_version":1}' },
    { svgPipeline: "resvg-safe" as const }, { textMeasurementMode: "headless" as const },
  ]) {
    const changedWorkspace = { ...workspace, ...change };
    for (const state of [batch, updating]) {
      assert.equal(currentThemeRender(state, changedWorkspace, null).batch, null, JSON.stringify(change));
      const detection = currentDiagramDetection(state, changedWorkspace);
      assert.equal(detection, batch.detection, JSON.stringify(change));
      assert.equal(detectedThemeFamily(detection, familyCapabilities), "flowchart");
    }
  }
});

test("source, config, UI theme and font changes hide stale detection until current inputs finish", () => {
  const updating = { status: "updating", previous: batch, snapshot: { publicationId: 2 as RenderPublicationId } } as const;
  for (const change of [
    { code: 'pie\n"A": 1' }, { mermaidConfig: '{"flowchart":{"look":"classic"}}' },
    { diagramTheme: "dark" as const }, { diagramFont: "arial" as const },
  ]) {
    const changedWorkspace = { ...workspace, ...change };
    for (const state of [batch, updating]) {
      assert.equal(currentDiagramDetection(state, changedWorkspace), null, JSON.stringify(change));
      assert.equal(currentThemeRender(state, changedWorkspace, null).batch, null, JSON.stringify(change));
    }
    const currentBatch: CompletedRenderBatch = {
      ...batch, snapshot: { ...batch.snapshot, operation: {
        ...batch.snapshot.operation, source: changedWorkspace.code, configJson: changedWorkspace.mermaidConfig,
        theme: changedWorkspace.diagramTheme, diagramFont: changedWorkspace.diagramFont,
      } },
    };
    assert.equal(currentDiagramDetection(currentBatch, changedWorkspace), currentBatch.detection);
  }
});

test("a completed render is hidden as soon as any current render input changes, before scheduling", () => {
  assert.equal(currentThemeRender(batch, workspace, null).batch, batch);
  for (const change of [
    { code: "pie\n\"A\": 1" }, { mermaidConfig: '{"flowchart":{"look":"classic"}}' },
    { diagramTheme: "dark" as const }, { themePresetId: "new-preset" }, { themePresetId: null },
    { themePresetId: null, themeRecipeJson: '{"schema_version":1}' },
    { svgPipeline: "resvg-safe" as const }, { textMeasurementMode: "headless" as const }, { diagramFont: "arial" as const },
  ]) {
    assert.equal(currentThemeRender(batch, { ...workspace, ...change }, null).status, "waiting", JSON.stringify(change));
  }
});

test("reading, validating and invalid imports never report last-valid evidence as the imported draft", () => {
  const session = beginThemeEditSession(1, { kind: "preset", presetId: workspace.themePresetId });
  assert.equal(currentThemeRender(batch, workspace, session).status, "ready");
  const importing = beginThemeImport(session);
  assert.deepEqual(currentThemeRender(batch, workspace, importing), { status: "waiting", reason: "importing", batch: null });
  const pending = completeThemeImport(importing, { ...themeValidationToken(importing), generation: importing.importGeneration }, "{");
  assert.deepEqual(currentThemeRender(batch, workspace, pending), { status: "waiting", reason: "validating", batch: null });
  const invalid = rejectThemeValidation(pending, themeValidationToken(pending), "Invalid JSON");
  assert.deepEqual(currentThemeRender(batch, workspace, invalid), { status: "waiting", reason: "invalid", batch: null });
  const updating = { status: "updating", previous: batch, snapshot: { publicationId: 2 as RenderPublicationId } } as const;
  for (const draft of [importing, pending, invalid]) {
    for (const state of [batch, updating]) {
      assert.equal(currentThemeRender(state, workspace, draft).batch, null);
      const detection = currentDiagramDetection(state, workspace);
      assert.equal(detectedThemeFamily(detection, familyCapabilities), "flowchart");
      assert.equal(themeFamilyTreatment({ family_designs: [{ family_id: "flowchart", treatment: "dedicated" }] }, detectedThemeFamily(detection, familyCapabilities)), "dedicated");
    }
  }
});

test("current render failure is distinct from missing evidence or a missing theme", () => {
  const failed: CompletedRenderBatch = {
    ...batch, status: "failed", mermaid: null,
    merman: { status: "failure", engine: "merman", stage: "render", message: "Cannot render", detail: null },
  };
  assert.equal(currentThemeRender(failed, workspace, null).status, "failed");
  assert.equal(readThemeExecutionEvidence(null), null);
  assert.equal(themeEvidenceOutcome(null), "unavailable");
});

test("theme outcome and target or host boundary remain separate and unknown values stay unconfirmed", () => {
  for (const [status, outcome] of [
    ["verified", "applied"], ["residual", "residual"], ["incomplete", "incomplete"],
    ["not_applicable", "not_present"], ["future", "unavailable"],
  ]) {
    assert.equal(themeEvidenceOutcome({ ...evidence, theme_status: status }), outcome);
  }
  for (const [status, outcome] of [
    ["portable", "checked"], ["host_dependent", "host_dependent"], ["rejected", "rejected"],
    ["unverified", "unverified"], ["future", "unavailable"],
  ]) {
    assert.equal(themeTargetOutcome({ ...evidence, target_status: status, diagnostics: [] }), outcome);
  }
  assert.equal(readThemeExecutionEvidence(metadata), evidence);
  assert.equal(readThemeExecutionEvidence({ ...metadata, theme_execution_evidence: { version: 99, theme_status: "verified" } }), null);
  assert.equal(readThemeExecutionEvidence({ ...metadata, theme_execution_evidence: { version: 1, theme_status: "verified" } }), null);
  assert.equal(themeTargetOutcome(null), "unavailable");
});

test("readable reasons are bounded, deduplicated, and retain unknown same-render codes", () => {
  const codes = ["unsupported-effect", "unsupported-effect", "future.reason", "unsupported-fill", "unsupported-stroke", "compatibility-unverified", "unsupported-typography"];
  const original = [...codes];
  assert.deepEqual(boundedThemeReasonCodes(codes), {
    visible: ["unsupported-effect", "future.reason", "unsupported-fill", "unsupported-stroke"], remaining: 2,
  });
  assert.deepEqual(codes, original);
  assert.deepEqual(boundedThemeReasonCodes([]), { visible: [], remaining: 0 });
  assert.deepEqual(boundedThemeReasonCodes(["unknown-target-reason"]), { visible: ["unknown-target-reason"], remaining: 0 });
});
