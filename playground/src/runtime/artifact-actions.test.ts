import assert from "node:assert/strict";
import test from "node:test";

import { planPngRaster } from "../lib/png-export-plan.ts";
import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "../lib/workspace-snapshot.ts";
import {
  ArtifactActionError,
  createArtifactActionOwner,
  type ArtifactActionIo,
} from "./artifact-actions.ts";
import type {
  MermanDomainFacade,
  MermanRuntimeState,
} from "./merman-core.ts";
import {
  freezeRenderOperation,
  type ConfiguredMermanOperationInput,
} from "./merman-operation-input.ts";
import type {
  CompletedRenderBatch,
  RenderCoordinatorState,
  RenderPublicationId,
} from "./render-coordinator.ts";
import { projectNavigableInlineSvg } from "./render-artifact.ts";
import { MERMAID_JS_VERSION } from "./mermaid-requirements.ts";

test("selects SVG and ASCII only from the named current publication", async () => {
  const publication = completedPublication("current");
  const publicationId = publication.snapshot.publicationId;
  const calls: string[] = [];
  const owner = createArtifactActionOwner({
    getRenderState: () => publication,
    getRuntimeState: () => readyRuntime(),
    io: recordingIo(calls),
  });

  await owner({ action: "copy-svg", engine: "merman", publicationId });
  await owner({ action: "download-svg", engine: "mermaid", publicationId });
  await owner({ action: "copy-ascii", publicationId });
  await owner({ action: "download-ascii", publicationId });

  assert.deepEqual(calls, [
    "copy-svg:merman",
    "download-svg:mermaid:mermaid-diagram",
    "copy-ascii:ascii-current",
    "download-ascii:ascii-current:merman-diagram",
  ]);
});

test("rejects stale, missing, and updating publications before I/O", async () => {
  const publication = completedPublication("current");
  const currentId = publication.snapshot.publicationId;
  const staleId = publicationId(2);
  const calls: string[] = [];
  let state: RenderCoordinatorState = publication;
  const owner = createArtifactActionOwner({
    getRenderState: () => state,
    getRuntimeState: () => readyRuntime(),
    io: recordingIo(calls),
  });

  await assert.rejects(
    owner({ action: "copy-svg", engine: "merman", publicationId: staleId }),
    (error: unknown) =>
      error instanceof ArtifactActionError &&
      error.code === "publication-not-current"
  );
  state = Object.freeze({
    status: "updating",
    previous: publication,
    snapshot: completedPublication("next", staleId).snapshot,
  });
  await assert.rejects(
    owner({ action: "copy-svg", engine: "merman", publicationId: currentId }),
    (error: unknown) =>
      error instanceof ArtifactActionError &&
      error.code === "publication-not-current"
  );
  state = Object.freeze({ status: "empty" });
  await assert.rejects(
    owner({ action: "copy-svg", engine: "merman", publicationId: currentId })
  );
  assert.deepEqual(calls, []);
});

test("rerenders only Merman PNG through the resvg-safe operation", async () => {
  const publication = completedPublication("png");
  const publicationId = publication.snapshot.publicationId;
  const renderInputs: ConfiguredMermanOperationInput[] = [];
  const runtime = readyRuntime((input) => {
    renderInputs.push(input);
    return {
      artifact: projectNavigableInlineSvg(svg("merman-png")),
      error: null,
      renderTime: 1,
      status: "success",
    };
  });
  const calls: string[] = [];
  const owner = createArtifactActionOwner({
    getRenderState: () => publication,
    getRuntimeState: () => runtime,
    io: recordingIo(calls),
  });

  const merman = await owner({
    action: "download-png",
    engine: "merman",
    publicationId,
    scale: 2,
  });
  const mermaid = await owner({
    action: "download-png",
    engine: "mermaid",
    publicationId,
    scale: 3,
  });

  assert.equal(renderInputs.length, 1);
  assert.deepEqual(renderInputs[0].bindingOptions.svg, {
    pipeline: "resvg-safe",
  });
  assert.equal(Object.isFrozen(merman), true);
  assert.equal(Object.isFrozen(mermaid), true);
  assert.deepEqual(calls, [
    "download-png:merman-png:merman-diagram:2",
    "download-png:mermaid:mermaid-diagram:3",
  ]);
});

