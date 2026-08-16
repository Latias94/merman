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

const FROZEN_EXPORT_TARGET: unique symbol = Symbol("FrozenExportTarget");

export interface FrozenExportTarget {
  readonly [FROZEN_EXPORT_TARGET]: true;
  readonly engine: ArtifactEngine;
  readonly filename: string;
  readonly publicationId: RenderPublicationId;
  readonly svgArtifact: NavigableInlineSvg;
}

export interface FreezeExportTargetCommand {
  readonly engine: ArtifactEngine;
  readonly publicationId: RenderPublicationId;
}

export interface ExportTargetOwner {
  freeze(command: FreezeExportTargetCommand): FrozenExportTarget;
  rasterArtifact(target: FrozenExportTarget): NavigableInlineSvg;
}

interface MermaidRasterAuthority {
  readonly kind: "artifact";
  readonly artifact: NavigableInlineSvg;
}

interface MermanRasterAuthority {
  readonly kind: "merman-operation";
  readonly operation: FrozenRenderOperation;
  readonly runtimeState: MermanRuntimeState;
  artifact?: NavigableInlineSvg;
}

type RasterAuthority = MermaidRasterAuthority | MermanRasterAuthority;

interface ArtifactActionBase {
  readonly publicationId: RenderPublicationId;
}

export type ArtifactActionCommand =
  | (ArtifactActionBase & { readonly action: "copy-ascii" })
  | (ArtifactActionBase & { readonly action: "download-ascii" })
  | (ArtifactActionBase & {
      readonly action: "copy-svg";
      readonly engine: ArtifactEngine;
    });

export interface ArtifactActionOwner {
  (command: ArtifactActionCommand): Promise<void>;
}

export interface ArtifactActionIo {
  copyAscii(ascii: string): Promise<void>;
  copySvg(artifact: NavigableInlineSvg): Promise<void>;
  downloadAscii(ascii: string, filename: string): void;
}

export interface ArtifactActionDependencies {
  readonly getRenderState: () => RenderCoordinatorState;
  readonly io: ArtifactActionIo;
}

export interface ExportTargetDependencies {
  readonly getRenderState: () => RenderCoordinatorState;
  readonly getRuntimeState: () => MermanRuntimeState;
}

export function createExportTargetOwner({
  getRenderState,
  getRuntimeState,
}: ExportTargetDependencies): ExportTargetOwner {
  const authorities = new WeakMap<object, RasterAuthority>();

  return Object.freeze({
    freeze(command: FreezeExportTargetCommand): FrozenExportTarget {
      const publication = currentPublication(
        getRenderState(),
        command.publicationId,
      );
      const svgArtifact = currentSvg(publication, command.engine);
      const target = Object.freeze({
        [FROZEN_EXPORT_TARGET]: true as const,
        engine: command.engine,
        filename: `${command.engine}-diagram`,
        publicationId: command.publicationId,
        svgArtifact,
      });
      authorities.set(
        target,
        command.engine === "mermaid"
          ? { kind: "artifact", artifact: svgArtifact }
          : {
              kind: "merman-operation",
              operation: publication.snapshot.operation,
              runtimeState: getRuntimeState(),
            },
      );
      return target;
    },

    rasterArtifact(target: FrozenExportTarget): NavigableInlineSvg {
      const authority = authorities.get(target);
      if (!authority) {
        throw actionError(
          "invalid-command",
          "The export target does not belong to this owner.",
        );
      }
      if (authority.kind === "artifact") return authority.artifact;
      if (authority.artifact) return authority.artifact;

      const runtimeState = authority.runtimeState;
      if (runtimeState.status !== "ready") {
        throw actionError(
          "runtime-unavailable",
          "Merman runtime was unavailable when this export target was opened.",
        );
      }
      if (
        runtimeState.facade.packageVersion !==
        authority.operation.versions.merman
      ) {
        throw actionError(
          "runtime-version-mismatch",
          "The Merman runtime did not match this export target.",
        );
      }
      authority.artifact = renderMermanRasterArtifact(
        runtimeState.facade,
        authority.operation,
      );
      return authority.artifact;
    },
  });
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
  | { readonly action: "copy-svg"; readonly artifact: NavigableInlineSvg };

export function createArtifactActionOwner({
  getRenderState,
  io,
}: ArtifactActionDependencies): ArtifactActionOwner {
  return async (command: ArtifactActionCommand): Promise<void> => {
    const plan = freezeActionPlan(command, getRenderState());
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
    }
  };
}

function freezeActionPlan(
  command: ArtifactActionCommand,
  renderState: RenderCoordinatorState
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
  throw actionError("invalid-command", "Unsupported artifact action.");
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
  if (publication.ascii.status !== "success") {
    throw actionError("artifact-unavailable", "ASCII artifact is unavailable.");
  }
  return publication.ascii.artifact;
}

function currentSvg(
  publication: CompletedRenderBatch,
  engine: ArtifactEngine
): NavigableInlineSvg {
  return engine === "merman"
    ? successfulMermanArtifact(publication).artifact
    : successfulMermaidArtifact(publication).artifact;
}

function renderMermanRasterArtifact(
  facade: MermanDomainFacade,
  operation: FrozenRenderOperation,
): NavigableInlineSvg {
  let result;
  try {
    result = facade.render(
      renderOperationWithSvgPipeline(operation, "resvg-safe"),
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
