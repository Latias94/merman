export function detectDiagramType(source: string): string {
  const firstLine = source.trim().split("\n")[0]?.toLowerCase() || "";
  if (firstLine.startsWith("flowchart") || firstLine.startsWith("graph")) {
    return "flowchart";
  }
  if (firstLine.startsWith("sequencediagram")) return "sequence";
  if (firstLine.startsWith("classdiagram")) return "class";
  if (firstLine.startsWith("statediagram")) return "state";
  if (firstLine.startsWith("erdiagram")) return "er";
  if (firstLine.startsWith("gantt")) return "gantt";
  if (firstLine.startsWith("pie")) return "pie";
  if (firstLine.startsWith("mindmap")) return "mindmap";
  if (firstLine.startsWith("gitgraph")) return "gitgraph";
  if (firstLine.startsWith("timeline")) return "timeline";
  if (firstLine.startsWith("journey")) return "journey";
  if (firstLine.startsWith("info")) return "info";
  if (firstLine.startsWith("zenuml")) return "zenuml";
  if (firstLine.startsWith("eventmodeling")) return "eventmodeling";
  if (firstLine.startsWith("c4")) return "c4";
  if (firstLine.startsWith("xychart")) return "xychart";
  if (firstLine.startsWith("architecture")) return "architecture";
  if (firstLine.startsWith("block")) return "block";
  if (firstLine.startsWith("packet")) return "packet";
  if (firstLine.startsWith("kanban")) return "kanban";
  if (firstLine.startsWith("quadrantchart")) return "quadrantchart";
  if (firstLine.startsWith("sankey")) return "sankey";
  if (firstLine.startsWith("radar")) return "radar";
  if (firstLine.startsWith("treemap")) return "treemap";
  if (firstLine.startsWith("treeview-beta")) return "treeView";
  if (firstLine.startsWith("requirementdiagram")) return "requirement";
  return "unknown";
}
