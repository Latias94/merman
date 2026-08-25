import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent,
  type ReactNode,
} from "react";

import {
  prepareSvgForResponsivePreview,
  type SvgDimensions,
} from "@/src/lib/svg-geometry";
import {
  resolveConnectedBounds,
  type SvgBounds,
} from "@/src/lib/svg-connected-bounds";
import type { MermaidCanvasTone } from "@/src/lib/mermaid-canvas-tone";
import type { SvgPresentationMode } from "@/src/lib/svg-presentation";
import type { NavigableInlineSvg } from "@/src/runtime/render-artifact";

interface Point {
  readonly x: number;
  readonly y: number;
}

interface FitGeometry {
  readonly centerOffset: Point;
  readonly dimensions: SvgDimensions;
}

interface RootOverflowSnapshot {
  readonly priority: string;
  readonly value: string;
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
      readonly kind: "pending";
      readonly pointerId: number;
      readonly startPointer: Point;
      readonly startPosition: Point;
    }
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
  readonly anchorPointers: Set<number>;
  readonly pointers: Map<number, Point>;
  autoFit: boolean;
  fitZoom: number;
  fittedPreview: PreparedSvgPreview | null;
  gesture: Gesture;
  position: Point;
  promotedAnchorGesture: boolean;
  scaleBaseZoom: number;
  zoom: number;
}

interface AppliedTransform {
  readonly autoFit: boolean;
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
  readonly originalRootOverflow: RootOverflowSnapshot | null;
  readonly preview: PreparedSvgPreview;
  readonly root: ShadowRoot;
}

interface RequestedPresentation {
  readonly key: number | null;
  readonly onReady?: (at: number) => void;
  readonly preview: PreparedSvgPreview | null;
}

interface SvgMountFailure {
  readonly error: Error;
  readonly preview: PreparedSvgPreview;
}

interface PreparedSvgPresentation {
  readonly error: Error | null;
  readonly preview: PreparedSvgPreview | null;
}

interface SvgViewportProps {
  artifact: NavigableInlineSvg | null;
  canvasTone: MermaidCanvasTone;
  presentationKey: number | null;
  controller: SvgViewportController;
  empty?: ReactNode;
  navigationEnabled?: boolean;
  onPresentationReady?: (at: number) => void;
  renderMountError?: (error: Error) => ReactNode;
  showSvgBounds: boolean;
  presentationMode: SvgPresentationMode;
}

const PAN_ACTIVATION_DISTANCE = 6;
const SUPPRESSED_ANCHOR_CLICK_WINDOW_MS = 1_000;
const NAVIGATION_ARIA_DISABLED_STATE = new WeakMap<Element, string | null>();

