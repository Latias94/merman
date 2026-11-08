import { errorMessage } from "./error-message.js";
import { samePreviewRenderKey, type PreviewSnapshot } from "./preview-model.js";
import {
  snapshotMessagePayload,
  type PreviewToWebviewMessage,
} from "./preview-messages.js";
import type { PreviewUpdateReason } from "./preview-policy.js";

export interface PreviewRenderHost {
  beforeRender?(): Promise<void>;
  renderContent(snapshot: PreviewSnapshot, signal: AbortSignal): Promise<string>;
  postMessage(message: PreviewToWebviewMessage): Promise<void>;
  info(message: string): void;
  error(message: string): void;
  isCurrentRequest(requestId: number): boolean;
  markRendered(requestId: number, snapshot: PreviewSnapshot, content: string): void;
}

export class PreviewRenderQueue {
  private requestId = 0;
  private pending: { snapshot: PreviewSnapshot; abortController: AbortController } | undefined;

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
      const content = await host.renderContent(snapshot, abortController.signal);
      if (!host.isCurrentRequest(requestId)) {
        return;
      }
      host.markRendered(requestId, snapshot, content);
      await host.postMessage({
        type: "renderSucceeded",
        requestId,
        snapshot: snapshotPayload,
        content,
      });
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
