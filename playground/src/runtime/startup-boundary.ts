export type StartupActivationReason =
  | "preview-presented"
  | "editor-intent";

export interface StartupBoundary {
  activate(reason: StartupActivationReason): boolean;
  reason(): StartupActivationReason | null;
  wait(): Promise<StartupActivationReason>;
}

export function createStartupBoundary(): StartupBoundary {
  let activationReason: StartupActivationReason | null = null;
  let resolveActivation: (
    reason: StartupActivationReason,
  ) => void = () => undefined;
  const activation = new Promise<StartupActivationReason>((resolve) => {
    resolveActivation = resolve;
  });

  return Object.freeze({
    activate(reason: StartupActivationReason) {
      if (activationReason !== null) return false;
      activationReason = reason;
      resolveActivation(reason);
      return true;
    },
    reason() {
      return activationReason;
    },
    wait() {
      return activationReason === null
        ? activation
        : Promise.resolve(activationReason);
    },
  });
}

export const playgroundStartupBoundary = createStartupBoundary();
