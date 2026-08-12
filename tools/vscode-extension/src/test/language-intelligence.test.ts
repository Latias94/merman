import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { describe, it } from "node:test";

import {
  LANGUAGE_INTELLIGENCE_SETTING,
  languageClientConfigurationAction,
  languageClientReconcileAction,
  languageClientWorkspaceTrustAction,
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

  it("retries trusted runtime startup when Workspace Trust is granted", () => {
    assert.equal(languageClientWorkspaceTrustAction({ enabled: true }, false), "start");
    assert.equal(languageClientWorkspaceTrustAction({ enabled: true }, true), "pushConfiguration");
    assert.equal(languageClientWorkspaceTrustAction({ enabled: false }, false), "showDisabledStatus");
    assert.equal(languageClientWorkspaceTrustAction({ enabled: false }, true), "stopAndDisable");
  });

  it("restarts only enabled clients for server-shape configuration changes", () => {
    assert.equal(languageClientConfigurationAction({
      affectsMerman: true,
      affectsLanguageIntelligence: false,
      diagnosticsEnabledChanged: false,
      diagnosticsEnabled: true,
      serverShapeChanged: true,
      hasClient: true,
      settings: { enabled: true },
    }), "restart");
    assert.equal(languageClientConfigurationAction({
      affectsMerman: true,
      affectsLanguageIntelligence: false,
      diagnosticsEnabledChanged: false,
      diagnosticsEnabled: true,
      serverShapeChanged: true,
      hasClient: true,
      settings: { enabled: false },
    }), "showDisabledStatus");
  });

  it("restarts existing clients when diagnostics are re-enabled", () => {
    assert.equal(languageClientConfigurationAction({
      affectsMerman: true,
      affectsLanguageIntelligence: false,
      diagnosticsEnabledChanged: true,
      diagnosticsEnabled: true,
      serverShapeChanged: false,
      hasClient: true,
      settings: { enabled: true },
    }), "restart");
    assert.equal(languageClientConfigurationAction({
      affectsMerman: true,
      affectsLanguageIntelligence: false,
      diagnosticsEnabledChanged: true,
      diagnosticsEnabled: false,
      serverShapeChanged: false,
      hasClient: true,
      settings: { enabled: true },
    }), "pushConfiguration");
    assert.equal(languageClientConfigurationAction({
      affectsMerman: true,
      affectsLanguageIntelligence: false,
      diagnosticsEnabledChanged: true,
      diagnosticsEnabled: true,
      serverShapeChanged: false,
      hasClient: false,
      settings: { enabled: true },
    }), "ignore");
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

  it("declares limited Workspace Trust support for safe packaged functionality", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      capabilities?: {
        untrustedWorkspaces?: {
          supported?: string;
          description?: string;
        };
      };
    };

    assert.equal(pkg.capabilities?.untrustedWorkspaces?.supported, "limited");
    assert.match(pkg.capabilities?.untrustedWorkspaces?.description ?? "", /Cargo-run/);
    assert.match(pkg.capabilities?.untrustedWorkspaces?.description ?? "", /Workspace Trust/);
  });

  it("declares analysis numeric settings with LSP-compatible integer bounds", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      contributes: {
        configuration:
          | {
              properties: Record<string, {
                maximum?: number;
                minimum?: number;
                pattern?: string;
                type?: string | string[];
              }>;
            }
          | Array<{
              properties: Record<string, {
                maximum?: number;
                minimum?: number;
                pattern?: string;
                type?: string | string[];
              }>;
            }>;
      };
    };
    const properties = configurationProperties(pkg.contributes.configuration);

    const fixedTodayPattern = properties["merman.analysis.fixed_today"]?.pattern;
    assert.equal(
      fixedTodayPattern,
      "^$|^(?:\\d{4}|\\+(?:[1-9]\\d{4,8}|1\\d{9}|20\\d{8}|21[0-3]\\d{7}|214[0-6]\\d{6}|2147[0-3]\\d{5}|21474[0-7]\\d{4}|214748[0-2]\\d{3}|2147483[0-5]\\d{2}|21474836[0-3]\\d|214748364[0-7])|-(?:000[1-9]|00[1-9]\\d|0[1-9]\\d{2}|[1-9]\\d{3}|[1-9]\\d{4,8}|1\\d{9}|20\\d{8}|21[0-3]\\d{7}|214[0-6]\\d{6}|2147[0-3]\\d{5}|21474[0-7]\\d{4}|214748[0-2]\\d{3}|2147483[0-5]\\d{2}|21474836[0-3]\\d|214748364[0-8]))-\\d{2}-\\d{2}$",
    );
    const fixedToday = new RegExp(fixedTodayPattern ?? "");
    for (const value of [
      "",
      "0000-01-01",
      "9999-12-31",
      "+10000-01-01",
      "+2147483647-12-31",
      "-0001-01-01",
      "-10000-01-01",
      "-2147483648-01-01",
    ]) {
      assert.equal(fixedToday.test(value), true, value);
    }
    for (const value of [
      "+9999-01-01",
      "+010000-01-01",
      "+2147483648-01-01",
      "-0000-01-01",
      "-010000-01-01",
      "-2147483649-01-01",
      "10000-01-01",
    ]) {
      assert.equal(fixedToday.test(value), false, value);
    }
    assert.deepEqual(properties["merman.analysis.fixed_local_offset_minutes"]?.type, [
      "integer",
      "null",
    ]);
    assert.equal(properties["merman.analysis.fixed_local_offset_minutes"]?.minimum, -1439);
    assert.equal(properties["merman.analysis.fixed_local_offset_minutes"]?.maximum, 1439);
    assert.equal(properties["merman.analysis.site_config"]?.type, "object");
    assert.deepEqual(properties["merman.analysis.resources.limits.max_source_bytes"]?.type, [
      "integer",
      "null",
    ]);
    assert.equal(properties["merman.analysis.resources.limits.max_source_bytes"]?.minimum, 1);
    assert.equal(
      properties["merman.analysis.resources.limits.max_source_bytes"]?.maximum,
      0xffff_ffff,
    );
    assert.deepEqual(
      properties["merman.analysis.resources.limits.max_document_diagrams"]?.type,
      ["integer", "null"],
    );
    assert.equal(
      properties["merman.analysis.resources.limits.max_document_diagrams"]?.minimum,
      0,
    );
    assert.equal(
      properties["merman.analysis.resources.limits.max_document_diagrams"]?.maximum,
      0xffff_ffff,
    );
  });

  it("does not advertise resource overrides for settings consumed as one global profile", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      contributes: {
        configuration:
          | {
              properties: Record<string, { scope?: string }>;
            }
          | Array<{
              properties: Record<string, { scope?: string }>;
            }>;
      };
    };
    const properties = configurationProperties(pkg.contributes.configuration);
    const globalProfileSettings = [
      "merman.languageIntelligence.enabled",
      "merman.diagnostics.enabled",
      "merman.analysis.fixed_today",
      "merman.analysis.fixed_local_offset_minutes",
      "merman.analysis.site_config",
      "merman.analysis.resources.limits.max_source_bytes",
      "merman.analysis.lint.profile",
      "merman.analysis.lint.enable_rules",
      "merman.analysis.lint.disable_rules",
      "merman.analysis.lint.rule_severities",
      "merman.preview.diagramTheme",
      "merman.preview.displayMode",
      "merman.preview.background",
    ];

    for (const setting of globalProfileSettings) {
      assert.equal(properties[setting]?.scope, "window", setting);
    }
    assert.equal(
      properties["merman.analysis.parse.suppress_errors"],
      undefined,
      "removed analysis settings must not be advertised again",
    );
    assert.equal(properties["merman.sourceActions.enabled"]?.scope, "resource");
  });

  it("documents current lint rule ids without rejecting future catalog entries", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      contributes: {
        configuration:
          | { properties: Record<string, unknown> }
          | Array<{ properties: Record<string, unknown> }>;
      };
    };
    const properties = configurationProperties(pkg.contributes.configuration) as Record<
      string,
      {
        items?: {
          enum?: unknown[];
          examples?: unknown[];
          properties?: Record<string, { enum?: unknown[]; examples?: unknown[] }>;
        };
      }
    >;

    for (const setting of [
      "merman.analysis.lint.enable_rules",
      "merman.analysis.lint.disable_rules",
    ]) {
      assert.equal(properties[setting]?.items?.enum, undefined, setting);
      assert.ok(
        properties[setting]?.items?.examples?.includes(
          "merman.parse.no_diagram",
        ),
        setting,
      );
    }

    const ruleId = properties["merman.analysis.lint.rule_severities"]
      ?.items?.properties?.rule_id;
    assert.equal(ruleId?.enum, undefined);
    assert.ok(ruleId?.examples?.includes("merman.parse.no_diagram"));
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
    assert.equal(properties["merman.preview.background"]?.scope, "window");
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
