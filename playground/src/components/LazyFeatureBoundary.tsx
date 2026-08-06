import {
  Component,
  Suspense,
  type ErrorInfo,
  type ReactNode,
} from "react";
import { LoaderCircle, RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type LazyFeaturePresentation =
  | { readonly kind: "panel" }
  | {
      readonly kind: "dialog";
      readonly open: boolean;
      onOpenChange(open: boolean): void;
      restoreFocus(): void;
    };

interface LazyFeatureBoundaryProps {
  readonly children: ReactNode;
  readonly feature: string;
  readonly presentation: LazyFeaturePresentation;
}

interface FeatureErrorBoundaryProps {
  readonly children: ReactNode;
  readonly failure: ReactNode;
}

interface FeatureErrorBoundaryState {
  readonly failed: boolean;
}

class FeatureErrorBoundary extends Component<
  FeatureErrorBoundaryProps,
  FeatureErrorBoundaryState
> {
  state: FeatureErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): FeatureErrorBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: unknown, info: ErrorInfo): void {
    console.error("Lazy feature failed to load.", error, info.componentStack);
  }

  render(): ReactNode {
    return this.state.failed ? this.props.failure : this.props.children;
  }
}

export function LazyFeatureBoundary({
  children,
  feature,
  presentation,
}: LazyFeatureBoundaryProps) {
  return (
    <FeatureErrorBoundary
      failure={
        <FeatureLoadState
          failed
          feature={feature}
          presentation={presentation}
        />
      }
    >
      <Suspense
        fallback={
          <FeatureLoadState
            failed={false}
            feature={feature}
            presentation={presentation}
          />
        }
      >
        {children}
      </Suspense>
    </FeatureErrorBoundary>
  );
}

function FeatureLoadState({
  failed,
  feature,
  presentation,
}: {
  readonly failed: boolean;
  readonly feature: string;
  readonly presentation: LazyFeaturePresentation;
}) {
  const { t } = useTranslation();
  const title = failed
    ? t("features.loadFailed", { feature })
    : t("features.loading", { feature });
  const body = (
    <div
      role={failed ? "alert" : "status"}
      aria-live={failed ? "assertive" : "polite"}
      className="flex min-h-32 flex-col items-center justify-center gap-3 px-5 py-8 text-center"
    >
      {failed ? (
        <>
          <p className="text-sm font-medium">{title}</p>
          <p className="max-w-md text-xs text-muted-foreground">
            {t("features.reloadDescription")}
          </p>
          <Button type="button" onClick={() => window.location.reload()}>
            <RotateCcw className="size-4" />
            {t("features.reload")}
          </Button>
        </>
      ) : (
        <>
          <LoaderCircle className="size-5 animate-spin" aria-hidden="true" />
          <p className="text-sm text-muted-foreground">{title}</p>
        </>
      )}
    </div>
  );

  if (presentation.kind === "panel") {
    return <div className="flex min-h-0 flex-1">{body}</div>;
  }

  return (
    <Dialog open={presentation.open} onOpenChange={presentation.onOpenChange}>
      <DialogContent
        className="max-w-md p-0"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          presentation.restoreFocus();
        }}
      >
        <DialogHeader className="sr-only">
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            {failed ? t("features.reloadDescription") : title}
          </DialogDescription>
        </DialogHeader>
        {body}
      </DialogContent>
    </Dialog>
  );
}
