export const SVG_PRESENTATION_MODES = ["infinite", "viewbox"] as const;

export type SvgPresentationMode = (typeof SVG_PRESENTATION_MODES)[number];

export const DEFAULT_SVG_PRESENTATION_MODE: SvgPresentationMode = "infinite";

export function isSvgPresentationMode(
  value: unknown,
): value is SvgPresentationMode {
  return (
    typeof value === "string" &&
    SVG_PRESENTATION_MODES.some((mode) => mode === value)
  );
}
