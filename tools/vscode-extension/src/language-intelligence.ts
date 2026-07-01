export const LANGUAGE_INTELLIGENCE_SETTING = "merman.languageIntelligence.enabled";

export interface LanguageIntelligenceSettings {
  enabled: boolean;
}

export function shouldStartLanguageClient(settings: LanguageIntelligenceSettings): boolean {
  return settings.enabled;
}

export function languageIntelligenceDisabledMessage(): string {
  return "Merman language intelligence is disabled. Enable `merman.languageIntelligence.enabled` to start the local language server.";
}
