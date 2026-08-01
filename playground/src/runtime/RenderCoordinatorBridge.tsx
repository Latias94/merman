import { useEffect, useMemo } from "react";
import { useShallow } from "zustand/react/shallow";

import { useAppStore } from "../store";
import { setRenderCoordinatorInput } from "./render-coordinator-browser.ts";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "./use-merman-runtime.ts";

export function RenderCoordinatorBridge() {
  const {
    code,
    diagramFont,
    diagramTheme,
    hostThemePreset,
    mermaidConfig,
    textMeasurementMode,
  } = useAppStore(
    useShallow((state) => ({
      code: state.code,
      diagramFont: state.diagramFont,
      diagramTheme: state.diagramTheme,
      hostThemePreset: state.hostThemePreset,
      mermaidConfig: state.mermaidConfig,
      textMeasurementMode: state.textMeasurementMode,
    }))
  );
  const facade = useMermanRuntime(selectMermanFacade);
  const options = useMemo(
    () => ({
      diagramFont,
      hostThemePreset:
        hostThemePreset === "none" ? undefined : hostThemePreset,
      textMeasurementMode,
    }),
    [diagramFont, hostThemePreset, textMeasurementMode]
  );

  useEffect(() => {
    setRenderCoordinatorInput({
      facade,
      source: code,
      theme: diagramTheme,
      configJson: mermaidConfig,
      options,
    });
  }, [code, diagramTheme, facade, mermaidConfig, options]);

  return null;
}
