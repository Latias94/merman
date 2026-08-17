import { sha256 } from "@noble/hashes/sha2.js";
import {
  RealmProtocolError,
  type RealmEngineArtifact,
} from "./channel-protocol.ts";

export const EPHEMERAL_STORAGE_BUDGETS = Object.freeze({
  maxEntries: 64,
  maxKeyBytes: 1_024,
  maxValueBytes: 16 * 1_024,
  maxTotalBytes: 64 * 1_024,
});

const TEXT_ENCODER = new TextEncoder();

export async function verifyAndCreateRealmEngineModuleLoader<T extends object>(
  artifact: RealmEngineArtifact,
  validate: (module: Record<string, unknown>) => T
): Promise<() => Promise<T>> {
  const actual = await sha256Hex(new TextEncoder().encode(artifact.source));
  if (actual !== artifact.sha256) {
    throw new RealmProtocolError("Realm engine artifact digest is invalid.");
  }
  let modulePromise: Promise<T> | null = null;
  return () => {
    modulePromise ??= importEngineModule(artifact, validate).catch((error) => {
      modulePromise = null;
      throw error;
    });
    return modulePromise;
  };
}

/**
 * Computes SHA-256 with Web Crypto when available and a pure-JavaScript
 * fallback for HTTP development origins where SubtleCrypto is unavailable.
 */
export async function sha256Hex(
  bytes: Uint8Array,
  subtle: Pick<SubtleCrypto, "digest"> | null =
    globalThis.crypto?.subtle ?? null,
): Promise<string> {
  const digest = subtle
    ? new Uint8Array(await subtle.digest("SHA-256", bytes))
    : sha256(bytes);
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}

async function importEngineModule<T extends object>(
  artifact: RealmEngineArtifact,
  validate: (module: Record<string, unknown>) => T
): Promise<T> {
  installEphemeralStorageFacades();
  const url = URL.createObjectURL(
    new Blob([artifact.source], { type: "text/javascript" })
  );
  try {
    const module = (await import(/* @vite-ignore */ url)) as Record<
      string,
      unknown
    >;
    return validate(module);
  } finally {
    URL.revokeObjectURL(url);
  }
}

function installEphemeralStorageFacades(): void {
  const owner = globalThis as typeof globalThis & {
    __mermanEphemeralStorage?: boolean;
  };
  if (owner.__mermanEphemeralStorage) return;
  for (const name of ["localStorage", "sessionStorage"] as const) {
    try {
      Object.defineProperty(globalThis, name, {
        configurable: false,
        enumerable: false,
        writable: false,
        value: createEphemeralStorageFacade(),
      });
    } catch (error) {
      throw new RealmProtocolError(
        `Realm could not replace origin-backed ${name}: ${errorMessage(error)}`
      );
    }
  }
  Object.defineProperty(owner, "__mermanEphemeralStorage", {
    configurable: false,
    enumerable: false,
    writable: false,
    value: true,
  });
}

export function createEphemeralStorageFacade(): Storage {
  const values = new Map<string, string>();
  let totalBytes = 0;
  return Object.freeze({
    get length() {
      return values.size;
    },
    clear() {
      values.clear();
      totalBytes = 0;
    },
    getItem(key: string) {
      return values.get(String(key)) ?? null;
    },
    key(index: number) {
      return Number.isInteger(index) && index >= 0
        ? [...values.keys()][index] ?? null
        : null;
    },
    removeItem(key: string) {
      const normalizedKey = String(key);
      const previous = values.get(normalizedKey);
      if (previous === undefined) return;
      totalBytes -= storageEntryBytes(normalizedKey, previous);
      values.delete(normalizedKey);
    },
    setItem(key: string, value: string) {
      const normalizedKey = String(key);
      const normalizedValue = String(value);
      const keyBytes = utf8Bytes(normalizedKey);
      const valueBytes = utf8Bytes(normalizedValue);
      const previous = values.get(normalizedKey);
      const nextEntries = previous === undefined ? values.size + 1 : values.size;
      const nextTotal =
        totalBytes -
        (previous === undefined ? 0 : storageEntryBytes(normalizedKey, previous)) +
        keyBytes +
        valueBytes;
      if (
        nextEntries > EPHEMERAL_STORAGE_BUDGETS.maxEntries ||
        keyBytes > EPHEMERAL_STORAGE_BUDGETS.maxKeyBytes ||
        valueBytes > EPHEMERAL_STORAGE_BUDGETS.maxValueBytes ||
        nextTotal > EPHEMERAL_STORAGE_BUDGETS.maxTotalBytes
      ) {
        throw quotaExceeded();
      }
      values.set(normalizedKey, normalizedValue);
      totalBytes = nextTotal;
    },
  });
}

function storageEntryBytes(key: string, value: string): number {
  return utf8Bytes(key) + utf8Bytes(value);
}

function utf8Bytes(value: string): number {
  return TEXT_ENCODER.encode(value).byteLength;
}

function quotaExceeded(): Error {
  if (typeof DOMException !== "undefined") {
    return new DOMException(
      "Ephemeral realm storage budget exceeded.",
      "QuotaExceededError"
    );
  }
  const error = new Error("Ephemeral realm storage budget exceeded.");
  error.name = "QuotaExceededError";
  return error;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
