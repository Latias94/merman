import {
  BUNDLED_ANALYSIS_CONFIG,
  analysisConfigValues,
  type AnalysisConfigClientObjectField,
  type AnalysisConfigClientSetting,
  type AnalysisConfigClientSettingNormalization,
  type NegotiatedAnalysisConfig,
} from "./analysis-config-contract.js";

export interface LintRuleSeverityOverride {
  rule_id: AnalysisConfigurableRuleId;
  severity: AnalysisDiagnosticSeverity;
}

export type AnalysisLintProfile = string;
export type AnalysisDiagnosticSeverity = string;
export type AnalysisConfigurableRuleId = string;

export interface AnalysisSettings {
  fixed_today?: string;
  fixed_local_offset_minutes?: number;
  site_config?: Record<string, unknown>;
  resources?: {
    limits?: {
      max_source_bytes?: number;
      max_document_diagrams?: number;
    };
  };
  lint?: {
    profile?: AnalysisLintProfile;
    enable_rules?: AnalysisConfigurableRuleId[];
    disable_rules?: AnalysisConfigurableRuleId[];
    rule_severities?: LintRuleSeverityOverride[];
  };
}

export interface ProjectedAnalysisSettings {
  settings: AnalysisSettings;
  unsupportedRuleIds: string[];
}

export type RawAnalysisSettings = Readonly<Record<string, unknown>>;

export function normalizeAnalysisSettings(
  raw: RawAnalysisSettings,
  contract: NegotiatedAnalysisConfig,
): AnalysisSettings {
  return normalizeSettings(raw, contract);
}

export function bootstrapAnalysisSettings(raw: RawAnalysisSettings): AnalysisSettings {
  return normalizeSettings(raw, BUNDLED_ANALYSIS_CONFIG, "snapshot_affecting");
}

export function projectAnalysisSettings(
  settings: AnalysisSettings,
  configurableRuleIds: readonly string[],
): ProjectedAnalysisSettings {
  const supported = new Set(configurableRuleIds);
  const unsupported = new Set<string>();
  const retainRuleId = (ruleId: string): boolean => {
    if (supported.has(ruleId)) {
      return true;
    }
    unsupported.add(ruleId);
    return false;
  };
  const lint = settings.lint
    ? compactObject({
        ...settings.lint,
        profile: settings.lint.profile,
        enable_rules: settings.lint.enable_rules?.filter(retainRuleId),
        disable_rules: settings.lint.disable_rules?.filter(retainRuleId),
        rule_severities: settings.lint.rule_severities?.filter((entry) =>
          retainRuleId(entry.rule_id)
        ),
      })
    : undefined;

  return {
    settings: compactObject({
      ...settings,
      lint: lint && Object.keys(lint).length > 0 ? lint : undefined,
    }),
    unsupportedRuleIds: [...unsupported],
  };
}

function normalizeSettings(
  raw: RawAnalysisSettings,
  contract: NegotiatedAnalysisConfig,
  requiredScope?: AnalysisConfigClientSetting["changeScope"],
): AnalysisSettings {
  const normalized = new Map<string, unknown>();
  for (const setting of contract.settings) {
    if (requiredScope && setting.changeScope !== requiredScope) {
      continue;
    }
    const value = normalizeSettingValue(raw[setting.path], setting.normalization, contract);
    if (value !== undefined) {
      normalized.set(setting.path, value);
    }
  }

  for (const setting of contract.settings) {
    if (!normalized.has(setting.path)) {
      continue;
    }
    const value = normalized.get(setting.path);
    if (!satisfiesRuntimeConstraints(value, setting, normalized)) {
      normalized.delete(setting.path);
    }
  }

  const result: Record<string, unknown> = {};
  for (const [path, value] of normalized) {
    setPath(result, path, value);
  }
  return result as AnalysisSettings;
}

function normalizeSettingValue(
  value: unknown,
  normalization: AnalysisConfigClientSettingNormalization,
  contract: NegotiatedAnalysisConfig,
): unknown {
  switch (normalization.kind) {
    case "string": {
      const normalized = normalizeOptionalString(value);
      if (!normalized) {
        return undefined;
      }
      if (normalization.matcher) {
        normalization.matcher.lastIndex = 0;
        if (!normalization.matcher.test(normalized)) {
          return undefined;
        }
      }
      if (
        normalization.values &&
        !analysisConfigValues(contract, normalization.values).includes(normalized)
      ) {
        return undefined;
      }
      return normalized;
    }
    case "integer":
      return normalizeIntegerInRange(value, normalization.minimum, normalization.maximum);
    case "object":
      return normalizePlainObject(value);
    case "rule_id_list": {
      const ruleIds = sanitizeRuleIds(value);
      return ruleIds.length > 0 ? ruleIds : undefined;
    }
    case "rule_severity_overrides": {
      const overrides = sanitizeObjectList(value, normalization.fields, contract);
      return overrides.length > 0 ? overrides : undefined;
    }
  }
}

function satisfiesRuntimeConstraints(
  value: unknown,
  setting: AnalysisConfigClientSetting,
  normalized: ReadonlyMap<string, unknown>,
): boolean {
  let civilDate: CivilDate | undefined;
  for (const constraint of setting.runtimeConstraints) {
    switch (constraint.kind) {
      case "canonical_civil_date":
        if (typeof value !== "string") {
          return false;
        }
        civilDate = parseCanonicalCivilDate(value);
        if (!civilDate) {
          return false;
        }
        break;
      case "representable_local_midnight": {
        if (typeof value !== "string") {
          return false;
        }
        civilDate ??= parseCanonicalCivilDate(value);
        if (!civilDate) {
          return false;
        }
        const offset = normalized.get(constraint.offsetSettingPath);
        if (offset !== undefined && typeof offset !== "number") {
          return false;
        }
        if (!isRepresentableLocalMidnight(civilDate, offset ?? 0)) {
          return false;
        }
        break;
      }
    }
  }
  return true;
}

