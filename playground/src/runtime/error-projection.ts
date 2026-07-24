export interface ErrorProjection {
  readonly detail: string | null;
  readonly summary: string;
}

export interface BindingErrorPayload {
  readonly code: number;
  readonly code_name: string;
  readonly kind: string;
  readonly capability_id: string | null;
  readonly message: string;
  readonly ok: false;
  readonly version: number;
}

const MAX_DETAIL_LENGTH = 8_000;
const MAX_SUMMARY_LENGTH = 8_000;
const MAX_DEPTH = 6;
const MAX_ENTRIES = 50;

export function projectError(error: unknown): ErrorProjection {
  try {
    return projectErrorValue(error);
  } catch {
    return {
      summary: "Unexpected error.",
      detail: '"[unreadable error]"',
    };
  }
}

function projectErrorValue(error: unknown): ErrorProjection {
  if (isBindingErrorPayload(error)) {
    return {
      summary: boundedSummary(nonEmpty(error.message, error.code_name)),
      detail: serializeDetail({
        version: error.version,
        code: error.code,
        code_name: error.code_name,
        kind: error.kind,
        capability_id: error.capability_id,
        message: error.message,
      }),
    };
  }

  if (error instanceof Error) {
    return {
      summary: boundedSummary(
        nonEmpty(error.message, error.name, "Unexpected error.")
      ),
      detail: projectNativeErrorDetail(error),
    };
  }

  if (typeof error === "string") {
    return {
      summary: boundedSummary(nonEmpty(error, "Unexpected error.")),
      detail: null,
    };
  }

  if (error && typeof error === "object") {
    const message = readProperty(error, "message");
    return {
      summary:
        typeof message === "string"
          ? boundedSummary(nonEmpty(message, "Unexpected error."))
          : "Unexpected error.",
      detail: serializeDetail(error),
    };
  }

  if (
    typeof error === "number" ||
    typeof error === "bigint" ||
    typeof error === "boolean"
  ) {
    return { summary: String(error), detail: null };
  }

  return { summary: "Unexpected error.", detail: null };
}

function nonEmpty(...candidates: string[]): string {
  return (
    candidates.find((candidate) => candidate.trim().length > 0)?.trim() ??
    "Unexpected error."
  );
}

function boundedSummary(summary: string): string {
  return summary.length <= MAX_SUMMARY_LENGTH
    ? summary
    : `${summary.slice(0, MAX_SUMMARY_LENGTH)}\n... [truncated]`;
}

export function isBindingErrorPayload(
  error: unknown
): error is BindingErrorPayload {
  try {
    if (!error || typeof error !== "object") return false;
    return (
      readProperty(error, "ok") === false &&
      typeof readProperty(error, "version") === "number" &&
      typeof readProperty(error, "code") === "number" &&
      typeof readProperty(error, "code_name") === "string" &&
      typeof readProperty(error, "kind") === "string" &&
      (readProperty(error, "capability_id") === null ||
        typeof readProperty(error, "capability_id") === "string") &&
      typeof readProperty(error, "message") === "string"
    );
  } catch {
    return false;
  }
}

function projectNativeErrorDetail(error: Error): string | null {
  const detail: Record<string, unknown> = {};
  let keys: string[];
  try {
    keys = Reflect.ownKeys(error)
      .filter((key): key is string => typeof key === "string")
      .filter((key) => key !== "message" && key !== "name" && key !== "stack")
      .sort();
  } catch {
    return '"[unreadable error]"';
  }

  for (const key of keys) {
    detail[key] = readProperty(error, key);
  }
  const name = readProperty(error, "name");
  if (typeof name === "string" && name && name !== "Error") {
    detail.name = name;
  }
  return Object.keys(detail).length > 0 ? serializeDetail(detail) : null;
}

function readProperty(value: object, key: string): unknown {
  try {
    return Reflect.get(value, key);
  } catch {
    return undefined;
  }
}

function serializeDetail(value: unknown): string {
  const seen = new WeakSet<object>();
  const normalized = normalizeValue(value, 0, seen);
  const detail = JSON.stringify(normalized, null, 2);
  if (detail.length <= MAX_DETAIL_LENGTH) return detail;
  return `${detail.slice(0, MAX_DETAIL_LENGTH)}\n... [truncated]`;
}

function normalizeValue(
  value: unknown,
  depth: number,
  seen: WeakSet<object>
): unknown {
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    return value;
  }
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : String(value);
  }
  if (typeof value === "bigint") return `${value}n`;
  if (typeof value === "undefined") return "[undefined]";
  if (typeof value === "symbol") return value.description ?? "[symbol]";
  if (typeof value === "function") return `[function ${value.name || "anonymous"}]`;
  if (depth >= MAX_DEPTH) return "[max depth]";
  if (seen.has(value)) return "[circular]";

  seen.add(value);
  if (Array.isArray(value)) {
    const items = value
      .slice(0, MAX_ENTRIES)
      .map((item) => normalizeValue(item, depth + 1, seen));
    if (value.length > MAX_ENTRIES) items.push(`[${value.length - MAX_ENTRIES} more items]`);
    return items;
  }

  const output: Record<string, unknown> = {};
  let keys: string[];
  try {
    keys = Reflect.ownKeys(value)
      .filter((key): key is string => typeof key === "string")
      .sort();
  } catch {
    return "[unreadable object]";
  }
  for (const key of keys.slice(0, MAX_ENTRIES)) {
    try {
      output[key] = normalizeValue(Reflect.get(value, key), depth + 1, seen);
    } catch {
      output[key] = "[unreadable]";
    }
  }
  if (keys.length > MAX_ENTRIES) {
    output["[truncated]"] = `${keys.length - MAX_ENTRIES} more properties`;
  }
  return output;
}
