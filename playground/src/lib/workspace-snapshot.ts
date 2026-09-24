import type { MermaidThemeSelection } from "./mermaid-theme-name.ts";

import type { DiagramFont } from "./diagram-font.ts";
import { DEFAULT_MERMAID_CONFIG } from "./mermaid-config.ts";
import type {
  MermanSvgPipeline,
  MermanTextMeasurementMode,
} from "../runtime/merman-core.ts";

export interface WorkspaceSnapshot {
  readonly code: string;
  readonly mermaidConfig: string;
  readonly diagramTheme: MermaidThemeSelection;
  readonly presentationThemePresetId: string | null;
  readonly presentationProfileId: string | null;
  readonly svgPipeline: MermanSvgPipeline;
  readonly textMeasurementMode: MermanTextMeasurementMode;
  readonly diagramFont: DiagramFont;
}

export const DEFAULT_WORKSPACE_SNAPSHOT: Readonly<WorkspaceSnapshot> =
  Object.freeze({
    code: `flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D`,
    mermaidConfig: DEFAULT_MERMAID_CONFIG,
    diagramTheme: "auto",
    presentationThemePresetId: null,
    presentationProfileId: null,
    svgPipeline: "parity",
    textMeasurementMode: "browser",
    diagramFont: "trebuchet",
  });
