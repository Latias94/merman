import type { MermanAsciiBatchResult } from "../runtime/render-coordinator.ts";

interface CurrentAsciiPublication {
  readonly ascii: MermanAsciiBatchResult | null;
}

export function isAsciiExportAvailable(
  asciiSupported: boolean,
  currentBatch: CurrentAsciiPublication | null,
): boolean {
  return (
    asciiSupported &&
    currentBatch !== null &&
    (currentBatch.ascii === null || currentBatch.ascii.status === "success")
  );
}
