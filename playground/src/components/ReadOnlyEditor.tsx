import { lazy } from "react";
import type { EditorProps } from "@monaco-editor/react";

import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";

const LocalReadOnlyEditor = lazy(() =>
  import("./ReadOnlyEditorFeature").then((module) => ({
    default: module.LocalReadOnlyEditor,
  })),
);
const LocalReadOnlyJsonEditor = lazy(() =>
  import("./ReadOnlyJsonEditorFeature").then((module) => ({
    default: module.LocalReadOnlyJsonEditor,
  })),
);

interface ReadOnlyEditorProps
  extends Omit<
    EditorProps,
    "defaultLanguage" | "defaultPath" | "defaultValue" | "keepCurrentModel" | "onChange"
  > {
  readonly feature: string;
  readonly value: string;
}

export function ReadOnlyEditor({
  feature,
  language,
  options,
  ...props
}: ReadOnlyEditorProps) {
  const Editor =
    language === "json" ? LocalReadOnlyJsonEditor : LocalReadOnlyEditor;
  return (
    <LazyFeatureBoundary
      feature={feature}
      presentation={{ kind: "panel" }}
    >
      <Editor
        {...props}
        language={language}
        options={{
          ...options,
          domReadOnly: true,
          occurrencesHighlight: "off",
          readOnly: true,
        }}
      />
    </LazyFeatureBoundary>
  );
}
