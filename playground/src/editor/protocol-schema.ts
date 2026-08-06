/**
 * Strict, deliberately small primitives shared by the editor protocol
 * projectors.  Keeping these helpers independent prevents request and query
 * result projectors from importing one another.
 */

export class EditorWorkerProtocolProjectionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "EditorWorkerProtocolProjectionError";
  }
}

export interface ObjectSchema {
  readonly allowed: ReadonlySet<string>;
  readonly required: readonly string[];
}

export function projectArray<T>(
  value: unknown,
  label: string,
  project: (item: unknown) => T,
): T[] {
  if (!Array.isArray(value)) {
    fail(`Editor ${label} must be an array.`);
  }
  return value.map(project);
}

export function projectStringArray(value: unknown, label: string): string[] {
  return projectArray(value, label, (item) => expectString(item, label));
}

export function hasDefinedOwn(
  record: Record<string, unknown>,
  key: string,
): boolean {
  return Object.hasOwn(record, key) && record[key] !== undefined;
}

export function optionalNullableProperty<Key extends string, Value>(
  record: Record<string, unknown>,
  key: Key,
  project: (value: unknown) => Value,
): { [Property in Key]?: Value | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  const value = record[key];
  return { [key]: value === null ? null : project(value) } as {
    [Property in Key]?: Value | null;
  };
}

export function optionalArrayProperty<Key extends string, Value>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
  project: (value: unknown) => Value,
): { [Property in Key]?: Value[] } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  return { [key]: projectArray(record[key], label, project) } as {
    [Property in Key]?: Value[];
  };
}

export function optionalBooleanProperty<Key extends string>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
): { [Property in Key]?: boolean } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  return { [key]: expectBoolean(record[key], label) } as {
    [Property in Key]?: boolean;
  };
}

export function optionalNullableStringProperty<Key extends string>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
): { [Property in Key]?: string | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  return { [key]: expectNullableString(record[key], label) } as {
    [Property in Key]?: string | null;
  };
}

export function optionalNullableNumberProperty<Key extends string>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
): { [Property in Key]?: number | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  const value = record[key];
  if (value !== null && typeof value !== "number") {
    fail(`Editor ${label} must be a number or null.`);
  }
  return { [key]: value } as { [Property in Key]?: number | null };
}

export function optionalNullableIntegerProperty<Key extends string>(
  record: Record<string, unknown>,
  key: Key,
  label: string,
): { [Property in Key]?: number | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  const value = record[key];
  return {
    [key]: value === null ? null : expectNonNegativeSafeInteger(value, label),
  } as { [Property in Key]?: number | null };
}

export function optionalNullableSetProperty<Key extends string, Value extends string>(
  record: Record<string, unknown>,
  key: Key,
  allowed: ReadonlySet<Value>,
  label: string,
): { [Property in Key]?: Value | null } {
  if (!Object.hasOwn(record, key) || record[key] === undefined) return {};
  const value = record[key];
  return {
    [key]: value === null ? null : expectSetValue(value, allowed, label),
  } as { [Property in Key]?: Value | null };
}

export function expectRecord(
  value: unknown,
  label: string,
): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail(`Editor ${label} must be an object.`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail(`Editor ${label} must be a plain object.`);
  }
  return value as Record<string, unknown>;
}

export function expectString(value: unknown, label: string): string {
  if (typeof value !== "string") {
    fail(`Editor ${label} must be a string.`);
  }
  return value;
}

export function expectNonEmptyString(value: unknown, label: string): string {
  const text = expectString(value, label);
  if (text.length === 0) {
    fail(`Editor ${label} must not be empty.`);
  }
  return text;
}

export function expectNonBlankString(value: unknown, label: string): string {
  const text = expectString(value, label);
  if (text.trim().length === 0) {
    fail(`Editor ${label} must not be blank.`);
  }
  return text;
}

export function expectNullableString(
  value: unknown,
  label: string,
): string | null {
  return value === null ? null : expectString(value, label);
}

export function expectBoolean(value: unknown, label: string): boolean {
  if (typeof value !== "boolean") {
    fail(`Editor ${label} must be a boolean.`);
  }
  return value;
}

export function expectRequestId(value: unknown): number {
  return expectPositiveSafeInteger(value, "request ID");
}

export function expectPositiveSafeInteger(
  value: unknown,
  label: string,
): number {
  if (!isPositiveSafeInteger(value)) {
    fail(`Editor ${label} must be a positive safe integer.`);
  }
  return value;
}

export function expectNonNegativeSafeInteger(
  value: unknown,
  label: string,
): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    fail(`Editor ${label} must be a non-negative safe integer.`);
  }
  return value as number;
}

export function isPositiveSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0;
}

export function expectSetValue<Value extends string>(
  value: unknown,
  allowed: ReadonlySet<Value>,
  label: string,
): Value {
  if (typeof value !== "string" || !allowed.has(value as Value)) {
    fail(`Editor ${label} is invalid.`);
  }
  return value as Value;
}

export function schema(required: readonly string[]): ObjectSchema {
  return Object.freeze({
    allowed: new Set(required),
    required,
  });
}

export function assertSchema(
  value: Record<string, unknown>,
  expected: ObjectSchema,
  label: string,
): void {
  const keys = Object.keys(value);
  if (
    keys.some((key) => !expected.allowed.has(key)) ||
    expected.required.some((key) => !Object.hasOwn(value, key))
  ) {
    fail(`Editor ${label} contains unexpected or missing fields.`);
  }
}

export function fail(message: string): never {
  throw new EditorWorkerProtocolProjectionError(message);
}