export function SvgViewport({
  artifact,
  canvasTone,
  presentationKey,
  controller,
  empty,
  navigationEnabled = true,
  onPresentationReady,
  renderMountError,
  showSvgBounds,
  presentationMode,
}: SvgViewportProps) {
  const prepared = useMemo<PreparedSvgPresentation>(() => {
    if (!artifact) return { error: null, preview: null };
    try {
      const preview = prepareSvgForResponsivePreview(artifact, document);
      return preview
        ? { error: null, preview }
        : {
            error: new Error("Merman preview could not parse the rendered SVG root."),
            preview: null,
          };
    } catch (error) {
      return { error: normalizeMountError(error), preview: null };
    }
  }, [artifact]);
  const preview = prepared.preview;
  const containerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const positionLayerRef = useRef<HTMLDivElement>(null);
  const scaleLayerRef = useRef<HTMLDivElement>(null);
  const shadowHostRef = useRef<HTMLDivElement>(null);
  const transformFrameRef = useRef(0);
  const fitFrameRef = useRef(0);
  const readyFrameRef = useRef(0);
  const suppressedAnchorClickTimerRef = useRef(0);
  const suppressNextAnchorClickRef = useRef(false);
  const [mountFailure, setMountFailure] = useState<SvgMountFailure | null>(null);
  const navigationEnabledRef = useRef(navigationEnabled);
  const presentationModeRef = useRef(presentationMode);
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
    anchorPointers: new Set(),
    autoFit: true,
    fitZoom: 1,
    fittedPreview: null,
    gesture: { kind: "idle" },
    pointers: new Map(),
    position: { x: 0, y: 0 },
    promotedAnchorGesture: false,
    scaleBaseZoom: 1,
    zoom: 1,
  });
  previewRef.current = preview;
  navigationEnabledRef.current = navigationEnabled;
  presentationModeRef.current = presentationMode;
  const presentationError =
    prepared.error ??
    (mountFailure?.preview === preview ? mountFailure.error : null);

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
      (previous?.container !== container ||
        previous.autoFit !== state.autoFit ||
        !Object.is(previous.zoom, state.zoom))
    ) {
      container.dataset.autoFit = String(state.autoFit);
      container.dataset.zoom = String(state.zoom);
    }
    if (!previous || !Object.is(previous.zoom, state.zoom)) {
      internalController(controller).publishZoom(state.zoom);
    }
    lastAppliedRef.current = {
      autoFit: state.autoFit,
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

  const clearAnchorClickSuppression = useCallback(() => {
    suppressNextAnchorClickRef.current = false;
    if (!suppressedAnchorClickTimerRef.current) return;
    window.clearTimeout(suppressedAnchorClickTimerRef.current);
    suppressedAnchorClickTimerRef.current = 0;
  }, []);

  const armAnchorClickSuppression = useCallback(() => {
    clearAnchorClickSuppression();
    suppressNextAnchorClickRef.current = true;
    suppressedAnchorClickTimerRef.current = window.setTimeout(() => {
      suppressNextAnchorClickRef.current = false;
      suppressedAnchorClickTimerRef.current = 0;
    }, SUPPRESSED_ANCHOR_CLICK_WINDOW_MS);
  }, [clearAnchorClickSuppression]);

  const cancelGesture = useCallback((releaseCapture = false) => {
    const state = stateRef.current;
    const container = containerRef.current;
    const pointerIds = [...state.pointers.keys()];
    state.pointers.clear();
    state.anchorPointers.clear();
    state.gesture = { kind: "idle" };
    state.promotedAnchorGesture = false;
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
    let centerOffset: Point = { x: 0, y: 0 };

    if (intrinsicSize) {
      const fitGeometry = resolveFitGeometry(
        mountedPresentationRef.current,
        currentPreview,
        presentationModeRef.current,
        intrinsicSize
      );
      centerOffset = fitGeometry.centerOffset;
      const insets = measureElementInsets(content);
      const availableSvgWidth = Math.max(availableWidth - insets.width, 1);
      const availableSvgHeight = Math.max(availableHeight - insets.height, 1);
      nextZoom = Math.min(
        1,
        availableSvgWidth / fitGeometry.dimensions.width,
        availableSvgHeight / fitGeometry.dimensions.height
      );
      if (!isPositiveFinite(nextZoom)) return false;
      if (currentPreview.rootSizing === "responsive") {
        scaleBaseZoom = resolveResponsiveSvgBaseZoom(
          intrinsicSize,
          nextZoom
        );
        shadowHost.style.width = `${intrinsicSize.width * scaleBaseZoom}px`;
        shadowHost.style.height = `${intrinsicSize.height * scaleBaseZoom}px`;
      } else {
        shadowHost.style.width = `${intrinsicSize.width}px`;
        shadowHost.style.height = `${intrinsicSize.height}px`;
        scaleBaseZoom = 1;
      }
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
    state.position = {
      x: -centerOffset.x * nextZoom,
      y: -centerOffset.y * nextZoom,
    };
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
    root.replaceChildren();
    let node: Element;
    try {
      node = preview.takeNode();
    } catch (error) {
      setMountFailure({ error: normalizeMountError(error), preview });
      return;
    }
    setMountFailure((current) =>
      current?.preview === preview ? null : current,
    );
    root.replaceChildren(node);
    stabilizeInheritedSvgColor(node, host);
    setNavigableAnchorsEnabled(root, navigationEnabledRef.current);
    const svg = node instanceof SVGSVGElement ? node : null;
    const originalRootOverflow = svg ? readRootOverflow(svg) : null;
    applySvgPresentationMode(svg, originalRootOverflow, presentationModeRef.current);
    const cleanupMountedNode = () => {
      restoreRootOverflow(svg, originalRootOverflow);
      root.replaceChildren();
    };
    const requested = requestedPresentationRef.current;
    if (requested.preview !== preview) {
      cleanupMountedNode();
      return;
    }
    mountedPresentationRef.current = {
      key: requested.key,
      onReady: requested.onReady,
      originalRootOverflow,
      preview,
      root,
    };
    scheduleFit();

    return () => {
      const mounted = mountedPresentationRef.current;
      if (mounted?.preview === preview && mounted.root === root) {
        mountedPresentationRef.current = null;
      }
      cleanupMountedNode();
    };
  }, [cancelGesture, preview, scheduleFit, scheduleTransform]);

  useLayoutEffect(() => {
    const root = mountedPresentationRef.current?.root;
    if (root) setNavigableAnchorsEnabled(root, navigationEnabled);
  }, [navigationEnabled, preview]);

  useLayoutEffect(() => {
    const mounted = mountedPresentationRef.current;
    if (!mounted || mounted.preview !== preview) return;
    applySvgPresentationMode(
      mounted.root.querySelector("svg"),
      mounted.originalRootOverflow,
      presentationMode
    );
    if (stateRef.current.autoFit) {
      scheduleFit();
    } else {
      schedulePresentationReady();
    }
  }, [presentationMode, preview, scheduleFit, schedulePresentationReady]);

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
      clearAnchorClickSuppression();
      cancelScheduledFrame(transformFrameRef);
      cancelScheduledFrame(fitFrameRef);
      cancelScheduledFrame(readyFrameRef);
    };
  }, [cancelGesture, clearAnchorClickSuppression]);

  const rebasePromotedGesture = useCallback((pending?: Extract<Gesture, { kind: "pending" }>) => {
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
        startPointer: pending?.pointerId === first[0] ? pending.startPointer : first[1],
        startPosition: pending?.pointerId === first[0] ? pending.startPosition : state.position,
      };
    } else {
      state.gesture = { kind: "idle" };
    }
    setDragging(state.gesture.kind === "pan" || state.gesture.kind === "pinch");
  }, [setDragging]);

  const promoteGesture = useCallback(
    (target: HTMLDivElement, pending?: Extract<Gesture, { kind: "pending" }>) => {
      const state = stateRef.current;
      state.promotedAnchorGesture ||= state.anchorPointers.size > 0;
      for (const pointerId of state.pointers.keys()) {
        capturePointer(target, pointerId);
      }
      disableAutoFit();
      rebasePromotedGesture(pending);
    },
    [disableAutoFit, rebasePromotedGesture]
  );

  const finishPointer = useCallback(
    (
      pointerId: number,
      target: HTMLDivElement,
      releaseCapture: boolean,
      armClickSuppression: boolean
    ) => {
      const state = stateRef.current;
      if (!state.pointers.delete(pointerId)) return;
      const endedOnAnchor = state.anchorPointers.delete(pointerId);
      const wasPromoted =
        state.gesture.kind === "pan" || state.gesture.kind === "pinch";
      if (wasPromoted) {
        if (
          state.promotedAnchorGesture &&
          endedOnAnchor &&
          armClickSuppression
        ) {
          armAnchorClickSuppression();
        }
        rebasePromotedGesture();
      } else {
        state.gesture = { kind: "idle" };
        setDragging(false);
      }
      if (state.pointers.size === 0) {
        state.promotedAnchorGesture = false;
      }
      if (releaseCapture) releaseCapturedPointer(target, pointerId);
    },
    [armAnchorClickSuppression, rebasePromotedGesture, setDragging]
  );

  const handlePointerDown = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      if (event.pointerType === "mouse" && event.button !== 0) return;
      const state = stateRef.current;
      if (state.pointers.size === 0) clearAnchorClickSuppression();
      const point = eventPoint(event);
      state.pointers.set(event.pointerId, point);
      if (
        navigationEnabled &&
        hasNavigableAnchorInPath(event.nativeEvent.composedPath())
      ) {
        state.anchorPointers.add(event.pointerId);
      }

      if (state.pointers.size >= 2) {
        event.preventDefault();
        window.getSelection()?.removeAllRanges();
        promoteGesture(event.currentTarget);
        return;
      }

      state.gesture = {
        kind: "pending",
        pointerId: event.pointerId,
        startPointer: point,
        startPosition: state.position,
      };
      setDragging(false);
    },
    [clearAnchorClickSuppression, navigationEnabled, promoteGesture, setDragging]
  );

  const handlePointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const state = stateRef.current;
      if (!state.pointers.has(event.pointerId)) return;
      const point = eventPoint(event);
      state.pointers.set(event.pointerId, point);

      const pending = state.gesture;
      if (pending.kind === "pending") {
        if (
          pending.pointerId !== event.pointerId ||
          pointDistance(pending.startPointer, point) < PAN_ACTIVATION_DISTANCE
        ) {
          return;
        }
        event.preventDefault();
        window.getSelection()?.removeAllRanges();
        promoteGesture(event.currentTarget, pending);
      } else if (pending.kind === "idle") {
        return;
      } else {
        event.preventDefault();
        window.getSelection()?.removeAllRanges();
      }

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
    [promoteGesture, scheduleTransform]
  );

  const handleAnchorClickCapture = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      if (!hasNavigableAnchorInPath(event.nativeEvent.composedPath())) return;
      const suppressPromotedPointerClick =
        suppressNextAnchorClickRef.current && event.detail > 0;
      if (navigationEnabled && !suppressPromotedPointerClick) return;

      event.preventDefault();
      event.stopPropagation();
      if (suppressPromotedPointerClick) clearAnchorClickSuppression();
    },
    [clearAnchorClickSuppression, navigationEnabled]
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
      className="preview-canvas relative h-full w-full cursor-grab touch-none select-none overflow-hidden"
      data-dragging="false"
      data-auto-fit="true"
      data-merman-svg-viewport="true"
      data-preview-canvas-tone={canvasTone}
      data-svg-presentation-mode={presentationMode}
      data-zoom="1"
      onAuxClickCapture={handleAnchorClickCapture}
      onClickCapture={handleAnchorClickCapture}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={(event) =>
        finishPointer(event.pointerId, event.currentTarget, true, true)
      }
      onPointerCancel={(event) =>
        finishPointer(event.pointerId, event.currentTarget, true, false)
      }
      onLostPointerCapture={(event) =>
        finishPointer(event.pointerId, event.currentTarget, false, false)
      }
      onDragStart={(event) => event.preventDefault()}
    >
      {presentationError ? (
        (renderMountError?.(presentationError) ?? empty)
      ) : preview ? (
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
              className="preview-container relative inline-flex"
              data-svg-presentation-mode={presentationMode}
            >
              <div ref={shadowHostRef} className="block shrink-0" />
              {showSvgBounds && (
                <div
                  aria-hidden="true"
                  className="preview-svg-bounds"
                  data-merman-svg-bounds="true"
                />
              )}
            </div>
          </div>
        </div>
      ) : (
        empty
      )}
    </div>
  );
}

