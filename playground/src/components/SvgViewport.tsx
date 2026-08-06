import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
  type PointerEvent,
  type ReactNode,
} from "react";

import {
  prepareSvgForResponsivePreview,
  type SvgDimensions,
} from "@/src/lib/svg-geometry";
import type { SafeInlineSvg } from "@/src/runtime/render-artifact";

interface Point {
  readonly x: number;
  readonly y: number;
}

const SVG_VIEWPORT_CONTROLLER = Symbol("svg-viewport-controller");

export interface SvgViewportController {
  readonly [SVG_VIEWPORT_CONTROLLER]: true;
  fitToView(): void;
  reset(): void;
  zoomIn(): void;
  zoomOut(): void;
}

interface ViewportCommands {
  fitToView(): void;
  reset(): void;
  zoomIn(): void;
  zoomOut(): void;
}

class SvgViewportControllerImpl implements SvgViewportController {
  readonly [SVG_VIEWPORT_CONTROLLER] = true;
  readonly fitToView = () => this.owner?.commands.fitToView();
  readonly reset = () => this.owner?.commands.reset();
  readonly zoomIn = () => this.owner?.commands.zoomIn();
  readonly zoomOut = () => this.owner?.commands.zoomOut();
  readonly getSnapshot = () => this.zoom;
  readonly subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  private readonly listeners = new Set<() => void>();
  private owner: {
    readonly commands: ViewportCommands;
    readonly token: symbol;
  } | null = null;
  private zoom = 1;

  connect(commands: ViewportCommands): () => void {
    const token = Symbol("svg-viewport-owner");
    this.owner = { commands, token };
    return () => {
      if (this.owner?.token === token) this.owner = null;
    };
  }

  publishZoom(zoom: number): void {
    if (Object.is(this.zoom, zoom)) return;
    this.zoom = zoom;
    for (const listener of this.listeners) listener();
  }
}

export function useSvgViewportController(): SvgViewportController {
  const controllerRef = useRef<SvgViewportController | null>(null);
  if (!controllerRef.current) {
    controllerRef.current = new SvgViewportControllerImpl();
  }
  return controllerRef.current;
}

export function useSvgViewportZoom(
  controller: SvgViewportController
): number {
  const internal = internalController(controller);
  return useSyncExternalStore(
    internal.subscribe,
    internal.getSnapshot,
    internal.getSnapshot
  );
}

type PreparedSvgPreview = NonNullable<
  ReturnType<typeof prepareSvgForResponsivePreview>
>;

type Gesture =
  | { readonly kind: "idle" }
  | {
      readonly kind: "pan";
      readonly pointerId: number;
      readonly startPointer: Point;
      readonly startPosition: Point;
    }
  | {
      readonly kind: "pinch";
      readonly pointerIds: readonly [number, number];
      readonly startDistance: number;
      readonly startMidpoint: Point;
      readonly startPosition: Point;
      readonly startZoom: number;
      readonly containerCenter: Point;
    };

interface ViewportState {
  readonly pointers: Map<number, Point>;
  autoFit: boolean;
  fitZoom: number;
  fittedPreview: PreparedSvgPreview | null;
  gesture: Gesture;
  position: Point;
  scaleBaseZoom: number;
  zoom: number;
}

interface AppliedTransform {
  readonly container: HTMLDivElement | null;
  readonly contentScale: number;
  readonly positionLayer: HTMLDivElement | null;
  readonly positionX: number;
  readonly positionY: number;
  readonly scaleLayer: HTMLDivElement | null;
  readonly zoom: number;
}

interface MountedPresentation {
  readonly key: number | null;
  onReady?: (at: number) => void;
  readonly preview: PreparedSvgPreview;
  readonly root: ShadowRoot;
}

interface RequestedPresentation {
  readonly key: number | null;
  readonly onReady?: (at: number) => void;
  readonly preview: PreparedSvgPreview | null;
}

