import { isBindingErrorPayload } from "@mermanjs/web";

export interface ErrorProjection {
  readonly detail: string | null;
  readonly summary: string;
}

const MAX_DETAIL_LENGTH = 8_000;
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
  if (isBindingPayload(error)) {
    return {
      summary: nonEmpty(error.message, error.code_name),
      detail: serializeDetail({
        version: error.version,
        code: error.code,
        code_name: error.code_name,
        message: error.message,
      }),
    };
  }

  if (error instanceof Error) {
    return {
      summary: nonEmpty(error.message, error.name, "Unexpected error."),
      detail:
        "cause" in error && error.cause !== undefined
          ? serializeDetail({ cause: error.cause })
          : null,
    };
  }

  if (typeof error === "string") {
    return { summary: nonEmpty(error, "Unexpected error."), detail: null };
  }

  if (error && typeof error === "object") {
    const message = readProperty(error, "message");
    return {
      summary:
        typeof message === "string"
          ? nonEmpty(message, "Unexpected error.")
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

function isBindingPayload(
  error: unknown
): error is Parameters<typeof isBindingErrorPayload>[0] & {
  version: number;
  ok: false;
  code: number;
  code_name: string;
  message: string;
} {
  try {
    return isBindingErrorPayload(error);
  } catch {
    return false;
  }
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