function normalizeMountError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

function stabilizeInheritedSvgColor(node: Element, host: HTMLElement): void {
  if (!(node instanceof SVGSVGElement)) return;
  const view = node.ownerDocument.defaultView;
  if (!view) return;

  const svgStyle = view.getComputedStyle(node);
  if (svgStyle.color !== view.getComputedStyle(host).color) return;
  const fill = svgStyle.fill.trim();
  if (!fill || !view.CSS.supports("color", fill)) return;

  node.style.setProperty("color", fill);
}

function internalController(
  controller: SvgViewportController
): SvgViewportControllerImpl {
  return controller as SvgViewportControllerImpl;
}

function hasNavigableAnchorInPath(path: readonly EventTarget[]): boolean {
  return path.some(
    (target) =>
      target instanceof Element &&
      target.localName.toLowerCase() === "a" &&
      (target.hasAttribute("href") || target.hasAttribute("xlink:href"))
  );
}

function setNavigableAnchorsEnabled(root: ShadowRoot, enabled: boolean): void {
  for (const anchor of root.querySelectorAll("a")) {
    if (enabled) {
      if (!NAVIGATION_ARIA_DISABLED_STATE.has(anchor)) continue;
      const original = NAVIGATION_ARIA_DISABLED_STATE.get(anchor);
      NAVIGATION_ARIA_DISABLED_STATE.delete(anchor);
      if (original === null || original === undefined) {
        anchor.removeAttribute("aria-disabled");
      } else {
        anchor.setAttribute("aria-disabled", original);
      }
      continue;
    }

    if (!NAVIGATION_ARIA_DISABLED_STATE.has(anchor)) {
      NAVIGATION_ARIA_DISABLED_STATE.set(
        anchor,
        anchor.getAttribute("aria-disabled")
      );
    }
    anchor.setAttribute("aria-disabled", "true");
  }
}

