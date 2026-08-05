import {
  REALM_BUDGETS,
  RealmProtocolError,
  RealmTimeoutError,
  utf8ByteLength,
  type RealmEngineArtifact,
  type RealmEngineArtifactId,
} from "./channel-protocol.ts";
import { isRealmEngineArtifactId } from "./generated/opaque-realm-plan.generated.ts";

interface GeneratedEngineManifest {
  readonly bytes: unknown;
  readonly id: unknown;
  readonly schemaVersion: unknown;
  readonly sha256: unknown;
}

export interface StaticEngineArtifactEnvironment {
  readonly fetch: (
    input: URL,
    init: Readonly<{ cache: "default"; signal: AbortSignal }>
  ) => Promise<Response>;
  readonly location: Readonly<{ href: string; origin: string }>;
}

export interface StaticEngineArtifactRequest {
  readonly manifest: GeneratedEngineManifest;
  readonly resourceUrl: string | null;
  readonly signal?: AbortSignal;
  readonly sourceUrl: string;
  readonly timeoutMs?: number;
}

const ENGINE_MANIFEST_KEYS = Object.freeze([
  "bytes",
  "id",
  "schemaVersion",
  "sha256",
]);

export async function createStaticRealmEngineArtifact(
  request: StaticEngineArtifactRequest,
  environment: StaticEngineArtifactEnvironment = browserEnvironment()
): Promise<RealmEngineArtifact> {
  const identity = validateGeneratedEngineManifest(request.manifest);
  const source = await fetchEngineArtifactSource(request, environment);
  if (utf8ByteLength(source) !== identity.bytes) {
    throw new RealmProtocolError("Realm engine artifact byte length is invalid.");
  }
  return Object.freeze({ ...identity, resourceUrl: request.resourceUrl, source });
}

function browserEnvironment(): StaticEngineArtifactEnvironment {
  return {
    fetch: (input, init) => fetch(input, init),
    location: window.location,
  };
}

function validateGeneratedEngineManifest(
  manifest: GeneratedEngineManifest
): Readonly<{
  bytes: number;
  id: RealmEngineArtifactId;
  schemaVersion: 1;
  sha256: string;
}> {
  const keys = Object.keys(manifest).sort();
  if (
    keys.length !== ENGINE_MANIFEST_KEYS.length ||
    !ENGINE_MANIFEST_KEYS.every((name, index) => keys[index] === name) ||
    manifest.schemaVersion !== 1 ||
    !isRealmEngineArtifactId(manifest.id) ||
    typeof manifest.bytes !== "number" ||
    !Number.isSafeInteger(manifest.bytes) ||
    manifest.bytes <= 0 ||
    manifest.bytes > REALM_BUDGETS.engineArtifactBytes ||
    typeof manifest.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/u.test(manifest.sha256)
  ) {
    throw new RealmProtocolError("Realm engine artifact manifest is invalid.");
  }
  return Object.freeze({
    bytes: manifest.bytes,
    id: manifest.id,
    schemaVersion: 1,
    sha256: manifest.sha256,
  });
}

async function fetchEngineArtifactSource(
  request: StaticEngineArtifactRequest,
  environment: StaticEngineArtifactEnvironment
): Promise<string> {
  const timeoutMs = request.timeoutMs ?? REALM_BUDGETS.stageTimeoutMs;
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0) {
    throw new RealmProtocolError("Realm engine artifact timeout is invalid.");
  }
  const url = new URL(request.sourceUrl, environment.location.href);
  if (url.origin !== environment.location.origin) {
    throw new RealmProtocolError("Realm engine artifact must be same-origin.");
  }
  if (request.signal?.aborted) {
    throw request.signal.reason instanceof Error
      ? request.signal.reason
      : new RealmProtocolError("Realm engine artifact request was aborted.");
  }
  const controller = new AbortController();
  const abortFromCaller = () => controller.abort(request.signal?.reason);
  request.signal?.addEventListener("abort", abortFromCaller, { once: true });
  if (request.signal?.aborted) abortFromCaller();
  const timeout = setTimeout(
    () =>
      controller.abort(
        new RealmTimeoutError("Realm engine artifact request timed out.")
      ),
    timeoutMs
  );
  try {
    const response = await environment.fetch(url, {
      cache: "default",
      signal: controller.signal,
    });
    if (!response.ok) {
      const error = new RealmProtocolError(
        `Realm engine artifact request failed with HTTP ${response.status}.`
      );
      const failure = abortEngineArtifactRequest(controller, error);
      cancelEngineResponseBody(response, failure);
      throw failure;
    }
    return await readBoundedEngineSource(response, controller);
  } catch (error) {
    if (controller.signal.aborted) {
      throw engineArtifactAbortError(controller.signal);
    }
    throw abortEngineArtifactRequest(controller, error);
  } finally {
    clearTimeout(timeout);
    request.signal?.removeEventListener("abort", abortFromCaller);
  }
}

