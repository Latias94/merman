const { commonRules } = require('./grammar/shared/common');
const { langiumRules } = require('./grammar/shared/langium');
const { preambleRules } = require('./grammar/shared/preamble');
const {
  architectureConflicts,
  architectureRules,
} = require('./grammar/families/architecture');
const { cynefinRules } = require('./grammar/families/cynefin');
const { flowchartRules } = require('./grammar/families/flowchart');
const {
  gitGraphConflicts,
  gitGraphRules,
} = require('./grammar/families/git-graph');
const { infoRules } = require('./grammar/families/info');
const { packetRules } = require('./grammar/families/packet');
const { pieConflicts, pieRules } = require('./grammar/families/pie');
const { radarRules } = require('./grammar/families/radar');
const { sankeyRules } = require('./grammar/families/sankey');
const { vennRules } = require('./grammar/families/venn');
const { eventModelingRules } = require('./grammar/families/event-modeling');
const { zenumlRules } = require('./grammar/families/zenuml');
const { mindmapRules } = require('./grammar/families/mindmap');
const { kanbanRules } = require('./grammar/families/kanban');
const { treemapRules } = require('./grammar/families/treemap');
const { treeViewRules } = require('./grammar/families/tree-view');
const { wardleyRules } = require('./grammar/families/wardley');
const { indentationExternals } = require('./grammar/shared/indentation');
const {
  recognizedFamilyRoots,
  recognizedFamilyRules,
} = require('./grammar/families/recognized');

module.exports = grammar({
  name: 'mermaid',

  extras: ($) => [/[ \t\f\u00a0]+/],

  word: ($) => $.identifier,

  externals: ($) => indentationExternals($),

  conflicts: ($) => [
    ...architectureConflicts($),
    ...gitGraphConflicts($),
    ...pieConflicts($),
  ],

  rules: {
    source_file: ($) => seq(
      optional($.bom),
      repeat(choice($.frontmatter, $.directive, $.comment, $._blank_line)),
      optional(choice(
        $.architecture_diagram,
        $.flowchart_diagram,
        $.event_modeling_diagram,
        $.sankey_diagram,
        $.venn_diagram,
        $.zenuml_diagram,
        $.mindmap_diagram,
        $.kanban_diagram,
        $.treemap_diagram,
        $.tree_view_diagram,
        $.info_diagram,
        $.cynefin_diagram,
        $.git_graph_diagram,
        $.packet_diagram,
        $.pie_diagram,
        $.radar_diagram,
        $.wardley_diagram,
        ...recognizedFamilyRoots($),
      )),
    ),

    ...preambleRules,
    ...commonRules,
    ...langiumRules,
    ...architectureRules,
    ...infoRules,
    ...cynefinRules,
    ...gitGraphRules,
    ...packetRules,
    ...pieRules,
    ...radarRules,
    ...wardleyRules,
    ...flowchartRules,
    ...eventModelingRules,
    ...sankeyRules,
    ...vennRules,
    ...zenumlRules,
    ...mindmapRules,
    ...kanbanRules,
    ...treemapRules,
    ...treeViewRules,
    ...recognizedFamilyRules,
  },
});
