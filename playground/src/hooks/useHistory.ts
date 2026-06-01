import { useState, useCallback, useEffect } from "react";

export interface HistoryItem {
  id: string;
  code: string;
  theme: string;
  timestamp: number;
  name?: string;
}

const HISTORY_KEY = "merman-history";
const MAX_HISTORY = 30;

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
}

function loadHistory(): HistoryItem[] {
  try {
    const stored = localStorage.getItem(HISTORY_KEY);
    return stored ? JSON.parse(stored) : [];
  } catch {
    return [];
  }
}

function saveHistory(history: HistoryItem[]): void {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(history));
  } catch (e) {
    console.warn("Failed to save history to localStorage:", e);
  }
}

export function useHistory() {
  const [history, setHistory] = useState<HistoryItem[]>(() => loadHistory());

  // 同步到 localStorage
  useEffect(() => {
    saveHistory(history);
  }, [history]);

  const addToHistory = useCallback(
    (code: string, theme: string, name?: string) => {
      if (!code.trim()) return;

      setHistory((prev) => {
        // 检查是否已存在相同代码
        const existingIndex = prev.findIndex((h) => h.code === code);
        if (existingIndex !== -1) {
          // 更新已存在的记录
          const updated = [...prev];
          updated[existingIndex] = {
            ...updated[existingIndex],
            theme,
            timestamp: Date.now(),
            name: name || updated[existingIndex].name,
          };
          // 移到最前面
          const [item] = updated.splice(existingIndex, 1);
          return [item, ...updated];
        }

        // 添加新记录
        const newItem: HistoryItem = {
          id: generateId(),
          code,
          theme,
          timestamp: Date.now(),
          name,
        };

        return [newItem, ...prev].slice(0, MAX_HISTORY);
      });
    },
    []
  );

  const removeFromHistory = useCallback((id: string) => {
    setHistory((prev) => prev.filter((h) => h.id !== id));
  }, []);

  const clearHistory = useCallback(() => {
    setHistory([]);
  }, []);

  const renameHistory = useCallback((id: string, name: string) => {
    setHistory((prev) =>
      prev.map((h) => (h.id === id ? { ...h, name } : h))
    );
  }, []);

  return {
    history,
    addToHistory,
    removeFromHistory,
    clearHistory,
    renameHistory,
  };
}
