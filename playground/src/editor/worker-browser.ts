import MermanLanguageWorker from "./merman-language.worker.ts?worker";
import {
  createMermanLanguageWorkerClient,
  type MermanLanguageWorkerClient,
} from "./worker-client.ts";

export async function startMermanLanguageWorker(): Promise<{
  readonly client: MermanLanguageWorkerClient;
  readonly legend: Awaited<ReturnType<MermanLanguageWorkerClient["initialize"]>>;
}> {
  const client = createMermanLanguageWorkerClient(
    new MermanLanguageWorker({ name: "merman-editor-language" })
  );
  try {
    return { client, legend: await client.initialize() };
  } catch (error) {
    client.dispose();
    throw error;
  }
}
