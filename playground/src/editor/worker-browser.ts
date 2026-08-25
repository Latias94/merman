import MermanLanguageWorker from "./merman-language.worker.ts?worker";
import {
  startMermanLanguageWorkerClient,
  type MermanLanguageWorkerStartup,
} from "./worker-client.ts";

export function startMermanLanguageWorker(): MermanLanguageWorkerStartup {
  return startMermanLanguageWorkerClient(
    new MermanLanguageWorker({ name: "merman-editor-language" }),
  );
}
