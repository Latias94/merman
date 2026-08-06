import type { PngRasterPlan } from "../lib/png-export-plan.ts";
import type { MermanDomainFacade, MermanRuntimeState } from "./merman-core.ts";
import { projectError, type ErrorProjection } from "./error-projection.ts";
import {
  renderOperationWithSvgPipeline,
  type FrozenRenderOperation,
} from "./merman-operation-input.ts";
import {
  isCompletedRenderState,
  type CompletedRenderBatch,
  type MermaidRenderSuccess,
  type MermanRenderSuccess,
  type RenderCoordinatorState,
  type RenderPublicationId,
} from "./render-coordinator.ts";
import {
  assertNavigableInlineSvgArtifact,
  type NavigableInlineSvg,
} from "./render-artifact.ts";

export type ArtifactEngine = "merman" | "mermaid";

interface ArtifactActionBase {
  readonly publicationId: RenderPublicationId;
}

export type ArtifactActionCommand =
  | (ArtifactActionBase & { readonly action: "copy-ascii" })
  | (ArtifactActionBase & { readonly action: "download-ascii" })
  | (ArtifactActionBase & {
      readonly action: "copy-svg";
      readonly engine: ArtifactEngine;
    })
  | (ArtifactActionBase & {
      readonly action: "download-svg";
      readonly engine: ArtifactEngine;
    })
  | (ArtifactActionBase & {
      readonly action: "download-png";
      readonly engine: ArtifactEngine;
      readonly scale?: number;
    });

type DownloadPngCommand = Extract<
  ArtifactActionCommand,
  { readonly action: "download-png" }
>;

type NonPngCommand = Exclude<ArtifactActionCommand, DownloadPngCommand>;

export interface ArtifactActionOwner {
  (command: DownloadPngCommand): Promise<PngRasterPlan>;
  (command: NonPngCommand): Promise<void>;
}

export interface ArtifactActionIo {
  copyAscii(ascii: string): Promise<void>;
  copySvg(artifact: NavigableInlineSvg): Promise<void>;
  downloadAscii(ascii: string, filename: string): void;
  downloadPng(
    artifact: NavigableInlineSvg,
    filename: string,
    scale: number
  ): Promise<PngRasterPlan>;
  downloadSvg(artifact: NavigableInlineSvg, filename: string): void;
}

export interface ArtifactActionDependencies {
  readonly getRenderState: () => RenderCoordinatorState;
  readonly getRuntimeState: () => MermanRuntimeState;
  readonly io: ArtifactActionIo;
}

export type ArtifactActionErrorCode =
  | "artifact-unavailable"
  | "invalid-command"
  | "publication-not-current"
  | "runtime-unavailable"
  | "runtime-version-mismatch"
  | "svg-render-failed";

export class ArtifactActionError extends Error {
  readonly code: ArtifactActionErrorCode;
  readonly detail: string | null;
  readonly projection: ErrorProjection;
  readonly stage: string | null;

  constructor(
    code: ArtifactActionErrorCode,
    projection: ErrorProjection,
    stage: string | null = null
  ) {
    super(projection.summary);
    this.name = "ArtifactActionError";
    this.code = code;
    this.detail = projection.detail;
    this.projection = projection;
    this.stage = stage;
  }
}

type FrozenActionPlan =
  | { readonly action: "copy-ascii"; readonly ascii: string }
  | {
      readonly action: "download-ascii";
      readonly ascii: string;
      readonly filename: string;
    }
  | { readonly action: "copy-svg"; readonly artifact: NavigableInlineSvg }
  | {
      readonly action: "download-svg";
      readonly artifact: NavigableInlineSvg;
      readonly filename: string;
    }
  | {
      readonly action: "download-png";
      readonly artifact: NavigableInlineSvg;
      readonly engine: "mermaid";
      readonly filename: string;
      readonly scale: number;
    }
  | {
      readonly action: "download-png";
      readonly engine: "merman";
      readonly facade: MermanDomainFacade;
      readonly filename: string;
      readonly operation: FrozenRenderOperation;
      readonly scale: number;
    };

export function createArtifactActionOwner({
  getRenderState,
  getRuntimeState,
  io,
}: ArtifactActionDependencies): ArtifactActionOwner {
  const execute = async (
    command: ArtifactActionCommand
  ): Promise<PngRasterPlan | void> => {
    const plan = freezeActionPlan(command, getRenderState(), getRuntimeState);
    switch (plan.action) {
      case "copy-ascii":
        await io.copyAscii(plan.ascii);
        return;
      case "copy-svg":
        await io.copySvg(plan.artifact);
        return;
      case "download-ascii":
        io.downloadAscii(plan.ascii, plan.filename);
        return;
      case "download-svg":
        io.downloadSvg(plan.artifact, plan.filename);
        return;
      case "download-png":
        return io.downloadPng(
          artifactForPng(plan),
          plan.filename,
          plan.scale
        );
    }
  };
  return execute as ArtifactActionOwner;
}