function resolveFitGeometry(
  mounted: MountedPresentation | null,
  preview: PreparedSvgPreview,
  presentationMode: SvgPresentationMode,
  intrinsicSize: SvgDimensions
): FitGeometry {
  const fallback = {
    centerOffset: { x: 0, y: 0 },
    dimensions: intrinsicSize,
  } satisfies FitGeometry;
  if (
    presentationMode !== "infinite" ||
    mounted?.preview !== preview
  ) {
    return fallback;
  }

  const svg = mounted.root.querySelector("svg");
  return svg ? measureVisualFitGeometry(svg, intrinsicSize) ?? fallback : fallback;
}

function measureVisualFitGeometry(
  svg: SVGSVGElement,
  intrinsicSize: SvgDimensions
): FitGeometry | null {
  const viewBox = svg.viewBox.baseVal;
  if (
    !isFiniteRectangle(viewBox.x, viewBox.y, viewBox.width, viewBox.height) ||
    viewBox.width <= 0 ||
    viewBox.height <= 0
  ) {
    return null;
  }

  const visualBounds = measureConnectedVisualBounds(svg, {
    bottom: viewBox.y + viewBox.height,
    left: viewBox.x,
    right: viewBox.x + viewBox.width,
    top: viewBox.y,
  });
  if (!visualBounds) return null;

  const { bottom, left, right, top } = visualBounds;
  const scaleX = intrinsicSize.width / viewBox.width;
  const scaleY = intrinsicSize.height / viewBox.height;
  const width = (right - left) * scaleX;
  const height = (bottom - top) * scaleY;
  const centerOffset = {
    x:
      ((left + right) / 2 - (viewBox.x + viewBox.width / 2)) * scaleX,
    y:
      ((top + bottom) / 2 - (viewBox.y + viewBox.height / 2)) * scaleY,
  };
  if (
    !isPositiveFinite(width) ||
    !isPositiveFinite(height) ||
    !Number.isFinite(centerOffset.x) ||
    !Number.isFinite(centerOffset.y)
  ) {
    return null;
  }

  return {
    centerOffset,
    dimensions: { width, height },
  };
}

