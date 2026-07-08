import {
  languageIntelligenceDisabledMessage,
  serverBackedCommandAction,
  type LanguageIntelligenceSettings,
} from "./language-intelligence.js";
import { errorMessage } from "./error-message.js";

export type ServerBackedCommandOutcome =
  | "completed"
  | "disabled"
  | "failed"
  | "missingClient";

type MaybePromise<T> = T | PromiseLike<T>;

export interface ServerBackedCommandOptions<TClient, TResponse> {
  settings: LanguageIntelligenceSettings;
  client: TClient | undefined;
  request(client: TClient): Promise<TResponse>;
  handleResponse(response: TResponse): Promise<void>;
  failureMessagePrefix: string;
  showWarningMessage(message: string): MaybePromise<unknown>;
}

export async function runServerBackedCommand<TClient, TResponse>({
  settings,
  client,
  request,
  handleResponse,
  failureMessagePrefix,
  showWarningMessage,
}: ServerBackedCommandOptions<TClient, TResponse>): Promise<ServerBackedCommandOutcome> {
  if (serverBackedCommandAction(settings) === "showDisabledWarning") {
    void showWarningMessage(languageIntelligenceDisabledMessage());
    return "disabled";
  }

  if (!client) {
    void showWarningMessage("Merman language server is not running.");
    return "missingClient";
  }

  try {
    const response = await request(client);
    await handleResponse(response);
    return "completed";
  } catch (error) {
    void showWarningMessage(`${failureMessagePrefix}: ${errorMessage(error)}`);
    return "failed";
  }
}
