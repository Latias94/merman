import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent,
  type ReactNode,
  type RefObject,
  type WheelEvent,
} from "react";
import { cn } from "@/lib/utils";

interface Point {
  x: number;
  y: number;
}

export interface SvgViewportController {
  zoom: number;
  position: Point;
  isDragging: boolean;
  containerRef: RefObject<HTMLDivElement | null>;
  contentRef: RefObject<HTMLDivElement | null>;
  zoomIn(): void;
  zoomOut(): void;
  reset(): void;
  fitToView(): void;
  handleWheel(event: WheelEvent<HTMLDivElement>): void;
  handlePointerDown(event: PointerEvent<HTMLDivElement>): void;
  handlePointerMove(event: PointerEvent<HTMLDivElement>): void;
  handlePointerUp(event: PointerEvent<HTMLDivElement>): void;
}

interface UseSvgViewportOptions {
  svg: string | null;
  enabled: boolean;
}

export function useSvgViewport({
  svg,
  enabled,
}: UseSvgViewportOptions): SvgViewportController {
  const [zoom, setZoom] = useState(1);
  const [position, setPosition] = useState<Point>({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState<Point>({ x: 0, y: 0 });
  const [isAutoFit, setIsAutoFit] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  const fitToView = useCallback(() => {
    const container = containerRef.current;
    const content = contentRef.current;
    if (!container || !content) return;

    const contentWidth = content.offsetWidth;
    const contentHeight = content.offsetHeight;
    if (contentWidth <= 0 || contentHeight <= 0) return;

    const availableWidth = Math.max(container.clientWidth - 48, 1);
    const availableHeight = Math.max(container.clientHeight - 48, 1);
    const nextZoom = Math.max(
      0.1,
      Math.min(1, availableWidth / contentWidth, availableHeight / contentHeight)
    );

    setZoom(Number(nextZoom.toFixed(3)));
    setPosition({ x: 0, y: 0 });
  }, []);

  const zoomIn = useCallback(() => {
    setIsAutoFit(false);
    setZoom((value) => Math.min(value * 1.2, 5));
  }, []);

  const zoomOut = useCallback(() => {
    setIsAutoFit(false);
    setZoom((value) => Math.max(value / 1.2, 0.1));
  }, []);

  const reset = useCallback(() => {
    setIsAutoFit(false);
    setZoom(1);
    setPosition({ x: 0, y: 0 });
  }, []);

  const handleWheel = useCallback((event: WheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    setIsAutoFit(false);
    const delta = Math.exp(-event.deltaY * 0.001);
    setZoom((value) => Math.max(0.1, Math.min(5, value * delta)));
  }, []);

  const handlePointerDown = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      if (event.button !== 0) return;

      event.preventDefault();
      window.getSelection()?.removeAllRanges();
      event.currentTarget.setPointerCapture(event.pointerId);
      setIsAutoFit(false);
      setIsDragging(true);
      setDragStart({
        x: event.clientX - position.x,
        y: event.clientY - position.y,
      });
    },
    [position]
  );

  const handlePointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      if (!isDragging) return;

      event.preventDefault();
      window.getSelection()?.removeAllRanges();
      setPosition({
        x: event.clientX - dragStart.x,
        y: event.clientY - dragStart.y,
      });
    },
    [dragStart, isDragging]
  );

  const handlePointerUp = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      if (isDragging && event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
      setIsDragging(false);
    },
    [isDragging]
  );

  useEffect(() => {
    if (!enabled || !svg) return;

    setIsAutoFit(true);
    const frame = requestAnimationFrame(fitToView);
    return () => cancelAnimationFrame(frame);
  }, [enabled, fitToView, svg]);

  useEffect(() => {
    if (!enabled || !svg || !isAutoFit) return;

    const container = containerRef.current;
    if (!container || typeof ResizeObserver === "undefined") return;

    let frame = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(fitToView);
    });

    observer.observe(container);

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [enabled, fitToView, isAutoFit, svg]);

  return {
    zoom,
    position,
    isDragging,
    containerRef,
    contentRef,
    zoomIn,
    zoomOut,
    reset,
    fitToView,
    handleWheel,
    handlePointerDown,
    handlePointerMove,
    handlePointerUp,
  };
}

interface SvgViewportProps {
  svg: string | null;
  controller: SvgViewportController;
  className?: string;
  contentClassName?: string;
  empty?: ReactNode;
}

export function SvgViewport({
  svg,
  controller,
  className,
  contentClassName,
  empty,
}: SvgViewportProps) {
  return (
    <div
      ref={controller.containerRef}
      className={cn(
        "relative h-full w-full overflow-hidden cursor-grab select-none touch-none",
        controller.isDragging && "cursor-grabbing",
        className
      )}
      onWheel={controller.handleWheel}
      onPointerDown={controller.handlePointerDown}
      onPointerMove={controller.handlePointerMove}
      onPointerUp={controller.handlePointerUp}
      onPointerCancel={controller.handlePointerUp}
      onDragStart={(event) => event.preventDefault()}
    >
      {svg ? (
        <div
          className="absolute left-1/2 top-1/2 will-change-transform"
          style={{
            transform: `translate3d(${controller.position.x}px, ${controller.position.y}px, 0)`,
          }}
        >
          <div
            className="will-change-transform"
            style={{
              transform: `translate(-50%, -50%) scale(${controller.zoom})`,
              transformOrigin: "center center",
            }}
          >
            <div
              ref={controller.contentRef}
              className={cn(
                "preview-container inline-flex bg-white rounded-lg shadow-sm p-4",
                contentClassName
              )}
              dangerouslySetInnerHTML={{ __html: svg }}
            />
          </div>
        </div>
      ) : (
        empty
      )}
    </div>
  );
}
