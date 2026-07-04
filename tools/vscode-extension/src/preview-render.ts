import type { PreviewSnapshot } from "./preview-model.js";
import {
  snapshotMessagePayload,
  type PreviewToWebviewMessage,
} from "./preview-messages.js";
import type { PreviewUpdateReason } from "./preview-policy.js";

export interface PreviewRenderHost {
  renderContent(snapshot: PreviewSnapshot, signal: AbortSignal): Promise<string>;
  postMessage(message: PreviewToWebviewMessage): Promise<void>;
  info(message: string): void;
  error(message: string): void;
  isCurrentRequest(requestId: number): boolean;
  markRendered(requestId: number, snapshot: PreviewSnapshot, content: string): void;
}

export class PreviewRenderQueue {
  private requestId = 0;
  private abortController: AbortController | undefined;

  cancelPending(): void {
    this.requestId += 1;
    this.abortController?.abort();
    this.abortController = undefined;
  }

  async render(
    snapshot: PreviewSnapshot,
    reason: PreviewUpdateReason,
    host: PreviewRenderHost,
  ): Promise<void> {
    const requestId = ++this.requestId;
    this.abortController?.abort();
    const abortController = new AbortController();
    this.abortController = abortController;
    const snapshotPayload = snapshotMessagePayload(snapshot);

    try {
      await host.postMessage({
        type: "renderStarted",
        requestId,
        reason,
        snapshot: snapshotPayload,
      });
      host.info(
        `refresh=${reason} source="${snapshot.input.title}" id="${snapshot.input.sourceId}" mode=${snapshot.displayMode}`,
      );
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
      if (this.abortController === abortController) {
        this.abortController = undefined;
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

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