async function readBoundedEngineSource(
  response: Response,
  controller: AbortController
): Promise<string> {
  const { signal } = controller;
  const contentLength = response.headers.get("content-length");
  if (contentLength !== null) {
    const declared = Number(contentLength);
    if (
      !Number.isSafeInteger(declared) ||
      declared <= 0 ||
      declared > REALM_BUDGETS.engineArtifactBytes
    ) {
      const error = new RealmProtocolError(
        "Realm engine artifact exceeds its byte budget."
      );
      const failure = abortEngineArtifactRequest(controller, error);
      cancelEngineResponseBody(response, failure);
      throw failure;
    }
  }
  const reader = response.body?.getReader();
  if (!reader) {
    throw new RealmProtocolError("Realm engine artifact response has no body.");
  }
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const chunks: string[] = [];
  let totalBytes = 0;
  try {
    while (true) {
      const next = await readEngineChunk(reader, signal);
      if (next.done) break;
      totalBytes += next.value.byteLength;
      if (totalBytes > REALM_BUDGETS.engineArtifactBytes) {
        throw new RealmProtocolError(
          "Realm engine artifact exceeds its byte budget."
        );
      }
      chunks.push(decodeEngineChunk(decoder, next.value, true));
    }
    chunks.push(decodeEngineChunk(decoder, undefined, false));
  } catch (error) {
    const failure = abortEngineArtifactRequest(controller, error);
    cancelEngineReader(reader, failure);
    throw failure;
  } finally {
    if (!signal.aborted) {
      reader.releaseLock();
    } else {
      try {
        reader.releaseLock();
      } catch {
        // An aborted read may remain pending until the underlying stream observes cancel().
      }
    }
  }
  return chunks.join("");
}

function readEngineChunk(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  signal: AbortSignal
): ReturnType<ReadableStreamDefaultReader<Uint8Array>["read"]> {
  if (signal.aborted) {
    const error = engineArtifactAbortError(signal);
    cancelEngineReader(reader, error);
    return Promise.reject(error);
  }
  return new Promise((resolve, reject) => {
    let settled = false;
    const settle = (callback: () => void) => {
      if (settled) return;
      settled = true;
      signal.removeEventListener("abort", onAbort);
      callback();
    };
    const onAbort = () => {
      const error = engineArtifactAbortError(signal);
      cancelEngineReader(reader, error);
      settle(() => reject(error));
    };
    signal.addEventListener("abort", onAbort, { once: true });
    void reader.read().then(
      (result) => settle(() => resolve(result)),
      (error: unknown) => settle(() => reject(error))
    );
    if (signal.aborted) onAbort();
  });
}

function cancelEngineReader(
  reader: ReadableStreamDefaultReader<Uint8Array>,
  reason: Error
): void {
  try {
    void reader.cancel(reason).catch(() => {});
  } catch {
    // The stream may already be errored or canceled by the underlying fetch.
  }
}

function cancelEngineResponseBody(response: Response, reason: Error): void {
  try {
    void response.body?.cancel(reason).catch(() => {});
  } catch {
    // The stream may already be errored or canceled by the underlying fetch.
  }
}

function abortEngineArtifactRequest(
  controller: AbortController,
  error: unknown
): Error {
  if (!controller.signal.aborted) {
    controller.abort(engineArtifactFailureReason(error));
  }
  return engineArtifactAbortError(controller.signal);
}

function engineArtifactFailureReason(error: unknown): Error {
  return error instanceof Error
    ? error
    : new RealmProtocolError("Realm engine artifact response failed.");
}

function engineArtifactAbortError(signal: AbortSignal): Error {
  return signal.reason instanceof Error
    ? signal.reason
    : new RealmProtocolError("Realm engine artifact request was aborted.");
}

function decodeEngineChunk(
  decoder: TextDecoder,
  bytes: Uint8Array | undefined,
  stream: boolean
): string {
  try {
    return decoder.decode(bytes, { stream });
  } catch {
    throw new RealmProtocolError("Realm engine artifact is not valid UTF-8.");
  }
}
