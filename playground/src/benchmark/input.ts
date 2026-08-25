import {
  validateScreenAvailableWidth,
  type CompareRenderPayload,
} from "../runtime/realm/channel-protocol.ts";
import { CANONICAL_RENDER_VIEWPORT } from "../runtime/render-viewport.ts";

export const CANONICAL_BENCHMARK_SCREEN_AVAILABLE_WIDTH =
  validateScreenAvailableWidth(CANONICAL_RENDER_VIEWPORT.width);

export function createCanonicalBenchmarkPayload(
  input: Omit<CompareRenderPayload, "screenAvailableWidth" | "viewport">,
): Readonly<CompareRenderPayload> {
  return Object.freeze({
    ...input,
    screenAvailableWidth: CANONICAL_BENCHMARK_SCREEN_AVAILABLE_WIDTH,
    viewport: CANONICAL_RENDER_VIEWPORT,
  });
}
