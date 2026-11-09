import flowchartSource from "./samples/flowchart.mmd?raw";
import flowchartConfig from "./samples/flowchart.config.json?raw";
import sequenceSource from "./samples/sequence.mmd?raw";
import sequenceConfig from "./samples/sequence.config.json?raw";
import classSource from "./samples/class.mmd?raw";
import classConfig from "./samples/class.config.json?raw";
import xychartSource from "./samples/xychart.mmd?raw";
import xychartConfig from "./samples/xychart.config.json?raw";
import pieSource from "./samples/pie.mmd?raw";
import pieConfig from "./samples/pie.config.json?raw";
import stateSource from "./samples/state.mmd?raw";
import stateConfig from "./samples/state.config.json?raw";
import erSource from "./samples/er.mmd?raw";
import erConfig from "./samples/er.config.json?raw";
import mindmapSource from "./samples/mindmap.mmd?raw";
import mindmapConfig from "./samples/mindmap.config.json?raw";
import gitGraphSource from "./samples/gitGraph.mmd?raw";
import gitGraphConfig from "./samples/gitGraph.config.json?raw";
import ganttSource from "./samples/gantt.mmd?raw";
import ganttConfig from "./samples/gantt.config.json?raw";
import kanbanSource from "./samples/kanban.mmd?raw";
import kanbanConfig from "./samples/kanban.config.json?raw";
import timelineSource from "./samples/timeline.mmd?raw";
import timelineConfig from "./samples/timeline.config.json?raw";
import architectureSource from "./samples/architecture.mmd?raw";
import architectureConfig from "./samples/architecture.config.json?raw";
import radarSource from "./samples/radar.mmd?raw";
import radarConfig from "./samples/radar.config.json?raw";
import quadrantChartSource from "./samples/quadrantChart.mmd?raw";
import quadrantChartConfig from "./samples/quadrantChart.config.json?raw";
import sankeySource from "./samples/sankey.mmd?raw";
import sankeyConfig from "./samples/sankey.config.json?raw";
import treemapSource from "./samples/treemap.mmd?raw";
import treemapConfig from "./samples/treemap.config.json?raw";
import requirementSource from "./samples/requirement.mmd?raw";
import requirementConfig from "./samples/requirement.config.json?raw";
import treeViewSource from "./samples/treeView.mmd?raw";
import treeViewConfig from "./samples/treeView.config.json?raw";

import type { ThemeSampleId } from "./families";
export type { ThemeSampleId } from "./families";

export const THEME_SAMPLES: readonly {
  id: ThemeSampleId;
  source: string;
  configJson: string;
}[] = [
  { id: "flowchart", source: flowchartSource, configJson: flowchartConfig },
  { id: "sequence", source: sequenceSource, configJson: sequenceConfig },
  { id: "class", source: classSource, configJson: classConfig },
  { id: "xychart", source: xychartSource, configJson: xychartConfig },
  { id: "pie", source: pieSource, configJson: pieConfig },
  { id: "state", source: stateSource, configJson: stateConfig },
  { id: "er", source: erSource, configJson: erConfig },
  { id: "mindmap", source: mindmapSource, configJson: mindmapConfig },
  { id: "gitGraph", source: gitGraphSource, configJson: gitGraphConfig },
  { id: "gantt", source: ganttSource, configJson: ganttConfig },
  { id: "kanban", source: kanbanSource, configJson: kanbanConfig },
  { id: "timeline", source: timelineSource, configJson: timelineConfig },
  { id: "architecture", source: architectureSource, configJson: architectureConfig },
  { id: "radar", source: radarSource, configJson: radarConfig },
  { id: "quadrantChart", source: quadrantChartSource, configJson: quadrantChartConfig },
  { id: "sankey", source: sankeySource, configJson: sankeyConfig },
  { id: "treemap", source: treemapSource, configJson: treemapConfig },
  { id: "requirement", source: requirementSource, configJson: requirementConfig },
  { id: "treeView", source: treeViewSource, configJson: treeViewConfig },
];

export function samplesForTheme(theme: { readonly families: readonly ThemeSampleId[]; readonly featuredSample?: ThemeSampleId }) {
  return THEME_SAMPLES.filter((sample) => theme.families.includes(sample.id))
    .toSorted((left, right) => Number(right.id === theme.featuredSample) - Number(left.id === theme.featuredSample));
}