test("preserves structured Resvg render failures", async () => {
  const publication = completedPublication("failure");
  const owner = createArtifactActionOwner({
    getRenderState: () => publication,
    getRuntimeState: () =>
      readyRuntime(() => ({
        artifact: null,
        error: Object.freeze({
          summary: "Resvg projection failed.",
          detail: '{"code":"MERMAN_RESVG"}',
        }),
        renderTime: 0,
        stage: "render",
        status: "failure",
      })),
    io: recordingIo([]),
  });

  await assert.rejects(
    owner({
      action: "download-png",
      engine: "merman",
      publicationId: publication.snapshot.publicationId,
    }),
    (error: unknown) => {
      assert.ok(error instanceof ArtifactActionError);
      assert.equal(error.code, "svg-render-failed");
      assert.equal(error.stage, "render");
      assert.match(error.detail ?? "", /MERMAN_RESVG/);
      return true;
    }
  );
});

function completedPublication(
  key: string,
  id: RenderPublicationId = publicationId(1)
): CompletedRenderBatch {
  const operation = freezeRenderOperation({
    compareEnabled: true,
    diagnosticsEnabled: false,
    layoutEnvironment: { containerWidth: 800, containerHeight: 600 },
    versions: { merman: "test-merman", mermaid: MERMAID_JS_VERSION },
    viewport: { width: 800, height: 600 },
    workspace: workspace(key),
  });
  return Object.freeze({
    detection: Object.freeze({
      status: "available",
      validity: "valid",
      diagramType: "flowchart",
      syntaxId: "flowchart-v2",
      effectiveLayoutId: "dagre",
    }),
    diagnostics: null,
    publishedAt: 1,
    snapshot: Object.freeze({ operation, publicationId: id }),
    svgPlan: null,
    status: "success",
    merman: Object.freeze({
      artifact: projectNavigableInlineSvg(svg("merman")),
      ascii: `ascii-${key}`,
      asciiError: null,
      engine: "merman",
      presentedAt: null,
      renderTimeMs: 1,
      status: "success",
    }),
    mermaid: Object.freeze({
      artifact: projectNavigableInlineSvg(svg("mermaid")),
      engine: "mermaid",
      prepareTimeMs: 1,
      presentationTimeMs: 1,
      presentedAt: null,
      renderTimeMs: 1,
      status: "success",
      version: MERMAID_JS_VERSION,
    }),
  });
}

function workspace(code: string): WorkspaceSnapshot {
  return {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    code,
    mermaidConfig: "{}",
  };
}

function publicationId(value: number): RenderPublicationId {
  return value as RenderPublicationId;
}

function readyRuntime(
  render: MermanDomainFacade["render"] = (input) => ({
    artifact: projectNavigableInlineSvg(svg(input.source)),
    error: null,
    renderTime: 1,
    status: "success",
  })
): MermanRuntimeState {
  return {
    status: "ready",
    suspended: false,
    facade: { packageVersion: "test-merman", render } as MermanDomainFacade,
  };
}

function recordingIo(calls: string[]): ArtifactActionIo {
  return {
    async copyAscii(ascii) {
      calls.push(`copy-ascii:${ascii}`);
    },
    async copySvg(artifact) {
      calls.push(`copy-svg:${label(artifact.svg)}`);
    },
    downloadAscii(ascii, filename) {
      calls.push(`download-ascii:${ascii}:${filename}`);
    },
    async downloadPng(artifact, filename, scale) {
      calls.push(`download-png:${label(artifact.svg)}:${filename}:${scale}`);
      return planPngRaster(100, 50, scale);
    },
    downloadSvg(artifact, filename) {
      calls.push(`download-svg:${label(artifact.svg)}:${filename}`);
    },
  };
}

function label(value: string): string {
  return /<text>([^<]+)<\/text>/.exec(value)?.[1] ?? "unknown";
}

function svg(value: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg"><text>${value}</text></svg>`;
}
