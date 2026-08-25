import MermaidSyntaxWorker from "./mermaid-syntax.worker.ts?worker";
import {
  startMermaidSyntaxWorkerClient,
  type MermaidSyntaxWorkerStartup,
} from "./syntax-worker-client.ts";

export function startMermaidSyntaxWorker(): MermaidSyntaxWorkerStartup {
  return startMermaidSyntaxWorkerClient(
    new MermaidSyntaxWorker({ name: "mermaid-tree-sitter-syntax" }),
  );
}