interface SvgViewportProps {
  artifact: SafeInlineSvg | null;
  presentationKey: number | null;
  controller: SvgViewportController;
  empty?: ReactNode;
  onPresentationReady?: (at: number) => void;
}

export function SvgViewport({
  artifact,
  presentationKey,
  controller,
  empty,
  onPresentationReady,
}: SvgViewportProps) {
  const preview = useMemo(() => {
    if (!artifact) return null;
    try {
      return prepareSvgForResponsivePreview(artifact);
    } catch {
      return null;
    }
  }, [artifact]);
  const containerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const positionLayerRef = useRef<HTMLDivElement>(null);
  const scaleLayerRef = useRef<HTMLDivElement>(null);
  const shadowHostRef = useRef<HTMLDivElement>(null);
  const transformFrameRef = useRef(0);
  const fitFrameRef = useRef(0);
  const readyFrameRef = useRef(0);
  const previewRef = useRef(preview);
  const lastAppliedRef = useRef<AppliedTransform | null>(null);
  const mountedPresentationRef = useRef<MountedPresentation | null>(null);
  const requestedPresentationRef = useRef<RequestedPresentation>({
    key: presentationKey,
    onReady: onPresentationReady,
    preview,
  });
  const presentedRef = useRef<{
    readonly key: number | null;
    readonly preview: PreparedSvgPreview;
  } | null>(null);
  const stateRef = useRef<ViewportState>({
    autoFit: true,
    fitZoom: 1,
    fittedPreview: null,
    gesture: { kind: "idle" },
    pointers: new Map(),
    position: { x: 0, y: 0 },
    scaleBaseZoom: 1,
    zoom: 1,
  });
  previewRef.current = preview;

  const applyTransform = useCallback(() => {
    transformFrameRef.current = 0;
    const state = stateRef.current;
    const positionLayer = positionLayerRef.current;
    const scaleLayer = scaleLayerRef.current;
    const container = containerRef.current;
    const hasCurrentFit = state.fittedPreview === previewRef.current;
    const contentScale = hasCurrentFit
      ? state.zoom / state.scaleBaseZoom
      : 1;
    const positionX = Math.round(state.position.x);
    const positionY = Math.round(state.position.y);
    const previous = lastAppliedRef.current;

    if (
      positionLayer &&
      (previous?.positionLayer !== positionLayer ||
        previous.positionX !== positionX ||
        previous.positionY !== positionY)
    ) {
      positionLayer.style.transform = `translate(${positionX}px, ${positionY}px)`;
    }
    if (
      scaleLayer &&
      (previous?.scaleLayer !== scaleLayer ||
        !Object.is(previous.contentScale, contentScale))
    ) {
      scaleLayer.style.transform = `translate(-50%, -50%) scale(${contentScale})`;
    }
    if (
      container &&
      (previous?.container !== container || !Object.is(previous.zoom, state.zoom))
    ) {
      container.dataset.zoom = String(state.zoom);
    }
    if (!previous || !Object.is(previous.zoom, state.zoom)) {
      internalController(controller).publishZoom(state.zoom);
    }
    lastAppliedRef.current = {
      container,
      contentScale,
      positionLayer,
      positionX,
      positionY,
      scaleLayer,
      zoom: state.zoom,
    };
  }, [controller]);

  const scheduleTransform = useCallback(() => {
    if (transformFrameRef.current) return;
    transformFrameRef.current = requestAnimationFrame(applyTransform);
  }, [applyTransform]);

  const schedulePresentationReady = useCallback(() => {
    cancelScheduledFrame(readyFrameRef);
    const scheduled = mountedPresentationRef.current;
    if (!scheduled) return;
    readyFrameRef.current = requestAnimationFrame(() => {
      readyFrameRef.current = 0;
      if (
        mountedPresentationRef.current !== scheduled ||
        shadowHostRef.current?.shadowRoot !== scheduled.root
      ) {
        return;
      }
      const identity = presentedRef.current;
      if (
        identity?.preview === scheduled.preview &&
        identity.key === scheduled.key
      ) {
        return;
      }

      const renderedSvg = scheduled.root.querySelector("svg");
      if (!renderedSvg) return;
      const bounds = renderedSvg.getBoundingClientRect();
      if (
        !Number.isFinite(bounds.width) ||
        !Number.isFinite(bounds.height) ||
        bounds.width <= 0 ||
        bounds.height <= 0
      ) {
        return;
      }

      presentedRef.current = {
        key: scheduled.key,
        preview: scheduled.preview,
      };
      scheduled.onReady?.(performance.now());
    });
  }, []);

  const setDragging = useCallback((dragging: boolean) => {
    const container = containerRef.current;
    if (container) {
      container.style.cursor = dragging ? "grabbing" : "";
      container.dataset.dragging = String(dragging);
    }
  }, []);

  const cancelGesture = useCallback((releaseCapture = false) => {
    const state = stateRef.current;
    const container = containerRef.current;
    const pointerIds = [...state.pointers.keys()];
    state.pointers.clear();
    state.gesture = { kind: "idle" };
    setDragging(false);

    if (!releaseCapture || !container) return;
    for (const pointerId of pointerIds) {
      releaseCapturedPointer(container, pointerId);
    }
  }, [setDragging]);

  const fitToView = useCallback((): boolean => {
    const container = containerRef.current;
    const content = contentRef.current;
    const shadowHost = shadowHostRef.current;
    const currentPreview = previewRef.current;
    if (
      !container ||
      !content ||
      !shadowHost ||
      !currentPreview ||
      container.clientWidth <= 0 ||
      container.clientHeight <= 0
    ) {
      return false;
    }

    const availableWidth = Math.max(container.clientWidth - 48, 1);
    const availableHeight = Math.max(container.clientHeight - 48, 1);
    const intrinsicSize = currentPreview.dimensions;
    let nextZoom: number;
    let scaleBaseZoom: number;

    if (intrinsicSize) {
      const insets = measureElementInsets(content);
      const availableSvgWidth = Math.max(availableWidth - insets.width, 1);
      const availableSvgHeight = Math.max(availableHeight - insets.height, 1);
      nextZoom = Math.min(
        1,
        availableSvgWidth / intrinsicSize.width,
        availableSvgHeight / intrinsicSize.height
      );
      if (!isPositiveFinite(nextZoom)) return false;
      shadowHost.style.width = `${Math.max(
        1,
        intrinsicSize.width * nextZoom
      )}px`;
      shadowHost.style.height = `${Math.max(
        1,
        intrinsicSize.height * nextZoom
      )}px`;
      scaleBaseZoom = nextZoom;
    } else {
      shadowHost.style.removeProperty("width");
      shadowHost.style.removeProperty("height");
      const contentSize = measureRenderedContent(content);
      if (!contentSize) return false;
      nextZoom = Math.min(
        1,
        availableWidth / contentSize.width,
        availableHeight / contentSize.height
      );
      if (!isPositiveFinite(nextZoom)) return false;
      scaleBaseZoom = 1;
    }

    cancelGesture(true);
    const state = stateRef.current;
    state.autoFit = true;
    state.fitZoom = nextZoom;
    state.fittedPreview = currentPreview;
    state.position = { x: 0, y: 0 };
    state.scaleBaseZoom = scaleBaseZoom;
    state.zoom = nextZoom;
    scheduleTransform();
    schedulePresentationReady();
    return true;
  }, [cancelGesture, schedulePresentationReady, scheduleTransform]);

  const scheduleFit = useCallback(() => {
    cancelScheduledFrame(fitFrameRef);
    fitFrameRef.current = requestAnimationFrame(() => {
      fitFrameRef.current = 0;
      fitToView();
    });
  }, [fitToView]);

  const disableAutoFit = useCallback(() => {
    cancelScheduledFrame(fitFrameRef);
    stateRef.current.autoFit = false;
  }, []);

  const zoomBy = useCallback(
    (factor: number) => {
      disableAutoFit();
      const state = stateRef.current;
      state.zoom = clampZoom(state.zoom * factor, state.fitZoom);
      scheduleTransform();
    },
    [disableAutoFit, scheduleTransform]
  );

  const reset = useCallback(() => {
    disableAutoFit();
    cancelGesture(true);
    const state = stateRef.current;
    state.position = { x: 0, y: 0 };
    state.zoom = 1;
    scheduleTransform();
    schedulePresentationReady();
  }, [
    cancelGesture,
    disableAutoFit,
    schedulePresentationReady,
    scheduleTransform,
  ]);

  useEffect(() => {
    return internalController(controller).connect({
      fitToView: () => fitToView(),
      reset,
      zoomIn: () => zoomBy(1.2),
      zoomOut: () => zoomBy(1 / 1.2),
    });
  }, [controller, fitToView, reset, zoomBy]);

  useEffect(() => {
    const host = shadowHostRef.current;
    const state = stateRef.current;
    presentedRef.current = null;
    cancelGesture(true);
    state.autoFit = true;
    state.fittedPreview = null;
    state.position = { x: 0, y: 0 };
    if (host) {
      host.style.removeProperty("width");
      host.style.removeProperty("height");
    }
    scheduleTransform();

    if (!host || !preview) return;
    const root = host.shadowRoot ?? host.attachShadow({ mode: "open" });
    root.replaceChildren(preview.takeNode());
    const requested = requestedPresentationRef.current;
    if (requested.preview !== preview) return;
    mountedPresentationRef.current = {
      key: requested.key,
      onReady: requested.onReady,
      preview,
      root,
    };
    scheduleFit();

    return () => {
      const mounted = mountedPresentationRef.current;
      if (mounted?.preview === preview && mounted.root === root) {
        mountedPresentationRef.current = null;
      }
      root.replaceChildren();
    };
  }, [cancelGesture, preview, scheduleFit, scheduleTransform]);

  useLayoutEffect(() => {
    requestedPresentationRef.current = {
      key: presentationKey,
      onReady: onPresentationReady,
      preview,
    };
    const mounted = mountedPresentationRef.current;
    if (!mounted || mounted.preview !== preview) return;
    if (mounted.key === presentationKey) {
      mounted.onReady = onPresentationReady;
      return;
    }
    mountedPresentationRef.current = {
      ...mounted,
      key: presentationKey,
      onReady: onPresentationReady,
    };
    presentedRef.current = null;
    schedulePresentationReady();
  }, [onPresentationReady, presentationKey, preview, schedulePresentationReady]);

  useEffect(() => {
    if (!preview) return;
    const container = containerRef.current;
    const content = contentRef.current;
    if (!container || !content) return;

    const handleVisibleLayout = () => {
      const state = stateRef.current;
      if (state.gesture.kind === "pinch") {
        state.gesture = {
          ...state.gesture,
          containerCenter: elementCenter(container),
        };
      }
      if (state.autoFit) {
        scheduleFit();
      } else {
        schedulePresentationReady();
      }
    };
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", handleVisibleLayout);
      return () => window.removeEventListener("resize", handleVisibleLayout);
    }

    const resizeObserver = new ResizeObserver(handleVisibleLayout);
    resizeObserver.observe(container);
    if (!preview.dimensions) resizeObserver.observe(content);
    const intersectionObserver =
      typeof IntersectionObserver === "undefined"
        ? null
        : new IntersectionObserver((entries) => {
            if (entries.some((entry) => entry.isIntersecting)) {
              handleVisibleLayout();
            }
          });
    intersectionObserver?.observe(container);

    return () => {
      resizeObserver.disconnect();
      intersectionObserver?.disconnect();
    };
  }, [preview, scheduleFit, schedulePresentationReady]);

  useEffect(() => {
    const handleBlur = () => cancelGesture(true);
    window.addEventListener("blur", handleBlur);
    return () => {
      window.removeEventListener("blur", handleBlur);
      cancelGesture(true);
      cancelScheduledFrame(transformFrameRef);
      cancelScheduledFrame(fitFrameRef);
      cancelScheduledFrame(readyFrameRef);
    };
  }, [cancelGesture]);

  const beginGesture = useCallback(() => {
    const state = stateRef.current;
    const pointers = [...state.pointers.entries()];
    if (pointers.length >= 2) {
      const [first, second] = pointers;
      if (!first || !second) return;
      const midpoint = pointMidpoint(first[1], second[1]);
      state.gesture = {
        kind: "pinch",
        containerCenter: elementCenter(containerRef.current),
        pointerIds: [first[0], second[0]],
        startDistance: Math.max(pointDistance(first[1], second[1]), 1),
        startMidpoint: midpoint,
        startPosition: state.position,
        startZoom: state.zoom,
      };
    } else if (pointers.length === 1) {
      const first = pointers[0];
      if (!first) return;
      state.gesture = {
        kind: "pan",
        pointerId: first[0],
        startPointer: first[1],
        startPosition: state.position,
      };
    } else {
      state.gesture = { kind: "idle" };
    }
    setDragging(state.gesture.kind !== "idle");
  }, [setDragging]);

  const finishPointer = useCallback(
    (pointerId: number, target: HTMLDivElement, releaseCapture: boolean) => {
      const state = stateRef.current;
      if (!state.pointers.delete(pointerId)) return;
      beginGesture();
      if (releaseCapture) releaseCapturedPointer(target, pointerId);
    },
    [beginGesture]
  );

  const handlePointerDown = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      if (event.pointerType === "mouse" && event.button !== 0) return;
      event.preventDefault();
      window.getSelection()?.removeAllRanges();
      try {
        event.currentTarget.setPointerCapture(event.pointerId);
      } catch {
        // Synthetic accessibility tooling may not create a capturable pointer.
      }
      disableAutoFit();
      const state = stateRef.current;
      state.pointers.set(event.pointerId, eventPoint(event));
      beginGesture();
    },
    [beginGesture, disableAutoFit]
  );

  const handlePointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const state = stateRef.current;
      if (!state.pointers.has(event.pointerId)) return;
      event.preventDefault();
      window.getSelection()?.removeAllRanges();
      state.pointers.set(event.pointerId, eventPoint(event));

      const gesture = state.gesture;
      if (gesture.kind === "pan") {
        const pointer = state.pointers.get(gesture.pointerId);
        if (!pointer) return;
        state.position = {
          x:
            gesture.startPosition.x +
            pointer.x -
            gesture.startPointer.x,
          y:
            gesture.startPosition.y +
            pointer.y -
            gesture.startPointer.y,
        };
      } else if (gesture.kind === "pinch") {
        const first = state.pointers.get(gesture.pointerIds[0]);
        const second = state.pointers.get(gesture.pointerIds[1]);
        if (!first || !second) return;
        const midpoint = pointMidpoint(first, second);
        const ratio =
          pointDistance(first, second) / gesture.startDistance;
        const nextZoom = clampZoom(
          gesture.startZoom * ratio,
          state.fitZoom
        );
        const zoomRatio = nextZoom / gesture.startZoom;
        const startOffset = {
          x: gesture.startMidpoint.x - gesture.containerCenter.x,
          y: gesture.startMidpoint.y - gesture.containerCenter.y,
        };
        const nextOffset = {
          x: midpoint.x - gesture.containerCenter.x,
          y: midpoint.y - gesture.containerCenter.y,
        };
        state.position = {
          x:
            nextOffset.x -
            (startOffset.x - gesture.startPosition.x) * zoomRatio,
          y:
            nextOffset.y -
            (startOffset.y - gesture.startPosition.y) * zoomRatio,
        };
        state.zoom = nextZoom;
      }
      scheduleTransform();
    },
    [scheduleTransform]
  );

  const handleWheel = useCallback(
    (event: globalThis.WheelEvent) => {
      event.preventDefault();
      disableAutoFit();
      const state = stateRef.current;
      state.zoom = clampZoom(
        state.zoom * Math.exp(-event.deltaY * 0.001),
        state.fitZoom
      );
      scheduleTransform();
    },
    [disableAutoFit, scheduleTransform]
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    container.addEventListener("wheel", handleWheel, { passive: false });
    return () => container.removeEventListener("wheel", handleWheel);
  }, [handleWheel]);

  return (
    <div
      ref={containerRef}
      className="relative h-full w-full cursor-grab touch-none select-none overflow-hidden"
      data-dragging="false"
      data-merman-svg-viewport="true"
      data-zoom="1"
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={(event) =>
        finishPointer(event.pointerId, event.currentTarget, true)
      }
      onPointerCancel={(event) =>
        finishPointer(event.pointerId, event.currentTarget, true)
      }
      onLostPointerCapture={(event) =>
        finishPointer(event.pointerId, event.currentTarget, false)
      }
      onDragStart={(event) => event.preventDefault()}
    >
      {preview ? (
        <div
          ref={positionLayerRef}
          className="absolute left-1/2 top-1/2"
          data-merman-viewport-position-layer="true"
          style={{ transform: "translate(0px, 0px)" }}
        >
          <div
            ref={scaleLayerRef}
            data-merman-viewport-scale-layer="true"
            style={{
              transform: "translate(-50%, -50%) scale(1)",
              transformOrigin: "center center",
            }}
          >
            <div
              ref={contentRef}
              className="preview-container inline-flex rounded-lg bg-white p-4 shadow-sm"
            >
              <div ref={shadowHostRef} className="block shrink-0" />
            </div>
          </div>
        </div>
      ) : (
        empty
      )}
    </div>
  );
}

