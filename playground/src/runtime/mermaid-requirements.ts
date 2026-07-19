import type { DiagramDetectionFacts } from "@mermanjs/web";
import {
  MERMAID_EXTERNAL_DIAGRAM_ALIAS_TO_MODULE,
  MERMAID_EXTERNAL_DIAGRAM_MODULE_IDS,
  MERMAID_LAYOUT_ID_TO_MODULE,
  MERMAID_LAYOUT_MODULE_IDS,
  type MermaidExternalDiagramModuleId,
  type MermaidLayoutModuleId,
} from "../generated/mermaid-reference.ts";

export {
  MERMAID_EXTERNAL_DIAGRAM_ALIAS_TO_MODULE,
  MERMAID_EXTERNAL_DIAGRAM_MODULE_IDS,
  MERMAID_JS_VERSION,
  MERMAID_LAYOUT_ID_TO_MODULE,
  MERMAID_LAYOUT_ELK_VERSION,
  MERMAID_LAYOUT_MODULE_IDS,
  MERMAID_LAYOUT_TIDY_TREE_VERSION,
  MERMAID_ZENUML_VERSION,
  ZENUML_CORE_VERSION,
} from "../generated/mermaid-reference.ts";
export type {
  MermaidExternalDiagramModuleId,
  MermaidLayoutModuleId,
} from "../generated/mermaid-reference.ts";

export interface MermaidExternalRequirements {
  readonly externalDiagrams: readonly MermaidExternalDiagramModuleId[];
  readonly layoutModules: readonly MermaidLayoutModuleId[];
}

export const NO_MERMAID_EXTERNAL_REQUIREMENTS: MermaidExternalRequirements =
  Object.freeze({
    externalDiagrams: Object.freeze([]),
    layoutModules: Object.freeze([]),
  });

const EXTERNAL_DIAGRAM_MODULE_IDS = new Set<string>(
  MERMAID_EXTERNAL_DIAGRAM_MODULE_IDS
);
const LAYOUT_MODULE_IDS = new Set<string>(MERMAID_LAYOUT_MODULE_IDS);

export function mermaidExternalRequirementsFor(
  facts: DiagramDetectionFacts
): MermaidExternalRequirements {
  if (facts.status === "unavailable") {
    return NO_MERMAID_EXTERNAL_REQUIREMENTS;
  }

  const externalDiagrams: MermaidExternalDiagramModuleId[] = [];
  const layoutModules: MermaidLayoutModuleId[] = [];
  const externalDiagram = lookupRegistration(
    MERMAID_EXTERNAL_DIAGRAM_ALIAS_TO_MODULE,
    facts.diagramType
  );
  const layoutModule = lookupRegistration(
    MERMAID_LAYOUT_ID_TO_MODULE,
    facts.effectiveLayoutId
  );
  if (externalDiagram) externalDiagrams.push(externalDiagram);
  if (layoutModule) layoutModules.push(layoutModule);
  return freezeRequirements(externalDiagrams, layoutModules);
}

function lookupRegistration<T extends string>(
  registrations: Readonly<Record<string, T>>,
  id: string
): T | null {
  return Object.prototype.hasOwnProperty.call(registrations, id)
    ? registrations[id]
    : null;
}

export function normalizeMermaidExternalRequirements(value: {
  readonly externalDiagrams: readonly string[];
  readonly layoutModules: readonly string[];
}): MermaidExternalRequirements {
  const externalDiagrams = normalizeIds(
    value.externalDiagrams,
    EXTERNAL_DIAGRAM_MODULE_IDS,
    "external diagram module id"
  ) as MermaidExternalDiagramModuleId[];
  const layoutModules = normalizeIds(
    value.layoutModules,
    LAYOUT_MODULE_IDS,
    "layout module id"
  ) as MermaidLayoutModuleId[];
  if (externalDiagrams.length === 0 && layoutModules.length === 0) {
    return NO_MERMAID_EXTERNAL_REQUIREMENTS;
  }
  return freezeRequirements(externalDiagrams, layoutModules);
}

function normalizeIds(
  values: readonly string[],
  allowed: ReadonlySet<string>,
  label: string
): string[] {
  const ids = [...new Set(values)].sort();
  for (const id of ids) {
    if (!allowed.has(id)) throw new Error(`Unknown Mermaid ${label}: ${id}`);
  }
  return ids;
}

function freezeRequirements(
  externalDiagrams: MermaidExternalDiagramModuleId[],
  layoutModules: MermaidLayoutModuleId[]
): MermaidExternalRequirements {
  return Object.freeze({
    externalDiagrams: Object.freeze(externalDiagrams),
    layoutModules: Object.freeze(layoutModules),
  });
}
