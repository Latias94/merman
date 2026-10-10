import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, FileUp, Plus } from "lucide-react";
import { toast } from "sonner";
import type { ThemeRecipeV1 } from "@mermanjs/web";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { downloadBlob } from "@/src/lib/export";
import { utf8ByteLength } from "@/src/lib/utf8";
import { SHARE_THEME_RECIPE_BYTES } from "@/src/lib/share";
import { projectError, type ErrorProjection } from "@/src/runtime/error-projection";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "@/src/runtime/use-merman-runtime";
import { useAppStore } from "@/src/store";

const NEW_THEME: ThemeRecipeV1 = {
  schema_version: 1,
  kind: "definition",
  definition: {
    authoring_schema_version: 1,
    expansion_version: 1,
    tokens: {
      canvas: "#fffaf0",
      surface: "#ffffff",
      text: "#202020",
      border: "#202020",
      accent: "#f16b50",
    },
  },
};

// Mounted for each editing session so cancelling never changes the workspace.
export function ThemeEditorDialog({ onClose, restoreFocus }: {
  onClose(): void;
  restoreFocus(): void;
}) {
  const { t } = useTranslation();
  const editorId = useId();
  const facade = useMermanRuntime(selectMermanFacade);
  const themePresetId = useAppStore((state) => state.themePresetId);
  const [draft, setDraft] = useState(() =>
    useAppStore.getState().themeRecipeJson ?? JSON.stringify(NEW_THEME, null, 2),
  );
  const [error, setError] = useState<ErrorProjection | null>(null);
  const [importing, setImporting] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);
  const importGeneration = useRef(0);

  useEffect(() => () => { importGeneration.current += 1; }, []);

  function replaceDraft(value: string) {
    importGeneration.current += 1;
    setImporting(false);
    setDraft(value);
    setError(null);
  }

  function checkSize(bytes: number) {
    if (bytes > SHARE_THEME_RECIPE_BYTES) {
      throw new Error(t("customTheme.tooLarge", {
        limit: SHARE_THEME_RECIPE_BYTES / 1024,
      }));
    }
  }

  function validatedRecipe(): string {
    if (!facade) throw new Error(t("wasm.notLoaded"));
    checkSize(utf8ByteLength(draft));
    const recipe = facade.validateThemeRecipe(draft);
    checkSize(utf8ByteLength(recipe));
    return recipe;
  }

  function apply() {
    try {
      const recipe = validatedRecipe();
      useAppStore.getState().setThemeRecipeJson(recipe);
      toast.success(t("customTheme.applied"));
      onClose();
    } catch (cause) {
      setError(projectError(cause));
    }
  }

  function download() {
    try {
      const recipe = validatedRecipe();
      downloadBlob(
        new Blob([recipe], { type: "application/json;charset=utf-8" }),
        "merman-theme.json",
      );
      setError(null);
    } catch (cause) {
      setError(projectError(cause));
    }
  }

  function usePreset() {
    if (!facade || !themePresetId) return;
    try {
      replaceDraft(JSON.stringify(facade.exportThemePreset(themePresetId), null, 2));
    } catch (cause) {
      setError(projectError(cause));
    }
  }

  async function importFile(file: File) {
    const generation = ++importGeneration.current;
    try {
      checkSize(file.size);
      setImporting(true);
      const text = await file.text();
      if (generation !== importGeneration.current) return;
      replaceDraft(text);
    } catch (cause) {
      if (generation !== importGeneration.current) return;
      setError(projectError(cause));
      setImporting(false);
    }
  }

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent
        className="max-h-[calc(100dvh-2rem)] min-w-0 overflow-y-auto sm:max-w-3xl"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          restoreFocus();
        }}
      >
        <DialogHeader className="min-w-0 pr-6">
          <DialogTitle>{t("customTheme.title")}</DialogTitle>
          <DialogDescription>{t("customTheme.description")}</DialogDescription>
        </DialogHeader>
        <div className="flex min-w-0 flex-wrap gap-2">
          <Button variant="outline" size="sm" onClick={() => replaceDraft(JSON.stringify(NEW_THEME, null, 2))}>
            <Plus className="size-4" />{t("customTheme.new")}
          </Button>
          {themePresetId && (
            <Button variant="outline" size="sm" disabled={!facade} onClick={usePreset}>
              {t("customTheme.usePreset")}
            </Button>
          )}
          <Button variant="outline" size="sm" onClick={() => fileInput.current?.click()}>
            <FileUp className="size-4" />{t("customTheme.import")}
          </Button>
          <Button variant="outline" size="sm" disabled={!facade || importing} onClick={download}>
            <Download className="size-4" />{t("customTheme.download")}
          </Button>
          <input
            ref={fileInput}
            type="file"
            accept=".json,application/json"
            className="hidden"
            aria-label={t("customTheme.import")}
            onChange={(event) => {
              const file = event.target.files?.[0];
              event.target.value = "";
              if (file) void importFile(file);
            }}
          />
        </div>
        <div className="min-w-0 space-y-2">
          <label htmlFor={editorId} className="text-sm font-medium">{t("customTheme.recipeLabel")}</label>
          <textarea
            id={editorId}
            value={draft}
            onChange={(event) => replaceDraft(event.target.value)}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            className="block h-64 min-h-40 w-full min-w-0 resize-y rounded-md border bg-background p-3 font-mono text-xs leading-relaxed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring sm:h-80"
            aria-describedby={`${editorId}-help`}
            aria-invalid={error !== null}
          />
          <p id={`${editorId}-help`} className="text-xs text-muted-foreground">{t("customTheme.help")}</p>
          {importing && <p role="status" className="text-sm">{t("customTheme.importing")}</p>}
          {error && (
            <div role="alert" className="min-w-0 space-y-2 rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
              <p className="whitespace-pre-wrap break-words [overflow-wrap:anywhere]">{error.summary}</p>
              {error.detail && (
                <details>
                  <summary className="cursor-pointer">{t("customTheme.diagnosticDetails")}</summary>
                  <pre className="mt-2 max-h-36 overflow-y-auto whitespace-pre-wrap break-words text-xs [overflow-wrap:anywhere]">{error.detail}</pre>
                </details>
              )}
              <p>{t("customTheme.unchanged")}</p>
            </div>
          )}
        </div>
        <p className="text-xs text-muted-foreground">{t("customTheme.limitations")}</p>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("customTheme.cancel")}</Button>
          <Button disabled={!facade || importing} onClick={apply}>{t("customTheme.apply")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