function measureConnectedVisualBounds(
  svg: SVGSVGElement,
  rootBounds: SvgBounds
): SvgBounds | null {
  const rootTransform = svg.getCTM();
  if (!rootTransform) return null;

  let toRoot: DOMMatrix;
  try {
    toRoot = rootTransform.inverse();
  } catch {
    return null;
  }

  const candidates = Array.from(svg.querySelectorAll<SVGGraphicsElement>("*"))
    .filter(isMeasurableSvgGraphic)
    .flatMap((element) => {
      const bounds = measureSvgGraphicInRootSpace(element, toRoot);
      return bounds ? [bounds] : [];
    });
  return resolveConnectedBounds(rootBounds, candidates);
}

function isMeasurableSvgGraphic(element: SVGGraphicsElement): boolean {
  if (
    element.localName === "a" ||
    element.localName === "g" ||
    element.localName === "svg" ||
    element.localName === "switch"
  ) {
    return false;
  }

  for (
    let ancestor = element.parentElement;
    ancestor && ancestor instanceof SVGElement;
    ancestor = ancestor.parentElement
  ) {
    if (
      ancestor.localName === "clipPath" ||
      ancestor.localName === "defs" ||
      ancestor.localName === "marker" ||
      ancestor.localName === "mask" ||
      ancestor.localName === "pattern" ||
      ancestor.localName === "symbol"
    ) {
      return false;
    }
  }

  const style = getComputedStyle(element);
  return (
    style.display !== "none" &&
    style.visibility !== "hidden" &&
    style.visibility !== "collapse"
  );
}

