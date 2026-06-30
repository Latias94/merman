import * as vscode from "vscode";

import { pngClipboardCommand } from "./export-options.js";
import { listPreviewInputsFromDocument } from "./preview-source.js";
import {
  buildMermaidSourceCodeLensSpecs,
  mermaidSourceCommandTarget,
} from "./source-actions.js";

const SOURCE_ACTION_SELECTOR: vscode.DocumentSelector = [
  { language: "mermaid" },
  { language: "markdown" },
  { language: "mdx" },
];

export function registerSourceCodeLens(context: vscode.ExtensionContext): void {
  const provider = new MermaidSourceCodeLensProvider(
    pngClipboardCommand(process.platform) !== undefined,
  );
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(SOURCE_ACTION_SELECTOR, provider),
  );
}

class MermaidSourceCodeLensProvider implements vscode.CodeLensProvider {
  constructor(private readonly includeCopyPng: boolean) {}

  provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
    const inputs = listPreviewInputsFromDocument(document);
    const specs = buildMermaidSourceCodeLensSpecs(inputs, {
      includeCopyPng: this.includeCopyPng,
    });
    return specs.map((spec) => {
      const line = Math.max(0, Math.min(spec.line, document.lineCount - 1));
      return new vscode.CodeLens(new vscode.Range(line, 0, line, 0), {
        title: spec.title,
        command: spec.command,
        arguments: [mermaidSourceCommandTarget(document.uri, spec.sourceId)],
      });
    });
  }
}
