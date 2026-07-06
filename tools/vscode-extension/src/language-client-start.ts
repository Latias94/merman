import { errorMessage } from "./error-message.js";

export interface StartableLanguageClient {
  start(): Promise<void>;
  stop(): Promise<void>;
}

export interface LanguageClientStartOptions<T extends StartableLanguageClient> {
  client: T;
  generation: number;
  startingTooltip: string;
  failedTooltip: string;
  isCurrentGeneration(generation: number): boolean;
  wireClient(client: T): void;
  updateStatus(stateLabel: string, tooltip?: string): void;
  pushConfiguration(client: T): Promise<void>;
  assignClient(client: T): void;
  clearClientIfCurrent(client: T): void;
  showStartError(message: string): void;
  onStaleStartup?(): void;
}

export async function startLanguageClientWithCleanup<T extends StartableLanguageClient>({
  client,
  generation,
  startingTooltip,
  failedTooltip,
  isCurrentGeneration,
  wireClient,
  updateStatus,
  pushConfiguration,
  assignClient,
  clearClientIfCurrent,
  showStartError,
  onStaleStartup,
}: LanguageClientStartOptions<T>): Promise<void> {
  wireClient(client);
  updateStatus("Starting", startingTooltip);
  try {
    await client.start();
    if (!isCurrentGeneration(generation)) {
      await client.stop();
      onStaleStartup?.();
      return;
    }
    await pushConfiguration(client);
    if (!isCurrentGeneration(generation)) {
      await client.stop();
      onStaleStartup?.();
      return;
    }
    assignClient(client);
  } catch (error) {
    clearClientIfCurrent(client);
    await client.stop().catch(() => undefined);
    updateStatus("Failed", failedTooltip);
    showStartError(`Merman language server failed to start: ${errorMessage(error)}`);
    throw error;
  }
}
