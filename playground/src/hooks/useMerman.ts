import { useEffect, useState, useCallback, useRef } from "react";
import {
  loadWasm,
  SUPPORTED_THEMES,
  type MermanWasm,
  type ValidationResult,
} from "@/src/lib/wasm-loader";

export interface RenderResult {
  svg: string | null;
  error: string | null;
  renderTime: number;
}

export function useMerman() {
  const [ready, setReady] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const wasmRef = useRef<MermanWasm | null>(null);

  useEffect(() => {
    let mounted = true;

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
    (code: string, theme: string): RenderResult => {
      if (!ready || !wasmRef.current) {
        return {
          svg: null,
          error: loading ? "WASM 模块加载中..." : "WASM 模块未加载",
          renderTime: 0,
        };
      }

      const startTime = performance.now();

      try {
        const svg = wasmRef.current.render_svg(code, theme);
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
        return { valid: false, error: "WASM 模块未加载" };
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
    (code: string): string | null => {
      if (!ready || !wasmRef.current) {
        return null;
      }
      return wasmRef.current.render_ascii(code);
    },
    [ready]
  );

  const getAsciiSupportedDiagrams = useCallback((): string[] => {
    if (!ready || !wasmRef.current) {
      return ['flowchart', 'sequence', 'class', 'state', 'er'];
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
    getAsciiSupportedDiagrams,
  };
}
