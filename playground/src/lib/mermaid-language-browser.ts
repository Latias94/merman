import type { MermaidLanguageCallbacks } from "./mermaid-language.ts";
import {
  registerMermaidLanguage,
  type MermaidLanguageRegistration,
} from "./mermaid-language.ts";
import { startMermanLanguageWorker } from "../editor/worker-browser.ts";
import { startMermaidSyntaxWorker } from "../editor/syntax-worker-browser.ts";

export async function registerBrowserMermaidLanguage(
  monaco: typeof import("monaco-editor"),
  callbacks: MermaidLanguageCallbacks = {},
): Promise<MermaidLanguageRegistration> {
  const semantic = startMermanLanguageWorker();
  let syntax: ReturnType<typeof startMermaidSyntaxWorker>;
  try {
    syntax = startMermaidSyntaxWorker();
  } catch (error) {
    semantic.client.dispose();
    throw error;
  }
  try {
    return await registerMermaidLanguage(
      monaco,
      { semantic, syntax },
      callbacks,
    );
  } catch (error) {
    syntax.client.dispose();
    semantic.client.dispose();
    throw error;
  }
}
