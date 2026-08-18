import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  type WorkspaceSnapshot,
} from "../lib/workspace-snapshot.ts";
import {
  ArtifactActionError,
  createArtifactActionOwner,
  createExportTargetOwner,
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

test("selects copied SVG and ASCII only from the named current publication", async () => {
  const publication = completedPublication("current");
  const publicationId = publication.snapshot.publicationId;
  const calls: string[] = [];
  const owner = createArtifactActionOwner({
    getRenderState: () => publication,
    getRuntimeState: () => readyRuntime(),
    io: recordingIo(calls),
  });

  await owner({ action: "copy-svg", engine: "merman", publicationId });
  await owner({ action: "copy-svg", engine: "mermaid", publicationId });
  await owner({ action: "copy-ascii", publicationId });
  await owner({ action: "download-ascii", publicationId });

  assert.deepEqual(calls, [
    "copy-svg:merman",
    "copy-svg:mermaid",
    "copy-ascii:ascii-current",
    "download-ascii:ascii-current:merman-diagram",
  ]);
});

test("renders and caches ASCII only after an explicit artifact action", async () => {
  const completed = completedPublication("on-demand");
  const publication: CompletedRenderBatch = Object.freeze({
    ...completed,
    ascii: null,
  });
  const renderInputs: ConfiguredMermanOperationInput[] = [];
  const calls: string[] = [];
  const owner = createArtifactActionOwner({
    getRenderState: () => publication,
    getRuntimeState: () =>
      readyRuntime(undefined, (input) => {
        renderInputs.push(input);
        return {
          ascii: "ascii-on-demand",
          error: null,
          status: "success",
        };
      }),
    io: recordingIo(calls),
  });

  await owner({
    action: "copy-ascii",
    publicationId: publication.snapshot.publicationId,
  });
  await owner({
    action: "download-ascii",
    publicationId: publication.snapshot.publicationId,
  });

  assert.equal(renderInputs.length, 1);
  assert.equal(renderInputs[0], publication.snapshot.operation);
  assert.deepEqual(calls, [
    "copy-ascii:ascii-on-demand",
    "download-ascii:ascii-on-demand:merman-diagram",
  ]);
});

test("publishes ASCII actions independently from a failed Merman SVG", async () => {
  const successful = completedPublication("ascii-only");
  const publication: CompletedRenderBatch = Object.freeze({
    ...successful,
    status: "failed",
    merman: Object.freeze({
      detail: null,
      engine: "merman",
      message: "Unsafe SVG.",
      stage: "svg-validation",
      status: "failure",
    }),
    mermaid: null,
  });
  const calls: string[] = [];
  const owner = createArtifactActionOwner({
    getRenderState: () => publication,
    getRuntimeState: () => readyRuntime(),
    io: recordingIo(calls),
  });

  await owner({
    action: "copy-ascii",
    publicationId: publication.snapshot.publicationId,
  });
  await owner({
    action: "download-ascii",
    publicationId: publication.snapshot.publicationId,
  });

  assert.deepEqual(calls, [
    "copy-ascii:ascii-ascii-only",
    "download-ascii:ascii-ascii-only:merman-diagram",
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

test("freezes an export target that does not retarget after a newer publication", () => {
  const first = completedPublication("first");
  let state: RenderCoordinatorState = first;
  const renderInputs: ConfiguredMermanOperationInput[] = [];
  const targetOwner = createExportTargetOwner({
    getRenderState: () => state,
    getRuntimeState: () =>
      readyRuntime((input) => {
        renderInputs.push(input);
        return {
          artifact: projectNavigableInlineSvg(svg("frozen-raster")),
          error: null,
          renderTime: 1,
          status: "success",
        };
      }),
  });
  const target = targetOwner.freeze({
    engine: "merman",
    publicationId: first.snapshot.publicationId,
  });
  const mermaidTarget = targetOwner.freeze({
    engine: "mermaid",
    publicationId: first.snapshot.publicationId,
  });

  state = completedPublication("second", publicationId(2));
  assert.equal(label(target.svgArtifact.svg), "merman");
  assert.equal(label(mermaidTarget.svgArtifact.svg), "mermaid");
  assert.equal(target.publicationId, first.snapshot.publicationId);
  assert.equal(label(targetOwner.rasterArtifact(target).svg), "frozen-raster");
  assert.equal(label(targetOwner.rasterArtifact(target).svg), "frozen-raster");
  assert.equal(renderInputs.length, 1);
  assert.deepEqual(renderInputs[0]?.bindingOptions.svg, {
    pipeline: "resvg-safe",
  });
  assert.equal(
    targetOwner.rasterArtifact(mermaidTarget),
    mermaidTarget.svgArtifact,
  );

  assert.throws(() =>
    targetOwner.freeze({
      engine: "mermaid",
      publicationId: first.snapshot.publicationId,
    }),
  );
});

test("preserves structured Resvg render failures", () => {
  const publication = completedPublication("failure");
  const owner = createExportTargetOwner({
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
  });
  const target = owner.freeze({
    engine: "merman",
    publicationId: publication.snapshot.publicationId,
  });

  assert.throws(
    () => owner.rasterArtifact(target),
    (error: unknown) => {
      assert.ok(error instanceof ArtifactActionError);
      assert.equal(error.code, "svg-render-failed");
      assert.equal(error.stage, "render");
      assert.match(error.detail ?? "", /MERMAN_RESVG/);
      return true;
    },
  );
});

function completedPublication(
  key: string,
  id: RenderPublicationId = publicationId(1)
): CompletedRenderBatch {
  const operation = freezeRenderOperation({
    asciiEnabled: true,
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
    ascii: Object.freeze({
      artifact: `ascii-${key}`,
      status: "success",
    }),
    publishedAt: 1,
    snapshot: Object.freeze({ operation, publicationId: id }),
    svgPlan: null,
    status: "success",
    merman: Object.freeze({
      artifact: projectNavigableInlineSvg(svg("merman")),
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
  }),
  renderAscii: MermanDomainFacade["renderAscii"] = () => ({
    ascii: "ascii",
    error: null,
    status: "success",
  }),
): MermanRuntimeState {
  return {
    status: "ready",
    suspended: false,
    facade: {
      packageVersion: "test-merman",
      render,
      renderAscii,
    } as MermanDomainFacade,
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
  };
}

function label(value: string): string {
  return /<text>([^<]+)<\/text>/.exec(value)?.[1] ?? "unknown";
}

function svg(value: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg"><text>${value}</text></svg>`;
}
