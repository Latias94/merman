import { useEffect, useState, useCallback, useRef } from "react";
import { DEFAULT_MERMAID_CONFIG } from "@/src/lib/mermaid-config";
import {
  getWasm,
  isWasmLoaded,
  loadWasm,
  SUPPORTED_THEMES,
  type MermanWasm,
  type SvgPipeline,
  type ValidationResult,
} from "@/src/lib/wasm-loader";

export interface RenderResult {
  svg: string | null;
  error: string | null;
  renderTime: number;
}

export const MERMAN_WASM_LOADING_ERROR = "__merman_wasm_loading__";
export const MERMAN_WASM_NOT_LOADED_ERROR = "__merman_wasm_not_loaded__";

interface RenderOptions {
  pipeline?: SvgPipeline;
}

export function mermanRuntimeErrorI18nKey(message: string | null | undefined) {
  if (message === MERMAN_WASM_LOADING_ERROR) return "wasm.loading";
  if (message === MERMAN_WASM_NOT_LOADED_ERROR) return "wasm.notLoaded";
  return null;
}

export function useMerman() {
  const initialWasm = isWasmLoaded() ? getWasm() : null;
  const [ready, setReady] = useState(initialWasm !== null);
  const [loading, setLoading] = useState(initialWasm === null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const wasmRef = useRef<MermanWasm | null>(initialWasm);

  useEffect(() => {
    let mounted = true;

    if (wasmRef.current) {
      setReady(true);
      setLoading(false);
      return () => {
        mounted = false;
      };
    }

    loadWasm()
      .then((wasm) => {
        if (mounted) {
          wasmRef.current = wasm;
          setReady(true);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (mounted) {
          setLoadError(err.message);
          setLoading(false);
        }
      });

    return () => {
      mounted = false;
    };
  }, []);

  const render = useCallback(
    (
      code: string,
      theme: string,
      configJson = DEFAULT_MERMAID_CONFIG,
      options?: RenderOptions
    ): RenderResult => {
      if (!ready || !wasmRef.current) {
        return {
          svg: null,
          error: loading
            ? MERMAN_WASM_LOADING_ERROR
            : MERMAN_WASM_NOT_LOADED_ERROR,
          renderTime: 0,
        };
      }

      const startTime = performance.now();

      try {
        const svg = wasmRef.current.render_svg(
          code,
          theme,
          configJson,
          options?.pipeline
        );
        const renderTime = performance.now() - startTime;
        return { svg, error: null, renderTime };
      } catch (e) {
        return {
          svg: null,
          error: e instanceof Error ? e.message : String(e),
          renderTime: 0,
        };
      }
    },
    [ready, loading]
  );

  const validate = useCallback(
    (code: string): ValidationResult => {
      if (!ready || !wasmRef.current) {
        return { valid: false, error: MERMAN_WASM_NOT_LOADED_ERROR };
      }
      return wasmRef.current.validate(code);
    },
    [ready]
  );

  const getThemes = useCallback((): string[] => {
    if (!ready || !wasmRef.current) {
      return [...SUPPORTED_THEMES];
    }
    return wasmRef.current.get_themes();
  }, [ready]);

  const getSupportedDiagrams = useCallback((): string[] => {
    if (!ready || !wasmRef.current) {
      return [];
    }
    return wasmRef.current.get_supported_diagrams();
  }, [ready]);

  const renderAscii = useCallback(
    (
      code: string,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG
    ): string | null => {
      if (!ready || !wasmRef.current) {
        return null;
      }
      return wasmRef.current.render_ascii(code, theme, configJson);
    },
    [ready]
  );

  const parseJson = useCallback(
    (
      code: string,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG
    ): string => {
      if (!ready || !wasmRef.current) {
        throw new Error(
          loading ? MERMAN_WASM_LOADING_ERROR : MERMAN_WASM_NOT_LOADED_ERROR
        );
      }
      return wasmRef.current.parse_json(code, theme, configJson);
    },
    [ready, loading]
  );

  const layoutJson = useCallback(
    (
      code: string,
      theme = "default",
      configJson = DEFAULT_MERMAID_CONFIG
    ): string => {
      if (!ready || !wasmRef.current) {
        throw new Error(
          loading ? MERMAN_WASM_LOADING_ERROR : MERMAN_WASM_NOT_LOADED_ERROR
        );
      }
      return wasmRef.current.layout_json(code, theme, configJson);
    },
    [ready, loading]
  );

  const getAsciiSupportedDiagrams = useCallback((): string[] => {
    if (!ready || !wasmRef.current) {
      return ['flowchart', 'sequence', 'class', 'er', 'xychart'];
    }
    return wasmRef.current.get_ascii_supported_diagrams();
  }, [ready]);

  return {
    ready,
    loading,
    loadError,
    render,
    validate,
    getThemes,
    getSupportedDiagrams,
    renderAscii,
    parseJson,
    layoutJson,
    getAsciiSupportedDiagrams,
  };
}
