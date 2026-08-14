function flowchartKeyword($) {
  return field(
    'keyword',
    alias(
      token(prec(20, choice('flowchart-elk', 'flowchart', 'graph'))),
      $.diagram_keyword,
    ),
  );
}

function flowchartHeaderDirection($) {
  return seq(
    token.immediate(/[ \t]+/),
    alias(token.immediate(/(?:LR|RL|TB|BT|TD|BR|[<>^v])/), $.direction),
  );
}

const flowchartRules = {
  flowchart_diagram: ($) => choice(
    seq(
      field('header', $.flowchart_header),
      optional(field('body', $.flowchart_body)),
    ),
    field('header', alias($._flowchart_eof_header, $.flowchart_header)),
  ),

  flowchart_header: ($) => choice(
    seq(
      flowchartKeyword($),
      field('direction', flowchartHeaderDirection($)),
      field('terminator', $._statement_terminator),
    ),
    seq(
      flowchartKeyword($),
      field('terminator', $._line_ending),
    ),
  ),

  _flowchart_eof_header: ($) => seq(
    flowchartKeyword($),
    optional(field('direction', flowchartHeaderDirection($))),
  ),

  flowchart_body: ($) => repeat1($._flowchart_item),

  _flowchart_item: ($) => choice(
    $.comment,
    $._blank_line,
    $.flow_subgraph,
    $.flow_edge_statement,
    $.flow_incomplete_edge_statement,
    $.flow_node_statement,
    $.flow_direction_statement,
  ),

  _flow_subgraph_item: ($) => choice(
    $.comment,
    $._blank_line,
    $.flow_subgraph,
    $.flow_edge_statement,
    $.flow_incomplete_edge_statement,
    $.flow_node_statement,
    $.flow_direction_statement,
  ),

  flow_direction_statement: ($) => seq(
    field('clause', $.flow_direction_clause),
    optional(field('trailing', $.flow_direction_trailing_text)),
    $._statement_terminator,
  ),

  flow_direction_clause: (_) => token(prec(
    20,
    /direction[ \t]+(?:LR|RL|TB|BT|TD)/,
  )),

  flow_direction_trailing_text: (_) => token(prec(-100, /[^;\r\n]+/)),

  flow_subgraph: ($) => prec.right(seq(
    field('keyword', 'subgraph'),
    optional(field('id', alias($._flow_identifier, $.identifier))),
    optional(field('label', choice($.flow_square_label, $.quoted_string))),
    $._statement_terminator,
    repeat($._flow_subgraph_item),
    field('end', $.flow_subgraph_end),
    optional($._statement_terminator),
  )),

  flow_subgraph_end: (_) => 'end',

  flow_edge_statement: ($) => seq(
    field('source', $.flow_node),
    field('edge', $.flow_edge),
    field('target', $.flow_node),
    repeat(seq(field('edge', $.flow_edge), field('target', $.flow_node))),
    $._statement_terminator,
  ),

  flow_incomplete_edge_statement: ($) => seq(
    field('source', $.flow_node),
    field('edge', $.flow_edge),
    field('recovery', $.flow_edge_recovery),
  ),

  flow_edge_recovery: ($) => seq(
    optional(field('text', $.flow_edge_recovery_text)),
    $._line_ending,
  ),

  flow_edge_recovery_text: (_) => token(prec(-100, /[^\r\n]+/)),

  flow_node_statement: ($) => seq(
    field('node', $.flow_node),
    $._statement_terminator,
  ),

  flow_node: ($) => seq(
    field('id', alias($._flow_identifier, $.identifier)),
    optional(field('shape', $.flow_shape)),
    optional(field('class', $.flow_class_annotation)),
  ),

  flow_shape: ($) => choice(
    $.flow_square_label,
    $.flow_round_label,
    $.flow_circle_label,
    $.flow_diamond_label,
    $.flow_hexagon_label,
  ),

  flow_square_label: ($) => seq('[', field('text', optional($.flow_label_text)), ']'),

  flow_round_label: ($) => seq('(', field('text', optional($.flow_label_text)), ')'),

  flow_circle_label: ($) => seq('((', field('text', optional($.flow_label_text)), '))'),

  flow_diamond_label: ($) => seq('{', field('text', optional($.flow_label_text)), '}'),

  flow_hexagon_label: ($) => seq('{{', field('text', optional($.flow_label_text)), '}}'),

  flow_label_text: (_) => token(prec(-5, /[^\]\)\}|\r\n]+/)),

  flow_edge: ($) => seq(
    optional(field('id', $.flow_edge_id)),
    field('operator', $.flow_arrow),
    optional(field('label', $.flow_edge_label)),
  ),

  flow_edge_id: ($) => seq(
    field('id', alias($._flow_identifier, $.identifier)),
    '@',
  ),

  flow_edge_label: ($) => seq('|', field('text', optional($.flow_edge_label_text)), '|'),

  flow_edge_label_text: (_) => token(prec(-5, /[^|\r\n]+/)),

  flow_arrow: (_) => token(/[ox<]?(?:(?:--+|==+|-?\.+-)[-ox>]?|~~~+)/),

  flow_class_annotation: ($) => seq(
    ':::',
    field('name', alias($._flow_identifier, $.identifier)),
  ),

  _flow_identifier: (_) => token(prec(-1,
    /[A-Za-z_\u00c0-\uffff](?:[A-Za-z0-9_\u00c0-\uffff]|-[A-Za-z0-9_\u00c0-\uffff])*/,
  )),
};

module.exports = { flowchartRules };
