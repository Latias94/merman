import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  useEffect,
  type CSSProperties,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { AlertCircle, Check, Download, X } from "lucide-react";
import { toast } from "sonner";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ExportPreview } from "@/src/components/ExportPreview";
import {
  createSvgExportBlob,
  downloadBlob,
  encodeRasterExport,
} from "@/src/lib/export";
import {
  planRasterExport,
  type RasterExportPlan,
  type RasterExportRequest,
  type RasterExportSource,
  type RasterSizing,
} from "@/src/lib/raster-export-plan";
import { inspectSvgForRasterExport } from "@/src/lib/svg-geometry";
import { exportTargetOwner } from "@/src/runtime/artifact-actions-browser";
import type {
  ArtifactEngine,
  FrozenExportTarget,
} from "@/src/runtime/artifact-actions";
import type { NavigableInlineSvg } from "@/src/runtime/render-artifact";
import type { RenderPublicationId } from "@/src/runtime/render-coordinator";

type ExportFormat = "svg" | "png" | "jpeg";
type RasterSizingMode = RasterSizing["mode"];
type BackgroundMode = "original" | "transparent" | "custom";
type ValidationField = "background" | "width" | "height";

interface ExportWorkbenchContextValue {
  openExport(
    engine: ArtifactEngine,
    publicationId: RenderPublicationId,
    restoreFocus?: HTMLElement | null,
  ): void;
}

const ExportWorkbenchContext = createContext<ExportWorkbenchContextValue | null>(
  null,
);

const SAFE_AREA_TOP =
  "var(--merman-safe-area-inset-top, env(safe-area-inset-top))";
const SAFE_AREA_RIGHT =
  "var(--merman-safe-area-inset-right, env(safe-area-inset-right))";
const SAFE_AREA_BOTTOM =
  "var(--merman-safe-area-inset-bottom, env(safe-area-inset-bottom))";
const SAFE_AREA_LEFT =
  "var(--merman-safe-area-inset-left, env(safe-area-inset-left))";
const EXPORT_HEADER_SAFE_AREA: CSSProperties = {
  paddingTop: `max(0.75rem, ${SAFE_AREA_TOP})`,
  paddingRight: `max(3rem, calc(${SAFE_AREA_RIGHT} + 3rem))`,
  paddingLeft: `max(1.25rem, ${SAFE_AREA_LEFT})`,
};
const EXPORT_CLOSE_SAFE_AREA: CSSProperties = {
  top: `max(0.75rem, ${SAFE_AREA_TOP})`,
  right: `max(0.75rem, ${SAFE_AREA_RIGHT})`,
};
const EXPORT_BODY_SAFE_AREA: CSSProperties = {
  paddingRight: SAFE_AREA_RIGHT,
  paddingLeft: SAFE_AREA_LEFT,
};
const EXPORT_FOOTER_SAFE_AREA: CSSProperties = {
  paddingRight: `max(1.25rem, ${SAFE_AREA_RIGHT})`,
  paddingBottom: `max(0.75rem, ${SAFE_AREA_BOTTOM})`,
  paddingLeft: `max(1.25rem, ${SAFE_AREA_LEFT})`,
};

export function ExportWorkbench({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const [target, setTarget] = useState<FrozenExportTarget | null>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);

  const openExport = useCallback(
    (
      engine: ArtifactEngine,
      publicationId: RenderPublicationId,
      restoreFocus?: HTMLElement | null,
    ) => {
      try {
        restoreFocusRef.current =
          restoreFocus ??
          (document.activeElement instanceof HTMLElement
            ? document.activeElement
            : null);
        setTarget(exportTargetOwner.freeze({ engine, publicationId }));
      } catch (error) {
        toast.error(error instanceof Error ? error.message : t("export.failed"));
      }
    },
    [t],
  );
  const close = useCallback(() => {
    setTarget(null);
    requestAnimationFrame(() => restoreFocusRef.current?.focus());
  }, []);
  const workbench = useMemo(() => ({ openExport }), [openExport]);

  return (
    <ExportWorkbenchContext.Provider value={workbench}>
      {children}
      {target && (
        <ExportDialog
          key={`${target.engine}:${target.publicationId}`}
          target={target}
          onClose={close}
        />
      )}
    </ExportWorkbenchContext.Provider>
  );
}

