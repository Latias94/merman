import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, Code, Search, X } from "lucide-react";

import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useAppStore } from "@/src/store";
import { useAsciiSupport } from "@/src/lib/ascii-capabilities";
import {
  asciiSupportLabelKey,
  type AsciiCapability,
} from "@/src/lib/ascii-support";
import {
  categories,
  examples,
  filterExamples,
  type Example,
} from "@/src/lib/examples";

const categoryKeys: Record<string, string> = {
  All: "examples.all",
  Software: "examples.categories.software",
  Strategy: "examples.categories.strategy",
  Data: "examples.categories.data",
  Flow: "examples.categories.flow",
  Planning: "examples.categories.planning",
  Reference: "examples.categories.reference",
  Grammar: "examples.categories.grammar",
};

export function ExampleGallery({
  open,
  onOpenChange,
  restoreFocus,
}: {
  readonly open: boolean;
  onOpenChange(open: boolean): void;
  restoreFocus(): void;
}) {
  const { t } = useTranslation();
  const setCode = useAppStore((state) => state.setCode);
  const asciiSupport = useAsciiSupport();
  const searchRef = useRef<HTMLInputElement>(null);
  const [selectedCategory, setSelectedCategory] = useState("All");
  const [query, setQuery] = useState("");
  const [asciiOnly, setAsciiOnly] = useState(false);

  const asciiDiagramTypes = useMemo(
    () => new Set(asciiSupport.supportedTypes),
    [asciiSupport.supportedTypes]
  );
  const searchableExamples = useMemo(
    () =>
      filterExamples({
        query,
        asciiOnly,
        asciiDiagramTypes,
      }),
    [asciiDiagramTypes, asciiOnly, query]
  );
  const visibleCategories = useMemo(
    () =>
      categories.filter(
        (category) =>
          category === "All" ||
          searchableExamples.some((example) => example.category === category)
      ),
    [searchableExamples]
  );
  const activeCategory = visibleCategories.includes(selectedCategory)
    ? selectedCategory
    : "All";
  const filteredExamples = useMemo(
    () =>
      filterExamples({
        category: activeCategory,
        query,
        asciiOnly,
        asciiDiagramTypes,
      }),
    [activeCategory, asciiDiagramTypes, asciiOnly, query]
  );
  const asciiReadyCount = useMemo(
    () =>
      examples.filter((example) =>
        asciiDiagramTypes.has(example.diagramType)
      ).length,
    [asciiDiagramTypes]
  );

  useEffect(() => {
    if (!visibleCategories.includes(selectedCategory)) {
      setSelectedCategory("All");
    }
  }, [selectedCategory, visibleCategories]);

  const handleSelectExample = (example: Example) => {
    setCode(example.source);
    onOpenChange(false);
  };

  const getCategoryLabel = (category: string) => {
    const key = categoryKeys[category];
    return key ? t(key) : category;
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        safeArea={false}
        showCloseButton={false}
        className="grid h-[100dvh] w-screen max-w-none grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden rounded-none border-0 p-0 sm:h-[min(90dvh,54rem)] sm:w-[calc(100vw-2rem)] sm:max-w-[76rem] sm:rounded-md sm:border"
        style={{
          paddingTop: "env(safe-area-inset-top)",
          paddingRight: "env(safe-area-inset-right)",
          paddingBottom: "env(safe-area-inset-bottom)",
          paddingLeft: "env(safe-area-inset-left)",
        }}
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          searchRef.current?.focus();
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          restoreFocus();
        }}
      >
        <DialogHeader className="border-b bg-muted/15 p-4 pr-14 text-left sm:px-5">
          <DialogTitle>{t("examples.title")}</DialogTitle>
          <DialogDescription className="sr-only">
            {t("examples.description")}
          </DialogDescription>
          <DialogClose asChild>
            <Button
              variant="ghost"
              size="icon"
              className="absolute right-3 top-3 size-10 sm:size-9"
              aria-label={t("examples.close")}
            >
              <X className="size-5" />
            </Button>
          </DialogClose>
          <div className="mt-3 flex flex-col gap-2 sm:flex-row sm:items-center">
            <div className="relative min-w-0 flex-1">
              <Search
                className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
                aria-hidden="true"
              />
              <Input
                ref={searchRef}
                type="search"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t("examples.searchPlaceholder")}
                aria-label={t("examples.searchLabel")}
                className="pl-9"
              />
            </div>
            <p
              className="shrink-0 text-xs text-muted-foreground"
              aria-live="polite"
              aria-atomic="true"
            >
              {t("examples.resultCount", { count: filteredExamples.length })}
            </p>
          </div>
        </DialogHeader>

        <div className="flex min-h-0 flex-1 flex-col overflow-hidden md:flex-row">
          <div className="shrink-0 overflow-hidden border-b p-2 md:w-48 md:overflow-y-auto md:border-b-0 md:border-r">
            <AsciiFilter
              id="ascii-only-desktop"
              asciiOnly={asciiOnly}
              onChange={setAsciiOnly}
              label={t("examples.asciiOnly")}
              className="mb-2 hidden md:flex"
            />
            <AsciiFilter
              id="ascii-only-mobile"
              asciiOnly={asciiOnly}
              onChange={setAsciiOnly}
              label={t("examples.asciiOnly")}
              className="mb-2 flex md:hidden"
            />
            <nav
              className="scrollbar-thin flex gap-1 overflow-x-auto pb-1 md:block md:space-y-1 md:overflow-visible md:pb-0"
              aria-label={t("examples.categoriesLabel")}
            >
              {visibleCategories.map((category) => (
                <button
                  key={category}
                  type="button"
                  onClick={() => setSelectedCategory(category)}
                  aria-pressed={activeCategory === category}
                  className={cn(
                    "flex shrink-0 items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors md:w-full",
                    activeCategory === category
                      ? "bg-primary text-primary-foreground"
                      : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                  )}
                >
                  <Code className="size-4 shrink-0" aria-hidden="true" />
                  <span>{getCategoryLabel(category)}</span>
                  {activeCategory === category && (
                    <ChevronRight
                      className="ml-auto hidden size-4 md:block"
                      aria-hidden="true"
                    />
                  )}
                </button>
              ))}
            </nav>
          </div>

          <ScrollArea className="min-h-0 flex-1">
            <div className="border-b px-4 py-2 text-xs text-muted-foreground">
              {asciiOnly
                ? t("examples.asciiFilterActive", {
                    count: filteredExamples.length,
                    total: asciiReadyCount,
                  })
                : t("examples.asciiFilterAvailable", {
                    count: asciiReadyCount,
                  })}
            </div>
            {filteredExamples.length === 0 ? (
              <div
                className="flex min-h-48 items-center justify-center px-6 text-center text-sm text-muted-foreground"
                role="status"
              >
                {t("examples.empty")}
              </div>
            ) : (
              <div className="grid grid-cols-1 gap-4 p-4 md:grid-cols-2 lg:grid-cols-3">
                {filteredExamples.map((example) => (
                  <button
                    key={example.id}
                    type="button"
                    onClick={() => handleSelectExample(example)}
                    className="group rounded-md border bg-card p-4 text-left transition-[border-color,box-shadow,transform] hover:border-primary/50 hover:shadow-sm active:scale-[0.99]"
                  >
                    <div className="mb-2 flex items-start justify-between gap-3">
                      <div className="min-w-0">
                        <h3 className="text-sm font-medium transition-colors group-hover:text-primary">
                          {example.title}
                        </h3>
                        <span className="text-xs text-muted-foreground">
                          {getCategoryLabel(example.category)}
                        </span>
                      </div>
                      <div className="flex shrink-0 flex-col items-end gap-1">
                        <div className="rounded bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                          {example.source.split("\n").length}{" "}
                          {t("examples.lines")}
                        </div>
                        {asciiDiagramTypes.has(example.diagramType) && (
                          <AsciiCapabilityBadge
                            capability={asciiSupport.capabilityFor(
                              example.diagramType
                            )}
                            t={t}
                          />
                        )}
                      </div>
                    </div>
                    <pre className="max-h-24 overflow-hidden rounded bg-muted/50 p-2 font-mono text-xs text-muted-foreground">
                      {example.source.slice(0, 200)}
                      {example.source.length > 200 && "..."}
                    </pre>
                  </button>
                ))}
              </div>
            )}
          </ScrollArea>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function AsciiFilter({
  id,
  asciiOnly,
  onChange,
  label,
  className,
}: {
  id: string;
  asciiOnly: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "w-full items-center gap-2 rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        className
      )}
    >
      <Checkbox
        id={id}
        checked={asciiOnly}
        onCheckedChange={(checked) => onChange(checked === true)}
      />
      <label htmlFor={id} className="flex-1 cursor-pointer text-left">
        {label}
      </label>
    </div>
  );
}

function AsciiCapabilityBadge({
  capability,
  t,
}: {
  capability: AsciiCapability | null;
  t: (key: string, options?: Record<string, unknown>) => string;
}) {
  return (
    <span className="rounded bg-muted px-2 py-0.5 text-xs text-muted-foreground">
      {t(asciiSupportLabelKey(capability))}
    </span>
  );
}
