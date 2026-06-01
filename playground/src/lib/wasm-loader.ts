import {
  asciiSupportedDiagrams,
  initMerman,
  renderAscii,
  renderSvg,
  supportedDiagrams,
  themes,
  validate as validateDiagram,
} from "@merman/web";

export interface ValidationResult {
  valid: boolean;
  error?: string;
}

export interface MermanWasm {
  init(): Promise<void>;
  render_svg(code: string, theme: string): string;
  render_ascii(code: string): string | null;
  get_supported_diagrams(): string[];
  get_themes(): string[];
  get_ascii_supported_diagrams(): string[];
  validate(code: string): ValidationResult;
}

let wasmModule: MermanWasm | null = null;
let loadingPromise: Promise<MermanWasm> | null = null;

export async function loadWasm(): Promise<MermanWasm> {
  if (wasmModule) {
    return wasmModule;
  }
  if (loadingPromise) {
    return loadingPromise;
  }

  loadingPromise = (async () => {
    await initMerman();
    wasmModule = createWasmAdapter();
    return wasmModule;
  })();

  return loadingPromise;
}

export function isWasmLoaded(): boolean {
  return wasmModule !== null;
}

export function getWasm(): MermanWasm {
  if (!wasmModule) {
    throw new Error("WASM module not loaded. Call loadWasm() first.");
  }
  return wasmModule;
}

function createWasmAdapter(): MermanWasm {
  return {
    async init() {},

    render_svg(code: string, theme: string): string {
      return renderSvg(sourceWithTheme(code, theme));
    },

    render_ascii(code: string): string | null {
      try {
        return renderAscii(code);
      } catch {
        return null;
      }
    },

    get_supported_diagrams(): string[] {
      return supportedDiagrams();
    },

    get_themes(): string[] {
      return themes();
    },

    get_ascii_supported_diagrams(): string[] {
      return asciiSupportedDiagrams();
    },

    validate(code: string): ValidationResult {
      const result = validateDiagram(code);
      return {
        valid: result.valid,
        error: result.error,
      };
    },
  };
}

function sourceWithTheme(source: string, theme: string): string {
  if (theme === "default" || hasInitDirective(source)) {
    return source;
  }

  const directive = `%%{init: {"theme": "${theme}"}}%%`;
  const newline = source.includes("\r\n") ? "\r\n" : "\n";
  const lines = source.split(/\r?\n/);

  if (lines[0]?.trim() === "---") {
    const frontmatterEnd = lines.findIndex(
      (line, index) => index > 0 && line.trim() === "---",
    );
    if (frontmatterEnd > 0) {
      return [
        ...lines.slice(0, frontmatterEnd + 1),
        directive,
        ...lines.slice(frontmatterEnd + 1),
      ].join(newline);
    }
  }

  return `${directive}${newline}${source}`;
}

function hasInitDirective(source: string): boolean {
  return /%%\s*\{\s*init\s*:/i.test(source);
}
