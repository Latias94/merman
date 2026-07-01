import * as vscode from "vscode";

import type { LanguageIntelligenceSettings } from "./language-intelligence.js";

export type TraceSetting = "off" | "messages" | "verbose";

export interface MermanServerSettings {
  path: string;
  args: string[];
  useCargoRun: boolean;
  cargoArgs: string[];
}

export interface MermanCliSettings {
  path: string;
  useCargoRun: boolean;
  cargoArgs: string[];
}

export interface DiagnosticsSettings {
  enabled: boolean;
}

export interface SourceActionSettings {
  enabled: boolean;
}

export interface LintRuleSeverityOverride {
  rule_id: string;
  severity: "error" | "warning" | "info" | "hint";
}

export interface AnalysisSettings {
  fixed_today?: string;
  fixed_local_offset_minutes?: number;
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

type LintProfile = "core" | "recommended" | "strict";

export function getMermanConfiguration(): vscode.WorkspaceConfiguration {
  return vscode.workspace.getConfiguration("merman");
}

export function getTraceSetting(): TraceSetting {
  return getMermanConfiguration().get<TraceSetting>("trace.server", "off");
}

export function getServerSettings(): MermanServerSettings {
  const config = getMermanConfiguration();
  return {
    path: config.get<string>("server.path", "").trim(),
    args: sanitizeStringArray(config.get<unknown[]>("server.args", [])),
    useCargoRun: config.get<boolean>("server.useCargoRun", false),
    cargoArgs: sanitizeStringArray(config.get<unknown[]>("server.cargoArgs", [])),
  };
}

export function getCliSettings(): MermanCliSettings {
  const config = getMermanConfiguration();
  return {
    path: config.get<string>("cli.path", "").trim(),
    useCargoRun: config.get<boolean>("cli.useCargoRun", false),
    cargoArgs: sanitizeStringArray(config.get<unknown[]>("cli.cargoArgs", [])),
  };
}

export function getDiagnosticsSettings(): DiagnosticsSettings {
  const config = getMermanConfiguration();
  return {
    enabled: config.get<boolean>("diagnostics.enabled", true),
  };
}

export function getSourceActionSettings(): SourceActionSettings {
  const config = getMermanConfiguration();
  return {
    enabled: config.get<boolean>("sourceActions.enabled", true),
  };
}

export function getLanguageIntelligenceSettings(): LanguageIntelligenceSettings {
  const config = getMermanConfiguration();
  return {
    enabled: config.get<boolean>("languageIntelligence.enabled", true),
  };
}

export function getAnalysisSettings(): AnalysisSettings {
  const analysisConfig = vscode.workspace.getConfiguration("merman.analysis");
  const fixedToday = normalizeOptionalString(analysisConfig.get<string>("fixed_today", ""));
  const fixedLocalOffsetMinutes = normalizeOptionalNumber(
    analysisConfig.get<number | null>("fixed_local_offset_minutes", null),
  );
  const suppressErrors = analysisConfig.get<boolean>("parse.suppress_errors", false);
  const maxSourceBytes = normalizePositiveNumber(
    analysisConfig.get<number>("resources.max_source_bytes", 0),
  );
  const lintProfile = normalizeLintProfile(
    analysisConfig.get<string>("lint.profile", "recommended"),
  );
  const enableRules = sanitizeStringArray(analysisConfig.get<unknown[]>("lint.enable_rules", []));
  const disableRules = sanitizeStringArray(
    analysisConfig.get<unknown[]>("lint.disable_rules", []),
  );
  const ruleSeverities = sanitizeRuleSeverities(
    analysisConfig.get<unknown[]>("lint.rule_severities", []),
  );

  return compactObject<AnalysisSettings>({
    fixed_today: fixedToday,
    fixed_local_offset_minutes: fixedLocalOffsetMinutes,
    parse: suppressErrors ? { suppress_errors: true } : undefined,
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

export function getDidChangeConfigurationPayload(): Record<string, unknown> {
  return {
    analysis: getAnalysisSettings(),
  };
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

function normalizeOptionalNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function normalizePositiveNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : undefined;
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
