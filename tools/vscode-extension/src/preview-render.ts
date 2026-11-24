import { createHash } from "node:crypto";

import { errorMessage } from "./error-message.js";
import { samePreviewRenderKey, type PreviewSnapshot } from "./preview-model.js";
import {
  snapshotMessagePayload,
  type PreviewToWebviewMessage,
} from "./preview-messages.js";
import type { PreviewUpdateReason } from "./preview-policy.js";
import type { PreviewInput } from "./preview-source.js";

export interface PreviewRenderContent {
  content: string;
  previewLayoutFrame?: Buffer;
}

export interface PreviewRenderHost {
  beforeRender?(): Promise<void>;
  renderContent(
    snapshot: PreviewSnapshot,
    signal: AbortSignal,
    previousLayoutFrame: Buffer | undefined,
  ): Promise<PreviewRenderContent>;
  postMessage(message: PreviewToWebviewMessage): Promise<void>;
  info(message: string): void;
  error(message: string): void;
  isCurrentRequest(requestId: number): boolean;
  markRendered(requestId: number, snapshot: PreviewSnapshot, content: string): void;
}

interface LayoutFrameSource {
  documentUri: string;
  documentVersion: number;
  sourceId: string;
  kind: PreviewInput["kind"];
  sourceHash: string;
  fitWidth?: number;
  fence?: {
    range: PreviewInput["sourceRange"];
    siblings: LayoutFrameFence[];
  };
}

interface LayoutFrameFence {
  sourceId: string;
  sourceHash: string;
  range: PreviewInput["sourceRange"];
}

export class PreviewRenderQueue {
  private requestId = 0;
  private pending: { snapshot: PreviewSnapshot; abortController: AbortController } | undefined;
  private layoutFrame: {
    source: LayoutFrameSource;
    frame: Buffer;
  } | undefined;

  reset(): void {
    this.cancelPending();
    this.layoutFrame = undefined;
  }

  cancelPending(): number {
    this.requestId += 1;
    this.pending?.abortController.abort();
    this.pending = undefined;
    return this.requestId;
  }

  currentRequestId(): number {
    return this.requestId;
  }

  hasPending(snapshot: PreviewSnapshot): boolean {
    return !!this.pending && samePreviewRenderKey(this.pending.snapshot, snapshot);
  }

  async render(
    snapshot: PreviewSnapshot,
    reason: PreviewUpdateReason,
    host: PreviewRenderHost,
  ): Promise<void> {
    const requestId = ++this.requestId;
    this.pending?.abortController.abort();
    const abortController = new AbortController();
    const pending = { snapshot, abortController };
    this.pending = pending;
    const snapshotPayload = snapshotMessagePayload(snapshot);
    const source = snapshot.displayMode === "svg" ? layoutFrameSource(snapshot) : undefined;
    if (!source || (this.layoutFrame && !continuesLayoutFrame(this.layoutFrame.source, source))) {
      this.layoutFrame = undefined;
    }

    try {
      await host.beforeRender?.();
      if (!host.isCurrentRequest(requestId) || abortController.signal.aborted) {
        return;
      }
      await host.postMessage({
        type: "renderStarted",
        requestId,
        reason,
        snapshot: snapshotPayload,
      });
      host.info(
        `refresh=${reason} source="${snapshot.input.title}" id="${snapshot.input.sourceId}" mode=${snapshot.displayMode}`,
      );
      if (!host.isCurrentRequest(requestId) || abortController.signal.aborted) {
        return;
      }
      const { content, previewLayoutFrame } = await host.renderContent(
        snapshot,
        abortController.signal,
        this.layoutFrame?.frame,
      );
      if (!host.isCurrentRequest(requestId) || abortController.signal.aborted) {
        return;
      }
      host.markRendered(requestId, snapshot, content);
      await host.postMessage({
        type: "renderSucceeded",
        requestId,
        snapshot: snapshotPayload,
        content,
      });
      if (source && host.isCurrentRequest(requestId) && !abortController.signal.aborted) {
        this.layoutFrame = previewLayoutFrame
          ? { source, frame: previewLayoutFrame }
          : undefined;
      }
    } catch (error) {
      if (!host.isCurrentRequest(requestId)) {
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      host.error(message);
      await postRenderFailed(host, requestId, snapshotPayload, message);
    } finally {
      if (this.pending === pending) {
        this.pending = undefined;
      }
    }
  }

  isCurrentRequest(requestId: number): boolean {
    return requestId === this.requestId;
  }
}

function layoutFrameSource(snapshot: PreviewSnapshot): LayoutFrameSource {
  const { documentUri, documentVersion, input, sources, sourceKey } = snapshot;
  const { kind, sourceId } = input;
  const fence = kind === "markdown-fence" ? {
    range: input.sourceRange,
    siblings: sources.filter((peer) => peer.sourceId !== sourceId).map((peer) => ({
      sourceId: peer.sourceId,
      sourceHash: hashFenceSource(peer.source),
      range: peer.sourceRange,
    })),
  } : undefined;
  return {
    documentUri,
    documentVersion,
    kind,
    sourceId,
    sourceHash: fence ? hashFenceSource(input.source) : sourceKey.sourceHash,
    fitWidth: sourceKey.fitWidth,
    fence,
  };
}

function hashFenceSource(source: string): string {
  return createHash("sha256").update(source).digest("hex");
}

function continuesLayoutFrame(previous: LayoutFrameSource, next: LayoutFrameSource): boolean {
  const sourceChanged = previous.sourceHash !== next.sourceHash;
  if (previous.documentUri !== next.documentUri || previous.kind !== next.kind ||
    previous.sourceId !== next.sourceId || previous.fitWidth !== next.fitWidth ||
    previous.documentVersion > next.documentVersion ||
    (previous.documentVersion === next.documentVersion && sourceChanged)) {
    return false;
  }
  const before = previous.fence;
  const after = next.fence;
  if (!before || !after) {
    return true;
  }
  if (before.range.startLine !== after.range.startLine || before.siblings.length !== after.siblings.length) {
    return false;
  }

  const lineDelta = after.range.endLine - before.range.endLine;
  return before.siblings.every((sibling, index) => {
    const current = after.siblings[index]!;
    const shift = sibling.range.startLine > before.range.endLine ? lineDelta : 0;
    const sharesSource = sourceChanged &&
      (sibling.sourceHash === previous.sourceHash || sibling.sourceHash === next.sourceHash);
    return sibling.sourceId === current.sourceId && sibling.sourceHash === current.sourceHash &&
      sibling.range.startLine + shift === current.range.startLine &&
      sibling.range.endLine + shift === current.range.endLine && !sharesSource;
  });
}

async function postRenderFailed(
  host: PreviewRenderHost,
  requestId: number,
  snapshot: ReturnType<typeof snapshotMessagePayload>,
  message: string,
): Promise<void> {
  try {
    await host.postMessage({
      type: "renderFailed",
      requestId,
      snapshot,
      error: message,
    });
  } catch (error) {
    host.error(`failed to notify preview webview: ${errorMessage(error)}`);
  }
}
