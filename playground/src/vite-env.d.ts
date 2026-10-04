/// <reference types="vite/client" />

declare const __PLAYGROUND_BUILD__: Readonly<{
  channel: "main" | "preview" | "tag" | "local";
  commit: string | null;
  ref: string | null;
  dirty: boolean;
  repository: string | null;
}>;
