import * as vscode from "vscode";

import { getSourceActionSettings } from "./config.js";
import { pngClipboardCommand } from "./export-options.js";
import { listPreviewInputsFromDocument } from "./preview-source.js";
import {
  buildMermaidSourceCodeLensSpecs,
  mermaidSourceCommandIdentity,
  mermaidSourceCommandSourceId,
  mermaidSourceCommandUri,
  mermaidSourceCommandTarget,
  mermaidSourceExportCopyActions,
  shouldRefreshSourceActionCodeLens,
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
  context.subscriptions.push(provider);
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(SOURCE_ACTION_SELECTOR, provider),
  );
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (shouldRefreshSourceActionCodeLens(event.affectsConfiguration.bind(event))) {
        provider.refresh();
      }
    }),
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

class MermaidSourceCodeLensProvider implements vscode.CodeLensProvider, vscode.Disposable {
  private readonly changeEmitter = new vscode.EventEmitter<void>();
  readonly onDidChangeCodeLenses = this.changeEmitter.event;

  constructor(private readonly includeCopyPng: boolean) {}

  provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
    const inputs = listPreviewInputsFromDocument(document);
    const specs = buildMermaidSourceCodeLensSpecs(inputs, getSourceActionSettings());
    return specs.map((spec) => {
      const line = Math.max(0, Math.min(spec.line, document.lineCount - 1));
      return new vscode.CodeLens(new vscode.Range(line, 0, line, 0), {
        title: spec.title,
        command: spec.command,
        arguments: [mermaidSourceCommandTarget(document.uri, spec.sourceIdentity ?? spec.sourceId)],
      });
    });
  }

  refresh(): void {
    this.changeEmitter.fire();
  }

  dispose(): void {
    this.changeEmitter.dispose();
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
    mermaidSourceCommandTarget(
      uri,
      mermaidSourceCommandIdentity(target) ?? mermaidSourceCommandSourceId(target),
    ),
  );
}
