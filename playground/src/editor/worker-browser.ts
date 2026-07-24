import MermanLanguageWorker from "./merman-language.worker.ts?worker";
import { SEMANTIC_TOKEN_DESCRIPTOR_DIGEST } from "@mermanjs/web";
import {
  createMermanLanguageWorkerClient,
  type EditorLanguageIdentity,
  type MermanLanguageWorkerClient,
} from "./worker-client.ts";

export async function startMermanLanguageWorker(): Promise<{
  readonly client: MermanLanguageWorkerClient;
  readonly identity: EditorLanguageIdentity;
}> {
  const client = createMermanLanguageWorkerClient(
    new MermanLanguageWorker({ name: "merman-editor-language" }),
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST
  );
  try {
    return { client, identity: await client.initialize() };
  } catch (error) {
    client.dispose();
    throw error;
  }
}