function internalController(
  controller: SvgViewportController
): SvgViewportControllerImpl {
  return controller as SvgViewportControllerImpl;
}

function measureRenderedContent(content: HTMLDivElement): SvgDimensions | null {
  if (content.offsetWidth <= 0 || content.offsetHeight <= 0) {
    return null;
  }
  return { width: content.offsetWidth, height: content.offsetHeight };
}

function minimumZoom(fitZoom: number): number {
  return Math.min(0.1, fitZoom / 10);
}

function clampZoom(value: number, fitZoom: number): number {
  return Math.max(minimumZoom(fitZoom), Math.min(5, value));
}

function measureElementInsets(element: HTMLElement): SvgDimensions {
  const style = window.getComputedStyle(element);
  return {
    width:
      parseCssPixels(style.paddingLeft) +
      parseCssPixels(style.paddingRight) +
      parseCssPixels(style.borderLeftWidth) +
      parseCssPixels(style.borderRightWidth),
    height:
      parseCssPixels(style.paddingTop) +
      parseCssPixels(style.paddingBottom) +
      parseCssPixels(style.borderTopWidth) +
      parseCssPixels(style.borderBottomWidth),
  };
}

function parseCssPixels(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function eventPoint(event: Pick<PointerEvent, "clientX" | "clientY">): Point {
  return { x: event.clientX, y: event.clientY };
}

function pointMidpoint(first: Point, second: Point): Point {
  return { x: (first.x + second.x) / 2, y: (first.y + second.y) / 2 };
}

function pointDistance(first: Point, second: Point): number {
  return Math.hypot(second.x - first.x, second.y - first.y);
}

function elementCenter(element: HTMLElement | null): Point {
  if (!element) return { x: 0, y: 0 };
  const bounds = element.getBoundingClientRect();
  return {
    x: bounds.left + bounds.width / 2,
    y: bounds.top + bounds.height / 2,
  };
}

function releaseCapturedPointer(
  element: HTMLElement,
  pointerId: number
): void {
  if (!element.hasPointerCapture(pointerId)) return;
  try {
    element.releasePointerCapture(pointerId);
  } catch {
    // Capture may already have been released by the browser.
  }
}

function cancelScheduledFrame(frame: { current: number }): void {
  if (frame.current) cancelAnimationFrame(frame.current);
  frame.current = 0;
}

function isPositiveFinite(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}
