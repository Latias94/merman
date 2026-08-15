const { commonConflicts, commonRules } = require('./grammar/shared/common');
const {
  langiumConflicts,
  langiumRules,
} = require('./grammar/shared/langium');
const { preambleRules } = require('./grammar/shared/preamble');
const {
  architectureConflicts,
  architectureRules,
} = require('./grammar/families/architecture');
const { blockConflicts, blockRules } = require('./grammar/families/block');
const { c4Rules } = require('./grammar/families/c4');
const {
  classConflicts,
  classRules,
} = require('./grammar/families/class');
const { cynefinRules } = require('./grammar/families/cynefin');
const {
  entityRelationshipRules,
} = require('./grammar/families/entity-relationship');
const {
  flowchartConflicts,
  flowchartRules,
} = require('./grammar/families/flowchart');
const { ganttRules } = require('./grammar/families/gantt');
const {
  gitGraphConflicts,
  gitGraphRules,
} = require('./grammar/families/git-graph');
const { infoRules } = require('./grammar/families/info');
const { ishikawaRules } = require('./grammar/families/ishikawa');
const { journeyRules } = require('./grammar/families/journey');
const { packetRules } = require('./grammar/families/packet');
const { pieConflicts, pieRules } = require('./grammar/families/pie');
const { quadrantChartRules } = require('./grammar/families/quadrant-chart');
const { radarConflicts, radarRules } = require('./grammar/families/radar');
const { railroadAbnfRules } = require('./grammar/families/railroad-abnf');
const {
  railroadEbnfConflicts,
  railroadEbnfRules,
} = require('./grammar/families/railroad-ebnf');
const { railroadPegRules } = require('./grammar/families/railroad-peg');
const { railroadSharedRules } = require('./grammar/families/railroad-shared');
const { railroadRules } = require('./grammar/families/railroad');
const { requirementRules } = require('./grammar/families/requirement');
const {
  sankeyConflicts,
  sankeyRules,
} = require('./grammar/families/sankey');
const { sequenceRules } = require('./grammar/families/sequence');
const { stateConflicts, stateRules } = require('./grammar/families/state');
const {
  swimlaneConflicts,
  swimlaneRules,
} = require('./grammar/families/swimlane');
const { timelineRules } = require('./grammar/families/timeline');
const { vennRules } = require('./grammar/families/venn');
const {
  eventModelingConflicts,
  eventModelingRules,
} = require('./grammar/families/event-modeling');
const { zenumlRules } = require('./grammar/families/zenuml');
const { mindmapRules } = require('./grammar/families/mindmap');
const { kanbanRules } = require('./grammar/families/kanban');
const { treemapRules } = require('./grammar/families/treemap');
const { treeViewRules } = require('./grammar/families/tree-view');
const { wardleyRules } = require('./grammar/families/wardley');
const {
  xyChartConflicts,
  xyChartRules,
} = require('./grammar/families/xy-chart');
const { indentationExternals } = require('./grammar/shared/indentation');

module.exports = grammar({
  name: 'mermaid',

  extras: ($) => [/[ \t\f\u00a0]+/],

  word: ($) => $.identifier,

  externals: ($) => [
    ...indentationExternals($),
    $._end_of_input,
  ],

  conflicts: ($) => [
    ...commonConflicts($),
    ...langiumConflicts($),
    ...architectureConflicts($),
    ...blockConflicts($),
    ...classConflicts($),
    ...eventModelingConflicts($),
    ...flowchartConflicts($),
    ...gitGraphConflicts($),
    ...pieConflicts($),
    ...radarConflicts($),
    ...railroadEbnfConflicts($),
    ...sankeyConflicts($),
    ...stateConflicts($),
    ...swimlaneConflicts($),
    ...xyChartConflicts($),
  ],

  rules: {
    source_file: ($) => seq(
      optional($.bom),
      repeat(choice($.frontmatter, $.directive, $.comment, $._blank_line)),
      optional(choice(
        $.architecture_diagram,
        $.block_diagram,
        $.c4_diagram,
        $.class_diagram,
        $.entity_relationship_diagram,
        $.flowchart_diagram,
        $.event_modeling_diagram,
        $.sequence_diagram,
        $.sankey_diagram,
        $.venn_diagram,
        $.zenuml_diagram,
        $.mindmap_diagram,
        $.kanban_diagram,
        $.treemap_diagram,
        $.tree_view_diagram,
        $.info_diagram,
        $.cynefin_diagram,
        $.gantt_diagram,
        $.git_graph_diagram,
        $.ishikawa_diagram,
        $.journey_diagram,
        $.packet_diagram,
        $.pie_diagram,
        $.quadrant_chart_diagram,
        $.radar_diagram,
        $.railroad_diagram,
        $.railroad_abnf_diagram,
        $.railroad_ebnf_diagram,
        $.railroad_peg_diagram,
        $.requirement_diagram,
        $.state_diagram,
        $.swimlane_diagram,
        $.timeline_diagram,
        $.wardley_diagram,
        $.xy_chart_diagram,
      )),
    ),

    ...preambleRules,
    ...commonRules,
    ...langiumRules,
    ...architectureRules,
    ...blockRules,
    ...c4Rules,
    ...classRules,
    ...entityRelationshipRules,
    ...infoRules,
    ...cynefinRules,
    ...ganttRules,
    ...gitGraphRules,
    ...ishikawaRules,
    ...journeyRules,
    ...packetRules,
    ...pieRules,
    ...quadrantChartRules,
    ...radarRules,
    ...railroadSharedRules,
    ...railroadRules,
    ...railroadAbnfRules,
    ...railroadEbnfRules,
    ...railroadPegRules,
    ...requirementRules,
    ...stateRules,
    ...swimlaneRules,
    ...timelineRules,
    ...wardleyRules,
    ...xyChartRules,
    ...flowchartRules,
    ...eventModelingRules,
    ...sequenceRules,
    ...sankeyRules,
    ...vennRules,
    ...zenumlRules,
    ...mindmapRules,
    ...kanbanRules,
    ...treemapRules,
    ...treeViewRules,
  },
});
