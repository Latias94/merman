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

export type ThemeSampleId = "flowchart" | "sequence" | "class" | "xychart" | "pie";

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
];
