export interface LintRuleSeverityOverride {
  rule_id: AnalysisConfigurableRuleId;
  severity: AnalysisDiagnosticSeverity;
}

export const ANALYSIS_LINT_PROFILES = ["core", "recommended", "strict"] as const;
export const ANALYSIS_DIAGNOSTIC_SEVERITIES = [
  "error",
  "warning",
  "info",
  "hint",
] as const;
export const ANALYSIS_CONFIGURABLE_RULE_IDS = [
  "merman.authoring.config.prefer_init_directive",
  "merman.authoring.config.prefer_frontmatter_config",
  "merman.compatibility.config.deprecated_flowchart_html_labels",
  "merman.compatibility.config.deprecated_external_diagram_loading",
  "merman.parse.no_diagram",
  "merman.parse.diagram_parse",
  "merman.compatibility.unsupported_diagram",
  "merman.parse.recovered_editor_facts",
  "merman.config.malformed_front_matter",
  "merman.config.invalid_directive_json",
  "merman.config.invalid_front_matter_yaml",
  "merman.config.invalid_theme_color",
  "merman.block.width_exceeds_columns",
  "merman.authoring.flowchart.explicit_direction",
  "merman.semantic.flowchart.unknown_style_target",
  "merman.git_graph.duplicate_commit_id",
] as const;

export type AnalysisLintProfile = (typeof ANALYSIS_LINT_PROFILES)[number];
export type AnalysisDiagnosticSeverity = (typeof ANALYSIS_DIAGNOSTIC_SEVERITIES)[number];
export type AnalysisConfigurableRuleId = (typeof ANALYSIS_CONFIGURABLE_RULE_IDS)[number];

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

export interface RawAnalysisSettings {
  fixedToday: unknown;
  fixedLocalOffsetMinutes: unknown;
  siteConfig: unknown;
  maxSourceBytes: unknown;
  maxDocumentDiagrams: unknown;
  lintProfile: string;
  enableRules: unknown[];
  disableRules: unknown[];
  ruleSeverities: unknown[];
}

export function normalizeAnalysisSettings(raw: RawAnalysisSettings): AnalysisSettings {
  const fixedToday = normalizeOptionalIsoDateString(raw.fixedToday);
  const fixedLocalOffsetMinutes = normalizeIntegerInRange(
    raw.fixedLocalOffsetMinutes,
    -1439,
    1439,
  );
  const siteConfig = normalizePlainObject(raw.siteConfig);
  const maxSourceBytes = normalizeIntegerInRange(raw.maxSourceBytes, 1, 0xffff_ffff);
  const maxDocumentDiagrams = normalizeIntegerInRange(
    raw.maxDocumentDiagrams,
    0,
    0xffff_ffff,
  );
  const lintProfile = normalizeLintProfile(raw.lintProfile);
  const enableRules = sanitizeRuleIds(raw.enableRules);
  const disableRules = sanitizeRuleIds(raw.disableRules);
  const ruleSeverities = sanitizeRuleSeverities(raw.ruleSeverities);

  return compactObject<AnalysisSettings>({
    fixed_today: fixedToday,
    fixed_local_offset_minutes: fixedLocalOffsetMinutes,
    site_config: siteConfig,
    resources: maxSourceBytes !== undefined || maxDocumentDiagrams !== undefined
      ? {
          limits: compactObject({
            max_source_bytes: maxSourceBytes,
            max_document_diagrams: maxDocumentDiagrams,
          }),
        }
      : undefined,
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

function sanitizeRuleIds(value: unknown[] | undefined): AnalysisConfigurableRuleId[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const ruleIds = new Set<string>(ANALYSIS_CONFIGURABLE_RULE_IDS);
  return value
    .filter((entry): entry is string => typeof entry === "string")
    .map((entry) => entry.trim())
    .filter((entry): entry is AnalysisConfigurableRuleId => ruleIds.has(entry));
}

function sanitizeRuleSeverities(value: unknown[] | undefined): LintRuleSeverityOverride[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const severities = new Set<string>(ANALYSIS_DIAGNOSTIC_SEVERITIES);
  const ruleIds = new Set<string>(ANALYSIS_CONFIGURABLE_RULE_IDS);
  return value.flatMap((entry) => {
    if (!entry || typeof entry !== "object") {
      return [];
    }
    const ruleId = normalizeOptionalString((entry as Record<string, unknown>).rule_id);
    const severity = normalizeOptionalString((entry as Record<string, unknown>).severity);
    if (!ruleId || !ruleIds.has(ruleId) || !severity || !severities.has(severity)) {
      return [];
    }
    return [
      {
        rule_id: ruleId as AnalysisConfigurableRuleId,
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
  const match = /^([+-]?\d{4,10})-(\d{2})-(\d{2})$/u.exec(normalized);
  if (!match) {
    return undefined;
  }
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (!Number.isInteger(year) || year < -2147483648 || year > 2147483647) {
    return undefined;
  }
  const canonicalYear = year >= 0 && year <= 9999
    ? year.toString().padStart(4, "0")
    : year >= 10000
    ? `+${year}`
    : `-${Math.abs(year).toString().padStart(4, "0")}`;
  if (`${canonicalYear}-${match[2]}-${match[3]}` !== normalized) {
    return undefined;
  }
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

function normalizeLintProfile(
  value: string,
): AnalysisLintProfile | undefined {
  return ANALYSIS_LINT_PROFILES.find((profile) => profile === value);
}

function compactObject<T extends object>(value: T): T {
  const entries = Object.entries(value).filter(([, fieldValue]) => fieldValue !== undefined);
  return Object.fromEntries(entries) as T;
}
