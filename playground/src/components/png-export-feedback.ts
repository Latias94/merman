import type { TFunction } from "i18next";

import { PngExportError } from "@/src/lib/export";

export function pngExportErrorMessage(
  error: unknown,
  t: TFunction
): string {
  if (error instanceof PngExportError) {
    return t("export.pngFailedAtDimensions", {
      width: error.plan.outputWidth,
      height: error.plan.outputHeight,
    });
  }
  return error instanceof Error ? error.message : t("export.failed");
}
