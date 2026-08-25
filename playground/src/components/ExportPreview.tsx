import { useLayoutEffect, useState } from "react";
import { ImageIcon, Loader2 } from "lucide-react";

export function ExportPreview({
  blob,
  busy,
  label,
}: {
  blob: Blob | null;
  busy: boolean;
  label: string;
}) {
  const [preview, setPreview] = useState<{
    readonly blob: Blob;
    readonly url: string;
  } | null>(null);

  useLayoutEffect(() => {
    if (!blob) {
      setPreview(null);
      return;
    }
    const url = URL.createObjectURL(blob);
    setPreview({ blob, url });
    return () => {
      URL.revokeObjectURL(url);
    };
  }, [blob]);

  const url = preview?.blob === blob ? preview.url : null;

  return (
    <div
      className="relative flex min-h-[220px] flex-1 items-center justify-center overflow-hidden bg-[linear-gradient(45deg,var(--preview-grid)_25%,transparent_25%),linear-gradient(-45deg,var(--preview-grid)_25%,transparent_25%),linear-gradient(45deg,transparent_75%,var(--preview-grid)_75%),linear-gradient(-45deg,transparent_75%,var(--preview-grid)_75%)] bg-[length:20px_20px] bg-[position:0_0,0_10px,10px_-10px,-10px_0px] p-4 sm:min-h-[300px]"
      aria-busy={busy}
    >
      {url ? (
        <img
          src={url}
          alt={label}
          className="max-h-full max-w-full object-contain"
        />
      ) : (
        <ImageIcon className="size-10 text-muted-foreground/50" aria-hidden="true" />
      )}
      {busy && (
        <div className="absolute inset-0 flex items-center justify-center bg-background/65 backdrop-blur-[1px]">
          <Loader2 className="size-6 animate-spin text-foreground" aria-hidden="true" />
        </div>
      )}
    </div>
  );
}