function measureSvgGraphicInRootSpace(
  element: SVGGraphicsElement,
  rootInverse: DOMMatrix
): SvgBounds | null {
  try {
    const box = element.getBBox();
    const elementTransform = element.getCTM();
    if (
      !elementTransform ||
      !isFiniteRectangle(box.x, box.y, box.width, box.height) ||
      box.width < 0 ||
      box.height < 0
    ) {
      return null;
    }

    const transform = rootInverse.multiply(elementTransform);
    const corners = [
      transformSvgPoint(transform, box.x, box.y),
      transformSvgPoint(transform, box.x + box.width, box.y),
      transformSvgPoint(transform, box.x, box.y + box.height),
      transformSvgPoint(
        transform,
        box.x + box.width,
        box.y + box.height
      ),
    ];
    const left = Math.min(...corners.map((point) => point.x));
    const right = Math.max(...corners.map((point) => point.x));
    const top = Math.min(...corners.map((point) => point.y));
    const bottom = Math.max(...corners.map((point) => point.y));
    return [left, right, top, bottom].every(Number.isFinite)
      ? { bottom, left, right, top }
      : null;
  } catch {
    return null;
  }
}

function transformSvgPoint(matrix: DOMMatrix, x: number, y: number): Point {
  return {
    x: matrix.a * x + matrix.c * y + matrix.e,
    y: matrix.b * x + matrix.d * y + matrix.f,
  };
}

function resolveResponsiveSvgBaseZoom(
  intrinsicSize: SvgDimensions,
  fitZoom: number
): number {
  const minimumBaseZoom = Math.max(
    1 / intrinsicSize.width,
    1 / intrinsicSize.height
  );
  return Number.isFinite(minimumBaseZoom)
    ? Math.max(fitZoom, minimumBaseZoom)
    : fitZoom;
}

function applySvgPresentationMode(
  svg: SVGSVGElement | null,
  originalOverflow: RootOverflowSnapshot | null,
  presentationMode: SvgPresentationMode
): void {
  if (!svg) return;
  if (presentationMode === "infinite") {
    svg.style.setProperty("overflow", "visible", "important");
    return;
  }
  restoreRootOverflow(svg, originalOverflow);
}

function readRootOverflow(svg: SVGSVGElement): RootOverflowSnapshot {
  return {
    priority: svg.style.getPropertyPriority("overflow"),
    value: svg.style.getPropertyValue("overflow"),
  };
}

function restoreRootOverflow(
  svg: SVGSVGElement | null,
  originalOverflow: RootOverflowSnapshot | null
): void {
  if (!svg) return;
  if (originalOverflow?.value) {
    svg.style.setProperty(
      "overflow",
      originalOverflow.value,
      originalOverflow.priority
    );
  } else {
    svg.style.removeProperty("overflow");
  }
}

function isFiniteRectangle(
  x: number,
  y: number,
  width: number,
  height: number
): boolean {
  return [x, y, width, height].every(Number.isFinite);
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

function capturePointer(element: HTMLElement, pointerId: number): void {
  try {
    element.setPointerCapture(pointerId);
  } catch {
    // Synthetic accessibility tooling may not create a capturable pointer.
  }
}

function cancelScheduledFrame(frame: { current: number }): void {
  if (frame.current) cancelAnimationFrame(frame.current);
  frame.current = 0;
}

function isPositiveFinite(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}
