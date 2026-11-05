import { useTranslation } from "react-i18next";
import { Toaster } from "sonner";

import { ToolbarControls } from "@/src/components/ToolbarControls";
import { ToolbarFeatureLaunchers } from "@/src/components/ToolbarFeatureLaunchers";

export function Toolbar() {
  const { t } = useTranslation();

  return (
    <>
      <Toaster position="bottom-right" richColors />
      <header className="flex h-14 shrink-0 items-center gap-1 border-b bg-card px-2 sm:gap-2 sm:px-4">
        <div className="flex min-w-0 shrink-0 items-center gap-1 xl:gap-4">
          <div className="flex items-center gap-2">
            <img
              src={`${import.meta.env.BASE_URL}icon.svg`}
              alt=""
              aria-hidden="true"
              className="size-8 rounded-md"
            />
            <div className="hidden xl:block">
              <h1 className="text-sm font-semibold leading-none">Merman</h1>
              <p className="text-xs text-muted-foreground">
                {t("app.playground")}
              </p>
            </div>
          </div>

          <div className="hidden h-6 w-px bg-border xl:block" />

          <ToolbarFeatureLaunchers />
        </div>

        <ToolbarControls />
      </header>
    </>
  );
}
