export const LANGUAGE_INTELLIGENCE_SETTING = "merman.languageIntelligence.enabled";

export interface LanguageIntelligenceSettings {
  enabled: boolean;
}

export type LanguageClientLifecycleAction =
  | "ignore"
  | "showDisabledStatus"
  | "stopAndDisable"
  | "start"
  | "restart"
  | "pushConfiguration";

export interface LanguageClientConfigurationChange {
  affectsMerman: boolean;
  affectsLanguageIntelligence: boolean;
  serverShapeChanged: boolean;
  hasClient: boolean;
  settings: LanguageIntelligenceSettings;
}

export function shouldStartLanguageClient(settings: LanguageIntelligenceSettings): boolean {
  return settings.enabled;
}

export function languageClientReconcileAction(
  settings: LanguageIntelligenceSettings,
  hasClient: boolean,
): LanguageClientLifecycleAction {
  if (!shouldStartLanguageClient(settings)) {
    return hasClient ? "stopAndDisable" : "showDisabledStatus";
  }
  return hasClient ? "pushConfiguration" : "start";
}

export function languageClientWorkspaceTrustAction(
  settings: LanguageIntelligenceSettings,
  hasClient: boolean,
): LanguageClientLifecycleAction {
  return languageClientReconcileAction(settings, hasClient);
}

export function languageClientConfigurationAction(
  change: LanguageClientConfigurationChange,
): LanguageClientLifecycleAction {
  if (!change.affectsMerman) {
    return "ignore";
  }
  if (change.affectsLanguageIntelligence) {
    return languageClientReconcileAction(change.settings, change.hasClient);
  }
  if (!shouldStartLanguageClient(change.settings)) {
    return "showDisabledStatus";
  }
  if (change.serverShapeChanged) {
    return "restart";
  }
  return change.hasClient ? "pushConfiguration" : "ignore";
}

export function serverBackedCommandAction(
  settings: LanguageIntelligenceSettings,
): "run" | "showDisabledWarning" {
  return shouldStartLanguageClient(settings) ? "run" : "showDisabledWarning";
}

export function languageIntelligenceDisabledMessage(): string {
  return "Merman language intelligence is disabled. Enable `merman.languageIntelligence.enabled` to start the local language server.";
}
