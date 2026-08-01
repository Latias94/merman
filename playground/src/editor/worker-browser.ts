import MermanLanguageWorker from "./merman-language.worker.ts?worker";
import { SEMANTIC_TOKEN_DESCRIPTOR_DIGEST } from "@mermanjs/web";
import {
  startMermanLanguageWorkerClient,
  type MermanLanguageWorkerStartup,
} from "./worker-client.ts";

export function startMermanLanguageWorker(): MermanLanguageWorkerStartup {
  return startMermanLanguageWorkerClient(
    new MermanLanguageWorker({ name: "merman-editor-language" }),
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST
  );
}
