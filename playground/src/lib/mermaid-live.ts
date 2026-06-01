import { buildMermaidConfig } from "@/src/lib/mermaid-config";

const MERMAID_LIVE_EDIT_URL = "https://mermaid.live/edit";
const MERMAID_INK_IMAGE_URL = "https://mermaid.ink/img";

interface MermaidLiveState {
  code: string;
  mermaid: string;
  updateDiagram: boolean;
  rough: boolean;
  panZoom: boolean;
  grid: boolean;
  editorMode: "code" | "config";
}

export function createMermaidLiveEditorUrl(
  code: string,
  theme: string,
  configJson: string
): string {
  return `${MERMAID_LIVE_EDIT_URL}#${serializeMermaidLiveState(
    code,
    theme,
    configJson
  )}`;
}

export function createMarkdownImageLink(
  code: string,
  theme: string,
  configJson: string
): string {
  const serialized = serializeMermaidLiveState(code, theme, configJson);
  const imageUrl = `${MERMAID_INK_IMAGE_URL}/${serialized}?type=png`;
  const liveUrl = `${MERMAID_LIVE_EDIT_URL}#${serialized}`;
  return `[![](${imageUrl})](${liveUrl})`;
}

function serializeMermaidLiveState(
  code: string,
  theme: string,
  configJson: string
): string {
  const state: MermaidLiveState = {
    code,
    mermaid: `${JSON.stringify(buildMermaidConfig(configJson, theme), null, 2)}\n`,
    updateDiagram: true,
    rough: false,
    panZoom: true,
    grid: true,
    editorMode: "code",
  };
  return `base64:${base64UrlEncode(JSON.stringify(state))}`;
}

function base64UrlEncode(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
}
