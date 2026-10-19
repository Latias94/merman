import type { ASTNode } from "monaco-editor/languages/features/json/register";

import { parseThemeDesignPointer } from "./theme-designer-model.ts";

export interface ThemeJsonRevealRequest {
  readonly sessionId: number;
  readonly revision: number;
  readonly rawJson: string;
  readonly pointer: string;
}

/** Locate an authored value in Monaco's parsed document without searching equal literals. */
export function findThemeJsonValue(root: ASTNode | undefined, pointer: string): ASTNode | undefined {
  const segments = parseThemeDesignPointer(pointer);
  if (!segments) return undefined;
  let node = root;
  for (const segment of segments) {
    if (node?.type === "object") {
      node = node.properties.find((property) => property.keyNode.value === segment)?.valueNode;
    } else if (node?.type === "array" && /^(0|[1-9]\d*)$/.test(segment)) {
      node = node.items[Number(segment)];
    } else {
      return undefined;
    }
  }
  return node;
}
