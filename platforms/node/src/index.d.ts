export type MermanResourceProfile =
  | "interactive"
  | "constrained"
  | "trusted-native"
  | "unbounded-for-trusted-input";

export interface MermanBindingOptions {
  version?: 1;
  runtime_policy?: "deterministic" | "native";
  resources?: {
    profile?: MermanResourceProfile;
    limits?: Record<string, number>;
  };
  [key: string]: unknown;
}

export interface CreateNodeEngineOptions {
  bindingOptions?: MermanBindingOptions;
  concurrency?: number;
  maxQueue?: number;
}

export interface RenderSvgOptions {
  /**
   * Cancels only while the request is waiting in the JS queue. Work that has started is not
   * preempted and its Promise settles with the actual operation result.
   */
  signal?: AbortSignal;
}

export interface MermanQueueState {
  readonly active: number;
  readonly pending: number;
  readonly concurrency: number;
  readonly maxQueue: number;
  readonly state: "open" | "disposing" | "disposed";
}

export declare class MermanNodeEngine {
  readonly queueState: MermanQueueState;
  renderSvg(source: string, options?: RenderSvgOptions): Promise<string>;
  /** Synchronous rendering is intended only for explicit SSG build paths. */
  renderSvgSync(source: string): string;
  dispose(): Promise<void>;
}

export declare function createNodeEngine(
  options?: CreateNodeEngineOptions,
): Promise<MermanNodeEngine>;

export declare class MermanError extends Error {
  readonly code: string;
}

export declare class MermanOperationError extends MermanError {
  readonly status: number | null;
  readonly codeName: string | null;
  readonly kind: "generic" | "unknown-operation" | "missing-capability" | string;
  readonly capabilityId: string | null;
}

export declare class MermanQueueSaturatedError extends MermanError {
  readonly maxQueue: number;
}

export declare class MermanDisposedError extends MermanError {}
export declare class MermanLifecycleError extends MermanError {}
export declare class MermanMissingPlatformPackageError extends MermanError {
  readonly packageName: string;
  readonly target: string;
}
export declare class MermanUnsupportedTargetError extends MermanError {
  readonly platform: string;
  readonly arch: string;
  readonly libc: string | null;
}
