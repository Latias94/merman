import {
  languageIntelligenceDisabledMessage,
  serverBackedCommandAction,
  type LanguageIntelligenceSettings,
} from "./language-intelligence.js";

export type RestartLanguageServerOutcome = "disabled" | "failed" | "restarted";

type MaybePromise<T> = T | PromiseLike<T>;

export interface RestartLanguageServerCommandOptions {
  settings: LanguageIntelligenceSettings;
  updateDisabledStatus(): void;
  runRestart(): Promise<void>;
  showWarningMessage(message: string): MaybePromise<unknown>;
  showInformationMessage(message: string): MaybePromise<unknown>;
}

export async function runRestartLanguageServerCommand({
  settings,
  updateDisabledStatus,
  runRestart,
  showWarningMessage,
  showInformationMessage,
}: RestartLanguageServerCommandOptions): Promise<RestartLanguageServerOutcome> {
  if (serverBackedCommandAction(settings) === "showDisabledWarning") {
    updateDisabledStatus();
    void showWarningMessage(languageIntelligenceDisabledMessage());
    return "disabled";
  }

  try {
    await runRestart();
  } catch {
    return "failed";
  }
  void showInformationMessage("Merman language server restarted.");
  return "restarted";
}
