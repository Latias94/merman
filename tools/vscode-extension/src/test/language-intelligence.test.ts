import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { describe, it } from "node:test";

import {
  LANGUAGE_INTELLIGENCE_SETTING,
  languageClientConfigurationAction,
  languageClientReconcileAction,
  languageIntelligenceDisabledMessage,
  serverBackedCommandAction,
  shouldStartLanguageClient,
} from "../language-intelligence.js";

describe("language intelligence adoption", () => {
  it("starts the language client only when language intelligence is enabled", () => {
    assert.equal(shouldStartLanguageClient({ enabled: true }), true);
    assert.equal(shouldStartLanguageClient({ enabled: false }), false);
  });

  it("models activation and setting-toggle lifecycle actions", () => {
    assert.equal(languageClientReconcileAction({ enabled: false }, false), "showDisabledStatus");
    assert.equal(languageClientReconcileAction({ enabled: false }, true), "stopAndDisable");
    assert.equal(languageClientReconcileAction({ enabled: true }, false), "start");
    assert.equal(languageClientReconcileAction({ enabled: true }, true), "pushConfiguration");
  });

  it("restarts only enabled clients for server-shape configuration changes", () => {
    assert.equal(languageClientConfigurationAction({
      affectsMerman: true,
      affectsLanguageIntelligence: false,
      serverShapeChanged: true,
      hasClient: true,
      settings: { enabled: true },
    }), "restart");
    assert.equal(languageClientConfigurationAction({
      affectsMerman: true,
      affectsLanguageIntelligence: false,
      serverShapeChanged: true,
      hasClient: true,
      settings: { enabled: false },
    }), "showDisabledStatus");
  });

  it("keeps server-backed commands non-invasive when disabled", () => {
    assert.equal(serverBackedCommandAction({ enabled: true }), "run");
    assert.equal(serverBackedCommandAction({ enabled: false }), "showDisabledWarning");
  });

  it("declares language intelligence as a VS Code setting with a non-invasive default", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      contributes: {
        configuration:
          | {
              properties: Record<string, { default?: unknown; markdownDescription?: string }>;
            }
          | Array<{
              properties: Record<string, { default?: unknown; markdownDescription?: string }>;
            }>;
      };
    };
    const setting = configurationProperties(pkg.contributes.configuration)[LANGUAGE_INTELLIGENCE_SETTING];

    assert.equal(setting?.default, true);
    assert.match(setting?.markdownDescription ?? "", /language server/);
    assert.match(setting?.markdownDescription ?? "", /preview and export/);
  });

  it("declares preview defaults in the native settings schema", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      contributes: {
        configuration:
          | {
              properties: Record<string, { default?: unknown; enum?: unknown[]; scope?: string }>;
            }
          | Array<{
              properties: Record<string, { default?: unknown; enum?: unknown[]; scope?: string }>;
            }>;
      };
    };
    const properties = configurationProperties(pkg.contributes.configuration);

    assert.deepEqual(properties["merman.preview.diagramTheme"]?.enum, [
      "source",
      "default",
      "dark",
      "forest",
      "neutral",
      "base",
    ]);
    assert.deepEqual(properties["merman.preview.displayMode"]?.enum, [
      "svg",
      "ascii",
      "unicode",
    ]);
    assert.deepEqual(properties["merman.preview.background"]?.enum, [
      "paper",
      "transparent",
      "dark",
    ]);
    assert.equal(properties["merman.preview.background"]?.scope, "resource");
  });

  it("groups settings by LSP-style product areas", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      contributes: {
        configuration: Array<{
          title: string;
          properties: Record<string, { default?: unknown; markdownDescription?: string }>;
        }>;
      };
    };

    assert.deepEqual(
      pkg.contributes.configuration.map((section) => section.title),
      [
        "Merman: Runtime",
        "Merman: Language Intelligence",
        "Merman: Analysis",
        "Merman: Preview and Export",
        "Merman: Development",
      ],
    );
  });

  it("uses an actionable disabled message for server-backed commands", () => {
    assert.match(languageIntelligenceDisabledMessage(), /language intelligence is disabled/);
    assert.match(languageIntelligenceDisabledMessage(), /merman\.languageIntelligence\.enabled/);
  });
});

function configurationProperties<T>(
  configuration: { properties: Record<string, T> } | Array<{ properties: Record<string, T> }>,
): Record<string, T> {
  if (!Array.isArray(configuration)) {
    return configuration.properties;
  }
  return Object.assign({}, ...configuration.map((section) => section.properties));
}
