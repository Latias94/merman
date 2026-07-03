import * as vscode from "vscode";

import { getSourceActionSettings } from "./config.js";
import { pngClipboardCommand } from "./export-options.js";
import { listPreviewInputsFromDocument } from "./preview-source.js";
import {
  buildMermaidSourceCodeLensSpecs,
  mermaidSourceCommandSourceId,
  mermaidSourceCommandUri,
  mermaidSourceCommandTarget,
  mermaidSourceExportCopyActions,
  type MermaidSourceCommandArgument,
} from "./source-actions.js";

const SOURCE_ACTION_SELECTOR: vscode.DocumentSelector = [
  { language: "mermaid" },
  { language: "markdown" },
  { language: "mdx" },
  { scheme: "file", pattern: "**/*.mdx" },
];

export function registerSourceCodeLens(context: vscode.ExtensionContext): void {
  const includeCopyPng = pngClipboardCommand(process.platform) !== undefined;
  const provider = new MermaidSourceCodeLensProvider(includeCopyPng);
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(SOURCE_ACTION_SELECTOR, provider),
  );
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "merman.sourceActions",
      async (target?: MermaidSourceCommandArgument) => {
        await showSourceActionPicker(target, includeCopyPng);
      },
    ),
  );
}

class MermaidSourceCodeLensProvider implements vscode.CodeLensProvider {
  constructor(private readonly includeCopyPng: boolean) {}

  provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
    const inputs = listPreviewInputsFromDocument(document);
    const specs = buildMermaidSourceCodeLensSpecs(inputs, getSourceActionSettings());
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

async function showSourceActionPicker(
  target: MermaidSourceCommandArgument | undefined,
  includeCopyPng: boolean,
): Promise<void> {
  const uri = mermaidSourceCommandUri(target);
  if (!uri) {
    return;
  }
  const picked = await vscode.window.showQuickPick(
    mermaidSourceExportCopyActions({ includeCopyPng }).map((action) => ({
      label: action.title,
      command: action.command,
    })),
    {
      placeHolder: "Choose a Mermaid source action",
    },
  );
  if (!picked) {
    return;
  }
  await vscode.commands.executeCommand(
    picked.command,
    mermaidSourceCommandTarget(uri, mermaidSourceCommandSourceId(target)),
  );
}
