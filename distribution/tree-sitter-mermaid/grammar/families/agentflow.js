// Source translation: Mermaid 12.0.0
// packages/mermaid/src/diagrams/agentflow/parser/agentflow.jison
// commit 98a0945418c76238f15df2afaddbba4272656c3b.
//
// Agentflow shares Flowchart's vertices and presentation commands, but owns its
// container vocabulary and edge operators. Shape data is YAML, including block
// scalars and indentation-sensitive mappings. Preserve that scoped payload without
// pretending to parse a subset of YAML; metadata and graph validation stay in Core.

const {
  createFlowFamilyConflicts,
  createFlowFamilyRules,
} = require('./flowchart');

const sharedRules = createFlowFamilyRules({
  prefix: 'agentflow',
  diagram: 'agentflow_diagram',
  header: 'agentflow_header',
  headerEof: '_agentflow_header_eof',
  keywords: ['agentflow-beta'],
});

const keyword = ($, value) => field(
  'keyword',
  alias(token(prec(30, value)), $.agentflow_statement_keyword),
);

const containerIdentity = ($) => seq(
  field('id', choice(
    alias($.agentflow_identifier, $.agentflow_node_id),
    $.agentflow_quoted_label,
    $.agentflow_markdown_label,
    $.agentflow_container_title,
  )),
  optional(field('label', $.agentflow_square_label)),
  optional(field('data', $.agentflow_shape_data)),
);

const agentflowRules = {
  ...sharedRules,

  agentflow_statement: ($) => choice(
    $.agentflow_flow_statement,
    $.agentflow_global_statement,
    $.agentflow_connector_statement,
    $.agentflow_edge_statement,
    $.agentflow_node_statement,
    $.agentflow_direction_statement,
    $.agentflow_class_definition_statement,
    $.agentflow_class_assignment_statement,
    $.agentflow_style_statement,
    $.agentflow_link_style_statement,
    $.agentflow_click_statement,
    $.agentflow_accessibility_title_statement,
    $.agentflow_accessibility_description_statement,
  ),

  agentflow_flow_statement: ($) => prec.right(50, seq(
    keyword($, 'flow'),
    optional(containerIdentity($)),
    field('terminator', $._statement_terminator),
    repeat($.agentflow_line_item),
    field('end', $.agentflow_end_statement),
  )),

  agentflow_global_statement: ($) => prec.right(50, seq(
    keyword($, 'global'),
    field('terminator', $._statement_terminator),
    repeat($.agentflow_line_item),
    field('end', $.agentflow_end_statement),
  )),

  agentflow_connector_statement: ($) => seq(
    keyword($, 'connector'),
    containerIdentity($),
  ),

  agentflow_container_title: (_) => token(prec(
    10,
    /[^;%\[\]@\s\r\n](?:[^;%\[\]@\r\n]*[ \t][^;%\[\]@\s\r\n][^;%\[\]@\r\n]*)/,
  )),

  agentflow_end_statement: ($) => keyword($, 'end'),

  agentflow_identifier: ($) => choice(
    sharedRules.agentflow_identifier($),
    token(prec(
      40,
      /(?:flow|connector|global)(?:[A-Za-z0-9_.!?$%#\u00c0-\uffff]|-[A-Za-z0-9_\u00c0-\uffff])+/,
    )),
  ),

  agentflow_arrow_start: (_) => token(prec(20, '--')),
  agentflow_continued_arrow_start: (_) => token(prec(25, /(?:\r\n|\n|\r)[ \t]*--/)),
  agentflow_arrow: (_) => token(prec(20, /(?:--+[x>]|-\.+-)/)),
  agentflow_continued_arrow: (_) => token(prec(25, /(?:\r\n|\n|\r)[ \t]*(?:--+[x>]|-\.+-)/)),

  agentflow_shape_data: ($) => seq(
    field('open', '@{'),
    optional(field('content', $.agentflow_metadata_yaml)),
    field('close', '}'),
  ),

  // The upstream shapeData/shapeDataStr lexer preserves the YAML buffer and
  // protects closing braces inside double-quoted strings. Apostrophes remain
  // ordinary payload characters, including in plain scalars such as "don't".
  agentflow_metadata_yaml: ($) => repeat1(choice(
    $.agentflow_metadata_string,
    $.agentflow_metadata_content,
  )),

  agentflow_metadata_string: (_) => token(prec(30,
    seq('"', /(?:[^"\\]|\\.)*/, '"'),
  )),

  agentflow_metadata_content: (_) => token(prec(-20, /[^}"]+/)),
};

const agentflowConflicts = ($) => createFlowFamilyConflicts($, 'agentflow');

module.exports = { agentflowConflicts, agentflowRules };
