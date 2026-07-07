export interface LintRuleSeverityOverride {
  rule_id: string;
  severity: "error" | "warning" | "info" | "hint";
}

export interface AnalysisSettings {
  fixed_today?: string;
  fixed_local_offset_minutes?: number;
  site_config?: Record<string, unknown>;
  parse?: {
    suppress_errors?: boolean;
  };
  resources?: {
    max_source_bytes?: number;
  };
  lint?: {
    profile?: "core" | "recommended" | "strict";
    enable_rules?: string[];
    disable_rules?: string[];
    rule_severities?: LintRuleSeverityOverride[];
  };
}

export interface RawAnalysisSettings {
  fixedToday: unknown;
  fixedLocalOffsetMinutes: unknown;
  siteConfig: unknown;
  suppressErrors: boolean;
  maxSourceBytes: unknown;
  lintProfile: string;
  enableRules: unknown[];
  disableRules: unknown[];
  ruleSeverities: unknown[];
}

type LintProfile = "core" | "recommended" | "strict";

export function normalizeAnalysisSettings(raw: RawAnalysisSettings): AnalysisSettings {
  const fixedToday = normalizeOptionalIsoDateString(raw.fixedToday);
  const fixedLocalOffsetMinutes = normalizeIntegerInRange(
    raw.fixedLocalOffsetMinutes,
    -1439,
    1439,
  );
  const siteConfig = normalizePlainObject(raw.siteConfig);
  const maxSourceBytes = normalizePositiveInteger(raw.maxSourceBytes);
  const lintProfile = normalizeLintProfile(raw.lintProfile);
  const enableRules = sanitizeStringArray(raw.enableRules);
  const disableRules = sanitizeStringArray(raw.disableRules);
  const ruleSeverities = sanitizeRuleSeverities(raw.ruleSeverities);

  return compactObject<AnalysisSettings>({
    fixed_today: fixedToday,
    fixed_local_offset_minutes: fixedLocalOffsetMinutes,
    site_config: siteConfig,
    parse: raw.suppressErrors ? { suppress_errors: true } : undefined,
    resources: maxSourceBytes ? { max_source_bytes: maxSourceBytes } : undefined,
    lint:
      lintProfile || enableRules.length || disableRules.length || ruleSeverities.length
        ? compactObject({
            profile: lintProfile,
            enable_rules: enableRules.length ? enableRules : undefined,
            disable_rules: disableRules.length ? disableRules : undefined,
            rule_severities: ruleSeverities.length ? ruleSeverities : undefined,
          })
        : undefined,
  });
}

function normalizePlainObject(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  const record = value as Record<string, unknown>;
  return Object.keys(record).length > 0 ? record : undefined;
}

function sanitizeStringArray(value: unknown[] | undefined): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((entry): entry is string => typeof entry === "string")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

function sanitizeRuleSeverities(value: unknown[] | undefined): LintRuleSeverityOverride[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const severities = new Set(["error", "warning", "info", "hint"]);
  return value.flatMap((entry) => {
    if (!entry || typeof entry !== "object") {
      return [];
    }
    const ruleId = normalizeOptionalString((entry as Record<string, unknown>).rule_id);
    const severity = normalizeOptionalString((entry as Record<string, unknown>).severity);
    if (!ruleId || !severity || !severities.has(severity)) {
      return [];
    }
    return [
      {
        rule_id: ruleId,
        severity: severity as LintRuleSeverityOverride["severity"],
      },
    ];
  });
}

function normalizeOptionalString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
}

function normalizeOptionalIsoDateString(value: unknown): string | undefined {
  const normalized = normalizeOptionalString(value);
  if (!normalized) {
    return undefined;
  }
  const match = /^(\d{4})-(\d{2})-(\d{2})$/u.exec(normalized);
  if (!match) {
    return undefined;
  }
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (month < 1 || month > 12) {
    return undefined;
  }
  const maxDay = daysInMonth(year, month);
  return day >= 1 && day <= maxDay ? normalized : undefined;
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

function normalizeIntegerInRange(
  value: unknown,
  minimum: number,
  maximum: number,
): number | undefined {
  return typeof value === "number" &&
    Number.isInteger(value) &&
    value >= minimum &&
    value <= maximum
    ? value
    : undefined;
}

function normalizePositiveInteger(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : undefined;
}

function normalizeLintProfile(
  value: string,
): LintProfile | undefined {
  switch (value) {
    case "core":
    case "recommended":
    case "strict":
      return value;
    default:
      return undefined;
  }
}

function compactObject<T extends object>(value: T): T {
  const entries = Object.entries(value).filter(([, fieldValue]) => fieldValue !== undefined);
  return Object.fromEntries(entries) as T;
}
