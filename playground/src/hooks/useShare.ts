import { useCallback, useEffect, useState } from "react";

interface ShareData {
  code: string;
  theme: string;
  config?: string;
  hostThemePreset?: string;
}

/**
 * 压缩并编码数据为 URL 安全的字符串
 */
function encode(data: ShareData): string {
  const json = JSON.stringify(data);
  // 使用 Base64 编码，URL 安全
  const base64 = btoa(encodeURIComponent(json));
  return base64;
}

/**
 * 解码 URL 中的数据
 */
function decode(hash: string): ShareData | null {
  try {
    const base64 = hash.startsWith("#") ? hash.slice(1) : hash;
    if (!base64) return null;
    const json = decodeURIComponent(atob(base64));
    return JSON.parse(json);
  } catch {
    return null;
  }
}

export function useShare() {
  const [initialData, setInitialData] = useState<ShareData | null>(null);

  // 页面加载时检查 URL hash
  useEffect(() => {
    const hash = window.location.hash;
    if (hash) {
      const data = decode(hash);
      if (data) {
        setInitialData(data);
      }
    }
  }, []);

  const createShareUrl = useCallback((
    code: string,
    theme: string,
    config?: string,
    hostThemePreset?: string
  ): string => {
    const encoded = encode({ code, theme, config, hostThemePreset });
    const baseUrl = `${window.location.origin}${window.location.pathname}`;
    return `${baseUrl}#${encoded}`;
  }, []);

  const copyShareUrl = useCallback(
    async (
      code: string,
      theme: string,
      config?: string,
      hostThemePreset?: string
    ): Promise<void> => {
      const url = createShareUrl(code, theme, config, hostThemePreset);
      await navigator.clipboard.writeText(url);
      // 更新 URL 但不刷新页面
      window.history.replaceState(null, "", url);
    },
    [createShareUrl]
  );

  const clearShareUrl = useCallback(() => {
    const baseUrl = `${window.location.origin}${window.location.pathname}`;
    window.history.replaceState(null, "", baseUrl);
  }, []);

  return {
    initialData,
    createShareUrl,
    copyShareUrl,
    clearShareUrl,
  };
}
