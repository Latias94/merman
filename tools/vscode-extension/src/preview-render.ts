import type { PreviewSnapshot } from "./preview-model.js";
import {
  snapshotMessagePayload,
  type PreviewToWebviewMessage,
} from "./preview-messages.js";
import type { PreviewUpdateReason } from "./preview-policy.js";

export interface PreviewRenderHost {
  renderSvg(source: string): Promise<string>;
  postMessage(message: PreviewToWebviewMessage): Promise<void>;
  info(message: string): void;
  error(message: string): void;
  isCurrentRequest(requestId: number): boolean;
  markRendered(requestId: number, snapshot: PreviewSnapshot, svg: string): void;
}

export class PreviewRenderQueue {
  private requestId = 0;

  cancelPending(): void {
    this.requestId += 1;
  }

  async render(
    snapshot: PreviewSnapshot,
    reason: PreviewUpdateReason,
    host: PreviewRenderHost,
  ): Promise<void> {
    const requestId = ++this.requestId;
    const snapshotPayload = snapshotMessagePayload(snapshot);
    await host.postMessage({
      type: "renderStarted",
      requestId,
      reason,
      snapshot: snapshotPayload,
    });

    try {
      host.info(
        `refresh=${reason} source="${snapshot.input.title}" id="${snapshot.input.sourceId}"`,
      );
      const svg = await host.renderSvg(snapshot.input.source);
      if (!host.isCurrentRequest(requestId)) {
        return;
      }
      host.markRendered(requestId, snapshot, svg);
      await host.postMessage({
        type: "renderSucceeded",
        requestId,
        snapshot: snapshotPayload,
        svg,
      });
    } catch (error) {
      if (!host.isCurrentRequest(requestId)) {
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      host.error(message);
      await host.postMessage({
        type: "renderFailed",
        requestId,
        snapshot: snapshotPayload,
        error: message,
      });
    }
  }

  isCurrentRequest(requestId: number): boolean {
    return requestId === this.requestId;
  }
}
