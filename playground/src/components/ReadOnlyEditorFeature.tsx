import Editor from "@monaco-editor/react";

import { ensureLocalMonacoConfigured } from "@/src/editor/monaco";
import { activateLocalMonacoJson } from "@/src/editor/monaco-json";

ensureLocalMonacoConfigured();
activateLocalMonacoJson();

export const LocalReadOnlyEditor = Editor;
