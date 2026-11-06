import Editor from "@monaco-editor/react";
import "monaco-editor/languages/definitions/rust/register";
import "monaco-editor/languages/definitions/javascript/register";
import "monaco-editor/languages/definitions/shell/register";
import "monaco-editor/languages/definitions/python/register";
import "monaco-editor/languages/definitions/ini/register";
import "monaco-editor/languages/definitions/html/register";

import { ensureLocalMonacoConfigured } from "@/src/editor/monaco";

ensureLocalMonacoConfigured();

export const LocalReadOnlyEditor = Editor;
