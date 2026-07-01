import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { describe, it } from "node:test";

import {
  LANGUAGE_INTELLIGENCE_SETTING,
  languageIntelligenceDisabledMessage,
  shouldStartLanguageClient,
} from "../language-intelligence.js";

describe("language intelligence adoption", () => {
  it("starts the language client only when language intelligence is enabled", () => {
    assert.equal(shouldStartLanguageClient({ enabled: true }), true);
    assert.equal(shouldStartLanguageClient({ enabled: false }), false);
  });

  it("declares language intelligence as a VS Code setting with a non-invasive default", () => {
    const pkg = JSON.parse(
      fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
    ) as {
      contributes: {
        configuration: {
          properties: Record<string, { default?: unknown; markdownDescription?: string }>;
        };
      };
    };
    const setting = pkg.contributes.configuration.properties[LANGUAGE_INTELLIGENCE_SETTING];

    assert.equal(setting?.default, true);
    assert.match(setting?.markdownDescription ?? "", /language server/);
    assert.match(setting?.markdownDescription ?? "", /preview and export/);
  });

  it("uses an actionable disabled message for server-backed commands", () => {
    assert.match(languageIntelligenceDisabledMessage(), /language intelligence is disabled/);
    assert.match(languageIntelligenceDisabledMessage(), /merman\.languageIntelligence\.enabled/);
  });
});
