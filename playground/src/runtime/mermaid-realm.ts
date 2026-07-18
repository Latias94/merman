import { assertSafeSvgForDom } from "@mermanjs/web/svg-safety";

import {
  createMermaidRealmController,
  type MermaidRealmController,
} from "./mermaid-realm-controller.ts";
import { createBrowserCompareRealmSession } from "./realm/parent-channel.ts";

function createBrowserCompareRealmController(): MermaidRealmController {
  return createMermaidRealmController({
    kind: "compare",
    createSession: (_kind, viewport, signal) =>
      createBrowserCompareRealmSession(viewport, signal),
    validateSvg: assertSafeSvgForDom,
  });
}

export const compareMermaidRealmController =
  createBrowserCompareRealmController();
