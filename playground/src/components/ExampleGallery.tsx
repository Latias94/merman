import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { categories, getExamplesByCategory, type Example } from "@/src/lib/examples";
import { useAsciiSupport } from "@/src/lib/ascii-capabilities";
import { detectDiagramType } from "@/src/lib/diagram-detection";
import { useAppStore } from "@/src/store";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { X, Code, ChevronRight } from "lucide-react";

// 分类翻译映射
const categoryKeys: Record<string, string> = {
  All: "examples.all",
  Flowchart: "examples.categories.flowchart",
  Sequence: "examples.categories.sequence",
  Class: "examples.categories.class",
  State: "examples.categories.state",
  ER: "examples.categories.er",
  Gantt: "examples.categories.gantt",
  Pie: "examples.categories.pie",
  Mindmap: "examples.categories.mindmap",
  Git: "examples.categories.git",
  Timeline: "examples.categories.timeline",
  Journey: "examples.categories.journey",
  Info: "examples.categories.info",
  ZenUML: "examples.categories.zenuml",
  "XY Chart": "examples.categories.xychart",
  Architecture: "examples.categories.architecture",
  C4: "examples.categories.c4",
  Block: "examples.categories.block",
  Packet: "examples.categories.packet",
  Kanban: "examples.categories.kanban",
  Quadrant: "examples.categories.quadrant",
  Sankey: "examples.categories.sankey",
  Radar: "examples.categories.radar",
  Treemap: "examples.categories.treemap",
  TreeView: "examples.categories.treeview",
  Requirement: "examples.categories.requirement",
};

export function ExampleGallery() {
  const { t } = useTranslation();
  const { showExamples, toggleExamples, setCode } = useAppStore();
  const asciiSupport = useAsciiSupport();
  const [selectedCategory, setSelectedCategory] = useState("All");
  const [asciiOnly, setAsciiOnly] = useState(false);

  const isExampleAsciiSupported = useMemo(
    () => (example: Example) =>
      example.asciiSupported ??
      asciiSupport.isSupported(detectDiagramType(example.code)),
    [asciiSupport]
  );

  const visibleCategories = useMemo(
    () =>
      categories.filter(
        (category) =>
          category === "All" ||
          !asciiOnly ||
          getExamplesByCategory(category).some(isExampleAsciiSupported)
      ),
    [asciiOnly, isExampleAsciiSupported]
  );

  const filteredExamples = useMemo(
    () =>
      getExamplesByCategory(selectedCategory).filter(
        (example) => !asciiOnly || isExampleAsciiSupported(example)
      ),
    [asciiOnly, isExampleAsciiSupported, selectedCategory]
  );
  const asciiReadyCount = useMemo(
    () => getExamplesByCategory("All").filter(isExampleAsciiSupported).length,
    [isExampleAsciiSupported]
  );

  useEffect(() => {
    if (!visibleCategories.includes(selectedCategory)) {
      setSelectedCategory("All");
    }
  }, [selectedCategory, visibleCategories]);

  if (!showExamples) return null;

  const toggleAsciiOnly = () => setAsciiOnly((value) => !value);

  const handleSelectExample = (code: string) => {
    setCode(code);
    toggleExamples();
  };

  const getCategoryLabel = (category: string) => {
    const key = categoryKeys[category];
    return key ? t(key) : category;
  };

  return (
    <div className="absolute inset-0 z-20 bg-background/95 backdrop-blur-sm flex flex-col">
      {/* 头部 */}
      <div className="flex items-center justify-between p-4 border-b">
        <div>
          <h2 className="text-lg font-semibold">{t("examples.title")}</h2>
          <p className="text-sm text-muted-foreground">
            {t("examples.description")}
          </p>
        </div>
        <Button variant="ghost" size="icon" onClick={toggleExamples}>
          <X className="size-5" />
        </Button>
      </div>

      <div className="flex-1 flex flex-col overflow-hidden md:flex-row">
        {/* 左侧分类 */}
        <div className="scrollbar-thin shrink-0 overflow-x-auto border-b p-2 md:w-48 md:overflow-y-auto md:border-b-0 md:border-r">
          <div className="mb-2 hidden w-full items-center gap-2 rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground md:flex">
            <Checkbox
              checked={asciiOnly}
              onCheckedChange={(checked) => setAsciiOnly(checked === true)}
            />
            <button
              type="button"
              onClick={toggleAsciiOnly}
              className="flex-1 text-left"
            >
              {t("examples.asciiOnly")}
            </button>
          </div>
          <nav className="flex gap-1 md:block md:space-y-1">
            <div className="flex shrink-0 items-center gap-2 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground md:hidden">
              <Checkbox
                checked={asciiOnly}
                onCheckedChange={(checked) => setAsciiOnly(checked === true)}
              />
              <button
                type="button"
                onClick={toggleAsciiOnly}
                className="text-left"
              >
                {t("examples.asciiOnly")}
              </button>
            </div>
            {visibleCategories.map((category) => (
              <button
                key={category}
                onClick={() => setSelectedCategory(category)}
                className={cn(
                  "flex shrink-0 items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors md:w-full",
                  selectedCategory === category
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                )}
              >
                <Code className="size-4 flex-shrink-0" />
                <span>{getCategoryLabel(category)}</span>
                {selectedCategory === category && (
                  <ChevronRight className="hidden size-4 ml-auto md:block" />
                )}
              </button>
            ))}
          </nav>
        </div>

        {/* 右侧示例列表 */}
        <ScrollArea className="flex-1">
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
          <div className="p-4 grid gap-4 grid-cols-1 md:grid-cols-2 lg:grid-cols-3">
            {filteredExamples.map((example) => (
              <button
                key={example.id}
                onClick={() => handleSelectExample(example.code)}
                className="group text-left p-4 border rounded-lg bg-card hover:border-primary/50 hover:shadow-md transition-all"
              >
                <div className="flex items-start justify-between mb-2">
                  <div>
                    <h3 className="font-medium text-sm group-hover:text-primary transition-colors">
                      {t(`examples.items.${example.id}`, {
                        defaultValue: example.name,
                      })}
                    </h3>
                    <span className="text-xs text-muted-foreground">
                      {getCategoryLabel(example.category)}
                    </span>
                  </div>
                  <div className="text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded">
                    {example.code.split("\n").length} {t("examples.lines")}
                  </div>
                </div>
                <pre className="text-xs text-muted-foreground bg-muted/50 p-2 rounded overflow-hidden max-h-24 font-mono">
                  {example.code.slice(0, 200)}
                  {example.code.length > 200 && "..."}
                </pre>
              </button>
            ))}
          </div>
        </ScrollArea>
      </div>
    </div>
  );
}