function freezeActionPlan(
  command: ArtifactActionCommand,
  renderState: RenderCoordinatorState,
  getRuntimeState: () => MermanRuntimeState
): FrozenActionPlan {
  const publication = currentPublication(renderState, command.publicationId);
  if (command.action === "copy-ascii") {
    return Object.freeze({
      action: command.action,
      ascii: currentAscii(publication),
    });
  }
  if (command.action === "download-ascii") {
    return Object.freeze({
      action: command.action,
      ascii: currentAscii(publication),
      filename: "merman-diagram",
    });
  }
  if (command.action === "copy-svg") {
    return Object.freeze({
      action: command.action,
      artifact: currentSvg(publication, command.engine),
    });
  }
  if (command.action === "download-svg") {
    return Object.freeze({
      action: command.action,
      artifact: currentSvg(publication, command.engine),
      filename: `${command.engine}-diagram`,
    });
  }

  const scale = command.scale ?? 2;
  if (!Number.isFinite(scale) || scale <= 0) {
    throw actionError("invalid-command", "PNG scale must be positive.");
  }
  if (command.engine === "mermaid") {
    return Object.freeze({
      action: command.action,
      artifact: successfulMermaidArtifact(publication).artifact,
      engine: command.engine,
      filename: "mermaid-diagram",
      scale,
    });
  }

  const runtimeState = getRuntimeState();
  if (runtimeState.status !== "ready") {
    throw actionError("runtime-unavailable", "Merman runtime is unavailable.");
  }
  if (
    runtimeState.facade.packageVersion !==
    publication.snapshot.operation.versions.merman
  ) {
    throw actionError(
      "runtime-version-mismatch",
      "The active Merman runtime does not match this render publication."
    );
  }
  successfulMermanArtifact(publication);
  return Object.freeze({
    action: command.action,
    engine: command.engine,
    facade: runtimeState.facade,
    filename: "merman-diagram",
    operation: publication.snapshot.operation,
    scale,
  });
}

function currentPublication(
  state: RenderCoordinatorState,
  publicationId: RenderPublicationId
): CompletedRenderBatch {
  if (
    !isCompletedRenderState(state) ||
    state.snapshot.publicationId !== publicationId
  ) {
    throw actionError(
      "publication-not-current",
      "The selected render publication is no longer current."
    );
  }
  return state;
}

function currentAscii(publication: CompletedRenderBatch): string {
  const merman = successfulMermanArtifact(publication);
  if (merman.ascii === null) {
    throw actionError("artifact-unavailable", "ASCII artifact is unavailable.");
  }
  return merman.ascii;
}

function currentSvg(
  publication: CompletedRenderBatch,
  engine: ArtifactEngine
): NavigableInlineSvg {
  return engine === "merman"
    ? successfulMermanArtifact(publication).artifact
    : successfulMermaidArtifact(publication).artifact;
}

function artifactForPng(
  plan: Extract<FrozenActionPlan, { readonly action: "download-png" }>
): NavigableInlineSvg {
  if (plan.engine === "mermaid") return plan.artifact;

  let result;
  try {
    result = plan.facade.render(
      renderOperationWithSvgPipeline(plan.operation, "resvg-safe")
    );
  } catch (error) {
    throw new ArtifactActionError(
      "svg-render-failed",
      projectError(error),
      "render"
    );
  }
  if (result.status === "failure") {
    throw new ArtifactActionError(
      "svg-render-failed",
      projectError(result.error),
      result.stage
    );
  }
  try {
    assertNavigableInlineSvgArtifact(result.artifact);
  } catch (error) {
    throw new ArtifactActionError(
      "svg-render-failed",
      projectError(error),
      "svg-validation"
    );
  }
  return result.artifact;
}

function successfulMermanArtifact(
  publication: CompletedRenderBatch
): MermanRenderSuccess {
  if (publication.merman.status !== "success") {
    throw actionError("artifact-unavailable", "Merman artifact is unavailable.");
  }
  return publication.merman;
}

function successfulMermaidArtifact(
  publication: CompletedRenderBatch
): MermaidRenderSuccess {
  if (!publication.mermaid || publication.mermaid.status !== "success") {
    throw actionError("artifact-unavailable", "Mermaid artifact is unavailable.");
  }
  return publication.mermaid;
}

function actionError(
  code: ArtifactActionErrorCode,
  message: string
): ArtifactActionError {
  return new ArtifactActionError(code, projectError(message));
}
