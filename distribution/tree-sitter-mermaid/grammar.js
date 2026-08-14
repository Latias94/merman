const { commonRules } = require('./grammar/shared/common');
const { preambleRules } = require('./grammar/shared/preamble');
const { flowchartRules } = require('./grammar/families/flowchart');
const { sankeyRules } = require('./grammar/families/sankey');
const { vennRules } = require('./grammar/families/venn');
const { eventModelingRules } = require('./grammar/families/event-modeling');
const { zenumlRules } = require('./grammar/families/zenuml');
const { mindmapRules } = require('./grammar/families/mindmap');
const { kanbanRules } = require('./grammar/families/kanban');
const { treemapRules } = require('./grammar/families/treemap');
const { treeViewRules } = require('./grammar/families/tree-view');
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

  rules: {
    source_file: ($) => seq(
      optional($.bom),
      repeat(choice($.frontmatter, $.directive, $.comment, $._blank_line)),
      optional(choice(
        $.flowchart_diagram,
        $.event_modeling_diagram,
        $.sankey_diagram,
        $.venn_diagram,
        $.zenuml_diagram,
        $.mindmap_diagram,
        $.kanban_diagram,
        $.treemap_diagram,
        $.tree_view_diagram,
        ...recognizedFamilyRoots($),
      )),
    ),

    ...preambleRules,
    ...commonRules,
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
