/// <reference lib="webworker" />

import {
  createEditorSession,
  editorCompletionTriggerCharacters,
  editorSemanticTokenDescriptor,
  initMerman,
  runtimeCatalog,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  transportApiVersion,
} from "@mermanjs/web";
import {
  createEditorWorkerRuntime,
  type EditorWorkerRuntimePort,
} from "./worker-runtime.ts";

const scope = self as unknown as {
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<unknown>) => void,
  ): void;
  addEventListener(type: "messageerror", listener: () => void): void;
  close(): void;
  postMessage(message: unknown, transfer: ArrayBuffer[]): void;
};
const port: EditorWorkerRuntimePort = {
  close: () => scope.close(),
  postMessage: (message, transfer) =>
    scope.postMessage(message, transfer ?? []),
};
const runtime = createEditorWorkerRuntime(port, {
  createEditorSession,
  editorCompletionTriggerCharacters,
  editorSemanticTokenDescriptor,
  initMerman,
  legendDigest: SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  runtimeCatalog,
  transportApiVersion,
});

scope.addEventListener("message", (event: MessageEvent<unknown>) => {
  void runtime.receive(event.data);
});

scope.addEventListener("messageerror", () => {
  runtime.receiveMessageError();
});
