import * as vscode from "vscode";

import type { LanguageIntelligenceSettings } from "./language-intelligence.js";
import type {
  PreviewBackground,
  PreviewDiagramTheme,
  PreviewDisplayMode,
} from "./preview-model.js";
import {
  BUNDLED_ANALYSIS_CONFIG,
  type NegotiatedAnalysisConfig,
} from "./analysis-config-contract.js";
import {
  bootstrapAnalysisSettings,
  normalizeAnalysisSettings,
  projectAnalysisSettings,
  type AnalysisSettings,
} from "./analysis-settings.js";

export type { AnalysisSettings } from "./analysis-settings.js";

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

export interface PreviewSettings {
  diagramTheme: PreviewDiagramTheme;
  displayMode: PreviewDisplayMode;
  background: PreviewBackground;
}

export function getMermanConfiguration(resource?: vscode.Uri): vscode.WorkspaceConfiguration {
  return vscode.workspace.getConfiguration("merman", resource);
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

export function getSourceActionSettings(resource: vscode.Uri): SourceActionSettings {
  const config = getMermanConfiguration(resource);
  return {
    enabled: config.get<boolean>("sourceActions.enabled", true),
  };
}

export function getPreviewSettings(): PreviewSettings {
  const config = getMermanConfiguration();
  return {
    diagramTheme: normalizePreviewDiagramTheme(
      config.get<string>("preview.diagramTheme", "source"),
    ),
    displayMode: normalizePreviewDisplayMode(config.get<string>("preview.displayMode", "svg")),
    background: normalizePreviewBackground(config.get<string>("preview.background", "paper")),
  };
}

export function getLanguageIntelligenceSettings(): LanguageIntelligenceSettings {
  const config = getMermanConfiguration();
  return {
    enabled: config.get<boolean>("languageIntelligence.enabled", true),
  };
}

export function getAnalysisSettings(contract: NegotiatedAnalysisConfig): AnalysisSettings {
  return normalizeAnalysisSettings(getRawAnalysisSettings(contract), contract);
}

export function getInitializationAnalysisSettings(): AnalysisSettings {
  return bootstrapAnalysisSettings(getRawAnalysisSettings(BUNDLED_ANALYSIS_CONFIG));
}

function getRawAnalysisSettings(contract: Pick<NegotiatedAnalysisConfig, "settings">) {
  const analysisConfig = vscode.workspace.getConfiguration("merman.analysis");
  return Object.fromEntries(
    contract.settings.map((setting) => [
      setting.path,
      analysisConfig.get<unknown>(setting.path),
    ]),
  );
}

export function getDidChangeConfigurationPayload(
  contract: NegotiatedAnalysisConfig,
): { payload: Record<string, unknown>; unsupportedRuleIds: string[] } {
  const projection = projectAnalysisSettings(
    getAnalysisSettings(contract),
    contract.configurableRuleIds,
  );
  return {
    payload: { analysis: projection.settings },
    unsupportedRuleIds: projection.unsupportedRuleIds,
  };
}

function normalizePreviewDiagramTheme(value: string): PreviewDiagramTheme {
  switch (value) {
    case "source":
    case "default":
    case "dark":
    case "forest":
    case "neutral":
    case "base":
      return value;
    default:
      return "source";
  }
}

function normalizePreviewDisplayMode(value: string): PreviewDisplayMode {
  switch (value) {
    case "svg":
    case "ascii":
    case "unicode":
      return value;
    default:
      return "svg";
  }
}

function normalizePreviewBackground(value: string): PreviewBackground {
  switch (value) {
    case "paper":
    case "transparent":
    case "dark":
      return value;
    default:
      return "paper";
  }
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
