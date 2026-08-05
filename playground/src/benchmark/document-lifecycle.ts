export type BenchmarkDocumentLifecycleSignal =
  | Readonly<{
      kind: "visibility-hidden";
      visibilityState: string;
    }>
  | Readonly<{
      kind: "freeze";
      visibilityState: string;
    }>
  | Readonly<{
      kind: "resume";
      visibilityState: string;
    }>
  | Readonly<{
      kind: "pagehide";
      persisted: boolean;
      visibilityState: string;
    }>
  | Readonly<{
      kind: "pageshow";
      persisted: boolean;
      visibilityState: string;
    }>;

export interface BenchmarkLifecycleEventTarget {
  addEventListener(type: string, listener: (event: unknown) => void): void;
  removeEventListener(type: string, listener: (event: unknown) => void): void;
}

export function createBrowserBenchmarkDocumentLifecycle(): BenchmarkDocumentLifecycle {
  return createBenchmarkDocumentLifecycle({
    documentTarget: asLifecycleEventTarget(document),
    getVisibilityState: () => document.visibilityState,
    windowTarget: asLifecycleEventTarget(window),
  });
}

export interface BenchmarkDocumentLifecycle {
  getVisibilityState(): string;
  subscribe(
    listener: (signal: BenchmarkDocumentLifecycleSignal) => void
  ): () => void;
}

export interface BenchmarkDocumentLifecycleDependencies {
  readonly documentTarget: BenchmarkLifecycleEventTarget;
  getVisibilityState(): string;
  readonly windowTarget: BenchmarkLifecycleEventTarget;
}

type BenchmarkLifecycleSignalWithoutVisibility =
  BenchmarkDocumentLifecycleSignal extends infer Signal
    ? Signal extends BenchmarkDocumentLifecycleSignal
      ? Omit<Signal, "visibilityState">
      : never
    : never;

export function createBenchmarkDocumentLifecycle({
  documentTarget,
  getVisibilityState,
  windowTarget,
}: BenchmarkDocumentLifecycleDependencies): BenchmarkDocumentLifecycle {
  return Object.freeze({
    getVisibilityState,
    subscribe(
      listener: (signal: BenchmarkDocumentLifecycleSignal) => void
    ) {
      const emit = (
        signal: BenchmarkLifecycleSignalWithoutVisibility
      ) => {
        listener(
          Object.freeze({
            ...signal,
            visibilityState: getVisibilityState(),
          }) as BenchmarkDocumentLifecycleSignal
        );
      };
      const onVisibilityChange = () => {
        const visibilityState = getVisibilityState();
        if (visibilityState === "visible") return;
        listener(Object.freeze({ kind: "visibility-hidden", visibilityState }));
      };
      const onFreeze = () => emit({ kind: "freeze" });
      const onResume = () => emit({ kind: "resume" });
      const onPageHide = (event: unknown) =>
        emit({ kind: "pagehide", persisted: readPersisted(event) });
      const onPageShow = (event: unknown) =>
        emit({ kind: "pageshow", persisted: readPersisted(event) });

      documentTarget.addEventListener("visibilitychange", onVisibilityChange);
      documentTarget.addEventListener("freeze", onFreeze);
      documentTarget.addEventListener("resume", onResume);
      windowTarget.addEventListener("pagehide", onPageHide);
      windowTarget.addEventListener("pageshow", onPageShow);

      let subscribed = true;
      return () => {
        if (!subscribed) return;
        subscribed = false;
        documentTarget.removeEventListener(
          "visibilitychange",
          onVisibilityChange
        );
        documentTarget.removeEventListener("freeze", onFreeze);
        documentTarget.removeEventListener("resume", onResume);
        windowTarget.removeEventListener("pagehide", onPageHide);
        windowTarget.removeEventListener("pageshow", onPageShow);
      };
    },
  });
}

function readPersisted(event: unknown): boolean {
  return Boolean(
    event &&
      typeof event === "object" &&
      "persisted" in event &&
      (event as { readonly persisted?: unknown }).persisted === true
  );
}

function asLifecycleEventTarget(target: EventTarget): BenchmarkLifecycleEventTarget {
  type NativeEventListener = Parameters<EventTarget["addEventListener"]>[1];
  return {
    addEventListener(type, listener) {
      target.addEventListener(type, listener as NativeEventListener);
    },
    removeEventListener(type, listener) {
      target.removeEventListener(type, listener as NativeEventListener);
    },
  };
}