interface CivilDate {
  year: number;
  month: number;
  day: number;
}

function parseCanonicalCivilDate(value: string): CivilDate | undefined {
  const match = /^([+-]?\d+)-(\d{2})-(\d{2})$/u.exec(value);
  if (!match) {
    return undefined;
  }
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (
    !Number.isInteger(year) ||
    year < -2_147_483_648 ||
    year > 2_147_483_647 ||
    month < 1 ||
    month > 12 ||
    day < 1 ||
    day > daysInMonth(year, month)
  ) {
    return undefined;
  }
  const canonicalYear = year >= 0 && year <= 9999
    ? String(year).padStart(4, "0")
    : year > 9999
    ? `+${year}`
    : `-${String(Math.abs(year)).padStart(4, "0")}`;
  return value === `${canonicalYear}-${match[2]}-${match[3]}`
    ? { year, month, day }
    : undefined;
}

function daysInMonth(year: number, month: number): number {
  switch (month) {
    case 2:
      return isLeapYear(year) ? 29 : 28;
    case 4:
    case 6:
    case 9:
    case 11:
      return 30;
    default:
      return 31;
  }
}

function isLeapYear(year: number): boolean {
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
}

function isRepresentableLocalMidnight(date: CivilDate, offsetMinutes: number): boolean {
  let year = BigInt(date.year);
  if (date.month <= 2) {
    year -= 1n;
  }
  const era = floorDiv(year, 400n);
  const yearOfEra = year - era * 400n;
  const monthPrime = BigInt(date.month + (date.month > 2 ? -3 : 9));
  const dayOfYear = (153n * monthPrime + 2n) / 5n + BigInt(date.day) - 1n;
  const dayOfEra = yearOfEra * 365n + yearOfEra / 4n - yearOfEra / 100n + dayOfYear;
  const daysSinceUnixEpoch = era * 146_097n + dayOfEra - 719_468n;
  const unixMillis = daysSinceUnixEpoch * 86_400_000n - BigInt(offsetMinutes) * 60_000n;
  return unixMillis >= -9_223_372_036_854_775_808n &&
    unixMillis <= 9_223_372_036_854_775_807n;
}

function floorDiv(dividend: bigint, divisor: bigint): bigint {
  const quotient = dividend / divisor;
  const remainder = dividend % divisor;
  return remainder < 0n ? quotient - 1n : quotient;
}

function normalizePlainObject(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const record = value as Record<string, unknown>;
  return Object.keys(record).length > 0 ? record : undefined;
}

function sanitizeRuleIds(value: unknown): AnalysisConfigurableRuleId[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.flatMap((entry) => {
    const normalized = normalizeOptionalString(entry);
    return normalized ? [normalized] : [];
  });
}

function sanitizeObjectList(
  value: unknown,
  fields: readonly AnalysisConfigClientObjectField[],
  contract: NegotiatedAnalysisConfig,
): Record<string, unknown>[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.flatMap((entry) => {
    if (!entry || typeof entry !== "object") {
      return [];
    }
    const source = entry as Record<string, unknown>;
    const normalized: Record<string, unknown> = {};
    for (const field of fields) {
      const fieldValue = normalizeObjectFieldValue(
        source[field.name],
        field.normalization,
        contract,
      );
      if (fieldValue === undefined) {
        if (field.required) {
          return [];
        }
        continue;
      }
      normalized[field.name] = fieldValue;
    }
    return Object.keys(normalized).length > 0 ? [normalized] : [];
  });
}

function normalizeObjectFieldValue(
  value: unknown,
  normalization: AnalysisConfigClientSettingNormalization,
  contract: NegotiatedAnalysisConfig,
): unknown {
  if (
    normalization.kind === "string" &&
    normalization.values === "configurable_rule_ids"
  ) {
    const lexicalNormalization = { ...normalization, values: undefined };
    return normalizeSettingValue(value, lexicalNormalization, contract);
  }
  return normalizeSettingValue(value, normalization, contract);
}

function normalizeOptionalString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 && value.trim() === value
    ? value
    : undefined;
}

function normalizeIntegerInRange(
  value: unknown,
  minimum: number,
  maximum: number,
): number | undefined {
  return typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= minimum &&
    value <= maximum
    ? value
    : undefined;
}

function setPath(root: Record<string, unknown>, path: string, value: unknown): void {
  const segments = path.split(".");
  let cursor = root;
  for (const segment of segments.slice(0, -1)) {
    const existing = cursor[segment];
    if (existing && typeof existing === "object" && !Array.isArray(existing)) {
      cursor = existing as Record<string, unknown>;
    } else {
      const child: Record<string, unknown> = {};
      cursor[segment] = child;
      cursor = child;
    }
  }
  const leaf = segments.at(-1);
  if (leaf) {
    cursor[leaf] = value;
  }
}

function compactObject<T extends object>(value: T): T {
  const entries = Object.entries(value).filter(([, fieldValue]) => fieldValue !== undefined);
  return Object.fromEntries(entries) as T;
}
