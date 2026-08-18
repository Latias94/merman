/// <reference lib="webworker" />

import { createMermaidSyntaxEngine } from "./syntax-engine.ts";
import {
  createSyntaxWorkerRuntime,
  type SyntaxWorkerRuntimePort,
} from "./syntax-worker-runtime.ts";

const scope = self as unknown as {
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<unknown>) => void,
  ): void;
  addEventListener(type: "messageerror", listener: () => void): void;
  close(): void;
  postMessage(message: unknown, transfer: ArrayBuffer[]): void;
};
const port: SyntaxWorkerRuntimePort = {
  close: () => scope.close(),
  postMessage: (message, transfer) =>
    scope.postMessage(message, transfer ?? []),
};
const runtime = createSyntaxWorkerRuntime(port, createMermaidSyntaxEngine);

scope.addEventListener("message", (event: MessageEvent<unknown>) => {
  void runtime.receive(event.data);
});
scope.addEventListener("messageerror", () => runtime.receiveMessageError());