export function useExportWorkbench(): ExportWorkbenchContextValue {
  const value = useContext(ExportWorkbenchContext);
  if (!value) {
    throw new Error("Export controls must be rendered inside ExportWorkbench.");
  }
  return value;
}

interface PreparedSvgExport {
  readonly kind: "svg";
  readonly key: string;
}

interface PreparedRasterExport {
  readonly kind: "raster";
  readonly artifact: NavigableInlineSvg;
  readonly key: string;
  readonly plan: Readonly<RasterExportPlan>;
}

type PreparedExport = PreparedSvgExport | PreparedRasterExport;

interface PreparationFailure {
  readonly error: string;
  readonly field: ValidationField | null;
}

interface RasterInput {
  readonly artifact: NavigableInlineSvg;
  readonly source: Readonly<RasterExportSource>;
}

interface EncodedExport {
  readonly blob: Blob;
  readonly key: string;
}

function ExportDialog({
  target,
  onClose,
}: {
  target: FrozenExportTarget;
  onClose(): void;
}) {
  const { t } = useTranslation();
  const intrinsic = useMemo(
    () => inspectSvgForRasterExport(target.svgArtifact),
    [target],
  );
  const [format, setFormat] = useState<ExportFormat>("svg");
  const [pngBackground, setPngBackground] =
    useState<BackgroundMode>("original");
  const [jpegBackground, setJpegBackground] =
    useState<Exclude<BackgroundMode, "transparent">>("custom");
  const [pngColor, setPngColor] = useState("#ffffff");
  const [jpegColor, setJpegColor] = useState("#ffffff");
  const [sizingMode, setSizingMode] =
    useState<RasterSizingMode>("scale");
  const [scale, setScale] = useState(2);
  const [widthDraft, setWidthDraft] = useState(() =>
    String(Math.ceil(intrinsic.width * 2)),
  );
  const [heightDraft, setHeightDraft] = useState(() =>
    String(Math.ceil(intrinsic.height * 2)),
  );
  const [quality, setQuality] = useState(90);
  const [encoded, setEncoded] = useState<EncodedExport | null>(null);
  const [busy, setBusy] = useState(false);
  const [downloaded, setDownloaded] = useState(false);
  const [encodeFailure, setEncodeFailure] = useState<string | null>(null);
  const encodeGenerationRef = useRef(0);
  const inFlightEncodeRef = useRef<ReturnType<typeof encodeRasterExport> | null>(
    null,
  );
  const usesRaster = format !== "svg";
  const rasterInputResult = useMemo<RasterInput | PreparationFailure | null>(() => {
    if (!usesRaster) return null;
    try {
      const artifact = exportTargetOwner.rasterArtifact(target);
      return Object.freeze({
        artifact,
        source:
          artifact === target.svgArtifact
            ? intrinsic
            : inspectSvgForRasterExport(artifact),
      });
    } catch (error) {
      return {
        error: error instanceof Error ? error.message : t("export.failed"),
        field: null,
      };
    }
  }, [intrinsic, t, target, usesRaster]);
  const rasterInput =
    rasterInputResult && !("error" in rasterInputResult)
      ? rasterInputResult
      : null;
  const rasterInputFailure =
    rasterInputResult && "error" in rasterInputResult
      ? rasterInputResult
      : null;

  const initializedRasterDimensionsRef = useRef(false);
  useEffect(() => {
    if (!rasterInput || initializedRasterDimensionsRef.current) return;
    initializedRasterDimensionsRef.current = true;
    setWidthDraft(String(Math.ceil(rasterInput.source.width * 2)));
    setHeightDraft(String(Math.ceil(rasterInput.source.height * 2)));
  }, [rasterInput]);

  const preparation = useMemo<PreparedExport | PreparationFailure>(() => {
    if (format === "svg") {
      return {
        kind: "svg",
        key: `${target.engine}:${target.publicationId}:svg`,
      };
    }
    if (rasterInputFailure) return rasterInputFailure;
    try {
      if (!rasterInput) throw new Error("Raster export source is unavailable.");
      const request = buildRasterRequest({
        format,
        heightDraft,
        jpegBackground,
        jpegColor,
        pngBackground,
        pngColor,
        quality,
        scale,
        sizingMode,
        widthDraft,
      });
      return {
        kind: "raster",
        artifact: rasterInput.artifact,
        key: `${target.engine}:${target.publicationId}:${JSON.stringify(request)}`,
        plan: planRasterExport(rasterInput.source, request),
      };
    } catch (error) {
      return {
        error:
          error instanceof DraftValidationError
            ? t(
                `export.${error.messageKey}`,
                error.messageKey === "invalidDimension"
                  ? { axis: t(`export.${error.field}`) }
                  : undefined,
              )
            : error instanceof Error
              ? error.message
              : t("export.failed"),
        field: error instanceof DraftValidationError ? error.field : null,
      };
    }
  }, [
    format,
    heightDraft,
    jpegBackground,
    jpegColor,
    pngBackground,
    pngColor,
    quality,
    rasterInput,
    rasterInputFailure,
    scale,
    sizingMode,
    t,
    target,
    widthDraft,
  ]);
  const preparationFailure: PreparationFailure | null =
    "error" in preparation ? preparation : null;
  const prepared: PreparedExport | null =
    "error" in preparation ? null : preparation;

  useEffect(() => {
    const generation = ++encodeGenerationRef.current;
    if (!prepared) {
      setBusy(false);
      setDownloaded(false);
      setEncodeFailure(null);
      return;
    }
    setDownloaded(false);
    setEncodeFailure(null);
    if (prepared.kind === "svg") {
      setBusy(false);
      setEncoded({
        blob: createSvgExportBlob(target.svgArtifact),
        key: prepared.key,
      });
      return;
    }

    let cancelled = false;
    setBusy(true);
    const timeout = window.setTimeout(() => {
      void (async () => {
        try {
          if (inFlightEncodeRef.current) {
            try {
              await inFlightEncodeRef.current;
            } catch {
              // The current recipe owns its own error state.
            }
          }
          if (cancelled || encodeGenerationRef.current !== generation) return;

          const operation = encodeRasterExport(prepared.artifact, prepared.plan);
          inFlightEncodeRef.current = operation;
          const blob = await operation;
          if (cancelled || encodeGenerationRef.current !== generation) return;
          setEncoded({
            blob,
            key: prepared.key,
          });
        } catch (error: unknown) {
          if (cancelled || encodeGenerationRef.current !== generation) return;
          setEncodeFailure(
            error instanceof Error ? error.message : t("export.failed"),
          );
        } finally {
          inFlightEncodeRef.current = null;
          if (!cancelled && encodeGenerationRef.current === generation) {
            setBusy(false);
          }
        }
      })();
    }, 120);
    return () => {
      cancelled = true;
      window.clearTimeout(timeout);
    };
  }, [prepared, t, target.svgArtifact]);

  const ready = Boolean(
    prepared && encoded?.key === prepared.key && !busy && !encodeFailure,
  );
  const activePlan = prepared?.kind === "raster" ? prepared.plan : null;
  const selectedBackground =
    format === "jpeg" ? jpegBackground : pngBackground;
  const customColor = format === "jpeg" ? jpegColor : pngColor;
  const setCustomColor = format === "jpeg" ? setJpegColor : setPngColor;
  const validationError = preparationFailure?.error ?? encodeFailure;
  const status = busy
    ? t("export.previewing")
    : downloaded
      ? t("export.downloaded")
      : ready
        ? t("export.ready")
        : "";

  const download = () => {
    if (!ready || !encoded || !prepared) return;
    const extension = prepared.kind === "svg" ? "svg" : prepared.plan.extension;
    downloadBlob(encoded.blob, `${target.filename}.${extension}`);
    setDownloaded(true);
  };

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        safeArea={false}
        showCloseButton={false}
        data-testid="export-dialog"
        data-export-engine={target.engine}
        data-export-publication={target.publicationId}
        className="grid h-[min(760px,calc(100dvh-2rem))] w-[min(980px,calc(100vw-2rem))] max-w-none grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0 max-sm:h-[100dvh] max-sm:w-screen max-sm:rounded-none max-sm:border-0"
      >
        <DialogHeader
          className="relative border-b px-5 py-3 pr-12 text-left"
          style={EXPORT_HEADER_SAFE_AREA}
        >
          <DialogTitle>{t("export.workbenchTitle")}</DialogTitle>
          <DialogDescription>
            {t("export.snapshot", {
              engine: target.engine === "merman" ? "Merman" : "Mermaid JS",
              publication: target.publicationId,
            })}
          </DialogDescription>
          <DialogClose asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="absolute right-3 top-3"
              style={EXPORT_CLOSE_SAFE_AREA}
              aria-label={t("export.close")}
            >
              <X className="size-4" />
            </Button>
          </DialogClose>
        </DialogHeader>

        <div
          className="flex min-h-0 flex-col overflow-auto lg:grid lg:grid-cols-[19rem_minmax(0,1fr)] lg:overflow-hidden"
          style={EXPORT_BODY_SAFE_AREA}
        >
          <div className="space-y-5 border-b p-4 lg:overflow-y-auto lg:border-b-0 lg:border-r">
            <div
              role="group"
              aria-label={t("export.format")}
              className="grid h-9 w-full grid-cols-3 rounded-lg bg-muted p-[3px]"
            >
              {(["svg", "png", "jpeg"] as const).map((value) => (
                <button
                  key={value}
                  type="button"
                  aria-pressed={format === value}
                  onClick={() => setFormat(value)}
                  className={cn(
                    "rounded-md px-2 text-xs font-medium text-muted-foreground transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    format === value &&
                      "bg-background text-foreground shadow-sm",
                  )}
                >
                  {value === "jpeg" ? "JPEG" : value.toUpperCase()}
                </button>
              ))}
            </div>

            {format === "svg" ? (
              <DimensionSummary
                source={intrinsic}
                plan={null}
                t={t}
              />
            ) : (
              <>
                <fieldset className="space-y-2">
                  <legend className="text-xs font-medium">
                    {t("export.background")}
                  </legend>
                  <div className="grid grid-cols-3 gap-1 rounded-md bg-muted p-1">
                    {(["original", "transparent", "custom"] as const).map(
                      (mode) => {
                        const disabled =
                          format === "jpeg" &&
                            (mode === "transparent" ||
                              (mode === "original" &&
                                !rasterInput?.source.originalBackground?.opaque));
                        return (
                          <button
                            key={mode}
                            type="button"
                            disabled={disabled}
                            aria-pressed={selectedBackground === mode}
                            onClick={() => {
                              if (format === "jpeg" && mode !== "transparent") {
                                setJpegBackground(mode);
                              } else if (format === "png") {
                                setPngBackground(mode);
                              }
                            }}
                            className={cn(
                              "h-8 rounded text-xs font-medium text-muted-foreground disabled:cursor-not-allowed disabled:opacity-40",
                              selectedBackground === mode &&
                                "bg-background text-foreground shadow-sm",
                            )}
                          >
                            {t(`export.background${capitalize(mode)}`)}
                          </button>
                        );
                      },
                    )}
                  </div>
                  {selectedBackground === "custom" && (
                    <div className="flex items-center gap-2">
                      <input
                        type="color"
                        value={isHexColor(customColor) ? customColor : "#ffffff"}
                        onChange={(event) => setCustomColor(event.target.value)}
                        aria-label={t("export.backgroundColor")}
                        className="size-9 shrink-0 cursor-pointer rounded-md border bg-transparent p-1"
                      />
                      <Input
                        value={customColor}
                        onChange={(event) => setCustomColor(event.target.value)}
                        aria-label={t("export.backgroundColor")}
                        aria-invalid={preparationFailure?.field === "background"}
                        aria-describedby={
                          preparationFailure?.field === "background"
                            ? "export-error"
                            : undefined
                        }
                        className="font-mono"
                      />
                    </div>
                  )}
                </fieldset>

                <fieldset className="space-y-2">
                  <legend className="text-xs font-medium">
                    {t("export.sizing")}
                  </legend>
                  <div className="grid grid-cols-4 gap-1 rounded-md bg-muted p-1">
                    {(["scale", "width", "height", "fit"] as const).map(
                      (mode) => (
                        <button
                          key={mode}
                          type="button"
                          aria-pressed={sizingMode === mode}
                          onClick={() => setSizingMode(mode)}
                          className={cn(
                            "h-8 rounded text-xs font-medium text-muted-foreground",
                            sizingMode === mode &&
                              "bg-background text-foreground shadow-sm",
                          )}
                        >
                          {t(`export.sizing${capitalize(mode)}`)}
                        </button>
                      ),
                    )}
                  </div>
                  {sizingMode === "scale" ? (
                    <div className="grid grid-cols-4 gap-1">
                      {[1, 2, 3, 4].map((value) => (
                        <button
                          key={value}
                          type="button"
                          aria-pressed={scale === value}
                          onClick={() => setScale(value)}
                          className={cn(
                            "h-9 rounded-md border text-xs font-medium text-muted-foreground",
                            scale === value && "border-ring bg-muted text-foreground",
                          )}
                        >
                          {value}×
                        </button>
                      ))}
                    </div>
                  ) : (
                    <div className="grid grid-cols-2 gap-2">
                      {(sizingMode === "width" || sizingMode === "fit") && (
                        <DimensionInput
                          axis="width"
                          value={widthDraft}
                          onChange={setWidthDraft}
                          invalid={preparationFailure?.field === "width"}
                          t={t}
                        />
                      )}
                      {(sizingMode === "height" || sizingMode === "fit") && (
                        <DimensionInput
                          axis="height"
                          value={heightDraft}
                          onChange={setHeightDraft}
                          invalid={preparationFailure?.field === "height"}
                          t={t}
                        />
                      )}
                    </div>
                  )}
                </fieldset>

                {format === "jpeg" && (
                  <div className="space-y-2">
                    <div className="flex items-center justify-between text-xs font-medium">
                      <label htmlFor="export-quality">
                        {t("export.quality")}
                      </label>
                      <span className="tabular-nums">{quality}%</span>
                    </div>
                    <input
                      id="export-quality"
                      type="range"
                      min={1}
                      max={100}
                      step={1}
                      value={quality}
                      onChange={(event) => setQuality(Number(event.target.value))}
                      aria-valuetext={`${quality}%`}
                      className="w-full accent-primary"
                    />
                    <p className="text-xs text-muted-foreground">
                      {t("export.jpegLossy")}
                    </p>
                  </div>
                )}

                <DimensionSummary
                  source={intrinsic}
                  plan={activePlan}
                  t={t}
                />
              </>
            )}
          </div>

          <div className="flex min-h-[260px] flex-col lg:min-h-0">
            <ExportPreview
              blob={encoded?.blob ?? null}
              busy={busy}
              label={t("export.previewAlt")}
            />
          </div>
        </div>

        <DialogFooter
          className="min-h-16 border-t bg-background px-5 py-3 sm:items-center"
          style={EXPORT_FOOTER_SAFE_AREA}
        >
          <div className="min-w-0 flex-1 text-left">
            {validationError && (
              <p
                id="export-error"
                role="alert"
                className="flex items-start gap-2 text-xs text-destructive"
              >
                <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
                <span>{validationError}</span>
              </p>
            )}
            <p
              role="status"
              aria-live="polite"
              className="flex min-h-5 items-center gap-1.5 text-xs text-muted-foreground"
            >
              {downloaded && <Check className="size-3.5 text-green-600" />}
              {status}
            </p>
          </div>
          <Button type="button" onClick={download} disabled={!ready}>
            <Download className="size-4" />
            {t("export.download")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function DimensionInput({
  axis,
  value,
  onChange,
  invalid,
  t,
}: {
  axis: "width" | "height";
  value: string;
  onChange(value: string): void;
  invalid: boolean;
  t: (key: string) => string;
}) {
  return (
    <label className="space-y-1 text-xs text-muted-foreground">
      <span>{t(`export.${axis}`)}</span>
      <div className="relative">
        <Input
          inputMode="numeric"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          aria-invalid={invalid}
          aria-describedby={invalid ? "export-error" : undefined}
          className="pr-8 tabular-nums"
        />
        <span className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-[11px]">
          px
        </span>
      </div>
    </label>
  );
}

function DimensionSummary({
  source,
  plan,
  t,
}: {
  source: Readonly<RasterExportSource>;
  plan: Readonly<RasterExportPlan> | null;
  t: (key: string, options?: Record<string, unknown>) => string;
}) {
  const sourceWidth = plan?.sourceWidth ?? source.width;
  const sourceHeight = plan?.sourceHeight ?? source.height;
  return (
    <dl className="grid grid-cols-2 gap-x-3 gap-y-1 rounded-md border bg-muted/20 p-3 text-xs">
      <dt className="text-muted-foreground">{t("export.intrinsic")}</dt>
      <dd className="text-right tabular-nums">
        {Math.ceil(sourceWidth)} × {Math.ceil(sourceHeight)}
      </dd>
      <dt className="text-muted-foreground">{t("export.output")}</dt>
      <dd data-testid="export-output-dimensions" className="text-right tabular-nums">
        {plan
          ? `${plan.outputWidth} × ${plan.outputHeight}`
          : `${Math.ceil(sourceWidth)} × ${Math.ceil(sourceHeight)}`}
      </dd>
      {plan?.downscaled && (
        <>
          <dt className="col-span-2 mt-1 text-amber-700 dark:text-amber-300">
            {t("export.downscaled", {
              width: plan.requestedWidth,
              height: plan.requestedHeight,
            })}
          </dt>
        </>
      )}
    </dl>
  );
}

function buildRasterRequest({
  format,
  heightDraft,
  jpegBackground,
  jpegColor,
  pngBackground,
  pngColor,
  quality,
  scale,
  sizingMode,
  widthDraft,
}: {
  format: Exclude<ExportFormat, "svg">;
  heightDraft: string;
  jpegBackground: Exclude<BackgroundMode, "transparent">;
  jpegColor: string;
  pngBackground: BackgroundMode;
  pngColor: string;
  quality: number;
  scale: number;
  sizingMode: RasterSizingMode;
  widthDraft: string;
}): RasterExportRequest {
  const sizing = buildSizing(sizingMode, scale, widthDraft, heightDraft);
  if (format === "png") {
    return {
      format,
      background:
        pngBackground === "custom"
          ? { mode: "custom", color: requireHexColor(pngColor) }
          : { mode: pngBackground },
      sizing,
    };
  }
  return {
    format,
    background:
      jpegBackground === "custom"
        ? { mode: "custom", color: requireHexColor(jpegColor) }
        : { mode: "original" },
    quality,
    sizing,
  };
}

function buildSizing(
  mode: RasterSizingMode,
  scale: number,
  widthDraft: string,
  heightDraft: string,
): RasterSizing {
  switch (mode) {
    case "scale":
      return { mode, scale };
    case "width":
      return { mode, width: parseDimension(widthDraft, "width") };
    case "height":
      return { mode, height: parseDimension(heightDraft, "height") };
    case "fit":
      return {
        mode,
        width: parseDimension(widthDraft, "width"),
        height: parseDimension(heightDraft, "height"),
      };
  }
}

function parseDimension(value: string, field: "width" | "height"): number {
  if (!/^\d+$/u.test(value)) {
    throw invalidDimension(field);
  }
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw invalidDimension(field);
  }
  return parsed;
}

function invalidDimension(field: "width" | "height"): DraftValidationError {
  return new DraftValidationError(field, "invalidDimension");
}

function requireHexColor(value: string): string {
  if (!isHexColor(value)) {
    throw new DraftValidationError(
      "background",
      "invalidBackground",
    );
  }
  return value.toLowerCase();
}

function isHexColor(value: string): boolean {
  return /^#[0-9a-f]{6}$/iu.test(value);
}

function capitalize(value: string): string {
  return `${value.charAt(0).toUpperCase()}${value.slice(1)}`;
}

class DraftValidationError extends Error {
  readonly field: ValidationField;
  readonly messageKey: "invalidBackground" | "invalidDimension";

  constructor(
    field: ValidationField,
    messageKey: DraftValidationError["messageKey"],
  ) {
    super(messageKey);
    this.name = "DraftValidationError";
    this.field = field;
    this.messageKey = messageKey;
  }
}
