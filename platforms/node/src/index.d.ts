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
  /** Raw per-request options JSON merged by the shared binding contract. */
  optionsJson?: string;
}

export interface MermanOperationRequest {
  operationId: string;
  source: string;
  uri?: string | null;
  /** Raw per-request options JSON merged over the reusable engine baseline. */
  optionsJson?: string;
}

export interface MermanOperationResult {
  operation_id: string;
  media_type: string;
  data: string;
  metadata_json: string;
}

export interface MermanTextMeasurementCatalog {
  protocol_version: number;
  provider_ids: ["vendored"];
  [key: string]: unknown;
}

export interface MermanRuntimeResourceLimit {
  id: string;
  phase: string;
  description: string;
  overridable: boolean;
  hard_cap: boolean;
  [key: string]: unknown;
}

export interface MermanRuntimeResourceProfile {
  id: string;
  purpose: string;
  trust_assumption: string;
  recommended_binding_default: boolean;
  limits: Record<string, number | null>;
  [key: string]: unknown;
}

export interface MermanRuntimeCatalog {
  schema_version: 1;
  transport_api_version: 1;
  package_version: string;
  capabilities: {
    capability_ids: string[];
    output_ids: string[];
    operation_ids: string[];
    system_adapter_ids: string[];
    text_measurement: MermanTextMeasurementCatalog | null;
    [key: string]: unknown;
  };
  registry: {
    diagram_family_count: number;
    [key: string]: unknown;
  };
  resources: {
    general_binding_default_profile: string;
    cli_default_profile: string;
    limits: MermanRuntimeResourceLimit[];
    profiles: MermanRuntimeResourceProfile[];
    [key: string]: unknown;
  };
  [key: string]: unknown;
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
  readonly runtimeCatalog: MermanRuntimeCatalog;
  renderSvg(source: string, options?: RenderSvgOptions): Promise<string>;
  /** Synchronous rendering is intended only for explicit SSG build paths. */
  renderSvgSync(source: string, options?: Omit<RenderSvgOptions, "signal">): string;
  executeOperation(
    request: MermanOperationRequest,
    options?: Pick<RenderSvgOptions, "signal">,
  ): Promise<MermanOperationResult>;
  executeOperationSync(request: MermanOperationRequest): MermanOperationResult;
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
