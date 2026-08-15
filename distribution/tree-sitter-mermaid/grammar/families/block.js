// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/block/parser/block.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(30, choice('block-beta', 'block'))), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => seq(
  optional(token.immediate(/[ \t]+/)),
  field(
    'keyword',
    alias(token.immediate(keyword), $.block_statement_keyword),
  ),
);

const shapeDelimiter = ($, delimiter) => alias(
  delimiter,
  $.block_shape_delimiter,
);

const immediateQuotedLabel = ($) => alias(
  token.immediate(prec(20, choice(
    seq('"', /(?:[^"\\\r\n]|\\.)*/, '"'),
    seq("'", /(?:[^'\\\r\n]|\\.)*/, "'"),
  ))),
  $.block_quoted_label,
);

const blockConflicts = ($) => [
  [
    $.block_node_statement,
    $.block_edge_statement,
    $.block_incomplete_edge_statement,
  ],
  [$.block_edge_statement],
];

const shape = ($, open, close) => seq(
  field('open', shapeDelimiter($, open)),
  optional(field('label', $.block_label)),
  field('close', shapeDelimiter($, close)),
);

const blockRules = {
  block_diagram: ($) => seq(
    field('header', $.block_header),
    field('body', $.block_body),
  ),

  block_header: ($) => prec(40, seq(
    diagramKeyword($),
    optional(token.immediate(/[ \t]+/)),
    optional(choice($.comment, $.directive)),
    field('terminator', $._line_ending),
  )),

  // Upstream deliberately skips horizontal whitespace, so `a b c` is three
  // declarations rather than one free-text line. Keep each construct explicit
  // and use only newlines/semicolons as synchronization tokens for recovery.
  block_body: ($) => repeat1($._block_item),

  _block_item: ($) => choice(
    $.block_composite_statement,
    $.block_columns_statement,
    $.block_space_statement,
    $.block_edge_statement,
    $.block_incomplete_edge_statement,
    $.block_class_definition_statement,
    $.block_class_assignment_statement,
    $.block_style_statement,
    $.block_accessibility_title_statement,
    $.block_accessibility_description_statement,
    $.block_node_statement,
    $.comment,
    $.directive,
    $._line_ending,
    ';',
  ),

  block_columns_statement: ($) => prec(40, seq(
    statementKeyword($, 'columns'),
    token.immediate(/[ \t]+/),
    field('count', choice(
      alias(token.immediate('auto'), $.block_column_count),
      $.block_column_count,
    )),
  )),

  block_column_count: (_) => token.immediate(/[0-9]+/),

  block_space_statement: ($) => prec(40, seq(
    statementKeyword($, 'space'),
    optional(seq(
      field('delimiter', token.immediate(':')),
      field('width', $.block_width),
    )),
  )),

  block_composite_statement: ($) => prec.right(50, seq(
    field('declaration', choice(
      seq(
        statementKeyword($, 'block'),
        field('delimiter', token.immediate(':')),
        field('node', $.block_node),
      ),
      statementKeyword($, 'block'),
    )),
    repeat($._block_nested_item),
    field('end', $.block_end),
  )),

  _block_nested_item: ($) => choice(
    $.block_composite_statement,
    $.block_columns_statement,
    $.block_space_statement,
    $.block_edge_statement,
    $.block_incomplete_edge_statement,
    $.block_class_definition_statement,
    $.block_class_assignment_statement,
    $.block_style_statement,
    $.block_accessibility_title_statement,
    $.block_accessibility_description_statement,
    $.block_node_statement,
    $.comment,
    $.directive,
    $._line_ending,
    ';',
  ),

  block_end: (_) => token(prec(40, 'end')),

  block_node_statement: ($) => field('node', $.block_node),

  block_node: ($) => seq(
    optional(token.immediate(/[ \t]+/)),
    field('id', $.block_identifier),
    optional(choice(
      field('shape', $.block_shape),
      field('recovery', $.block_incomplete_shape),
    )),
    optional(field('width', $.block_width_clause)),
  ),

  block_width_clause: ($) => seq(
    field('delimiter', token.immediate(':')),
    field('value', $.block_width),
  ),

  block_width: (_) => token.immediate(/[0-9]+/),

  block_shape: ($) => choice(
    shape($, '[', ']'),
    shape($, '(', ')'),
    shape($, '([', '])'),
    shape($, '[[', ']]'),
    shape($, '[(', ')]'),
    shape($, '((', '))'),
    shape($, '(((', ')))'),
    shape($, '{', '}'),
    shape($, '{{', '}}'),
    shape($, '>', ']'),
    shape($, '[/', '/]'),
    shape($, '[/', ']'),
    shape($, '[', '/]'),
    shape($, '[\\', '\\]'),
    shape($, '[/', '\\]'),
    shape($, '[\\', '/]'),
    shape($, '(-', '-)'),
    $.block_arrow_shape,
  ),

  block_incomplete_shape: ($) => prec(-30, seq(
    field('open', alias(choice(
      '<[',
      '(((',
      '((',
      '([',
      '[[',
      '[(',
      '[/',
      '[\\',
      '{{',
      '(-',
      '[',
      '(',
      '{',
      '>',
    ), $.block_shape_delimiter)),
    optional(field('text', $.block_shape_recovery_text)),
  )),

  block_shape_recovery_text: (_) => token(prec(-20, /[^;\r\n]+/)),

  block_arrow_shape: ($) => seq(
    field('open', shapeDelimiter($, '<[')),
    optional(field('label', $.block_label)),
    field('close', shapeDelimiter($, ']>(')),
    field('direction', $.block_arrow_direction),
    repeat(seq(
      field('delimiter', ','),
      field('direction', $.block_arrow_direction),
    )),
    field('direction_close', shapeDelimiter($, ')')),
  ),

  block_arrow_direction: (_) => choice(
    'right',
    'left',
    'up',
    'down',
    'x',
    'y',
  ),

  block_label: ($) => choice(
    $.block_quoted_label,
    $.block_bare_label,
  ),

  block_quoted_label: (_) => token(prec(20, choice(
    seq('"', /(?:[^"\\\r\n]|\\.)*/, '"'),
    seq("'", /(?:[^'\\\r\n]|\\.)*/, "'"),
  ))),

  block_bare_label: (_) => token(prec(
    5,
    /[^"'\[\](){}<>\\\/\r\n]+/,
  )),

  block_edge_statement: ($) => prec(50, seq(
    field('source', $.block_node),
    optional(token.immediate(/[ \t]+/)),
    field('edge', $.block_edge),
    field('target', $.block_node),
    repeat(seq(
      optional(token.immediate(/[ \t]+/)),
      field('edge', $.block_edge),
      field('target', $.block_node),
    )),
  )),

  block_edge: ($) => choice(
    $.block_labeled_edge,
    field('operator', $.block_edge_operator),
  ),

  block_labeled_edge: ($) => seq(
    field('operator', $.block_edge_label_start),
    optional(token.immediate(/[ \t]+/)),
    field('label', immediateQuotedLabel($)),
    optional(token.immediate(/[ \t]+/)),
    field('operator', $.block_edge_operator),
  ),

  block_edge_label_start: (_) => token.immediate(/[xo<]?(?:--|==|-\.)/),

  block_edge_operator: (_) => token.immediate(
    /[xo<]?(?:(?:--+|==+|-?\.+-)[-xo>]?|~~~+)/,
  ),

  block_incomplete_edge_statement: ($) => prec(-20, seq(
    field('source', $.block_node),
    optional(token.immediate(/[ \t]+/)),
    field('operator', choice(
      $.block_edge_operator,
      $.block_edge_label_start,
    )),
    optional(field('recovery', $.block_edge_recovery)),
  )),

  block_edge_recovery: (_) => token(prec(-20, /[^;\r\n]+/)),

  block_class_definition_statement: ($) => prec(40, seq(
    statementKeyword($, 'classDef'),
    token.immediate(/[ \t]+/),
    field('class', $.block_identifier),
    token.immediate(/[ \t]+/),
    field('style', $.block_style_list),
  )),

  block_class_assignment_statement: ($) => prec(40, seq(
    statementKeyword($, 'class'),
    token.immediate(/[ \t]+/),
    field('target', $.block_identifier_list),
    token.immediate(/[ \t]+/),
    field('class', $.block_identifier_list),
  )),

  block_style_statement: ($) => prec(40, seq(
    statementKeyword($, 'style'),
    token.immediate(/[ \t]+/),
    field('target', $.block_identifier_list),
    token.immediate(/[ \t]+/),
    field('style', $.block_style_list),
  )),

  block_identifier_list: ($) => seq(
    field('item', $.block_identifier),
    repeat(seq(
      field('delimiter', token.immediate(',')),
      optional(token.immediate(/[ \t]+/)),
      field('item', $.block_identifier),
    )),
  ),

  block_style_list: ($) => seq(
    field('item', $.block_style_declaration),
    repeat(seq(
      field('delimiter', token.immediate(',')),
      optional(token.immediate(/[ \t]+/)),
      field('item', $.block_style_declaration),
    )),
  ),

  block_style_declaration: ($) => seq(
    field('property', $.block_style_property),
    field('delimiter', token.immediate(':')),
    field('value', $.block_style_value),
  ),

  block_style_property: (_) => token(/[A-Za-z_][A-Za-z0-9_-]*/),

  block_style_value: (_) => token.immediate(/[^,;\r\n]+/),

  block_accessibility_title_statement: ($) => prec.right(40, seq(
    statementKeyword($, 'accTitle'),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.block_line_text)),
  )),

  block_accessibility_description_statement: ($) => prec.right(40, seq(
    statementKeyword($, 'accDescr'),
    optional(token.immediate(/[ \t]+/)),
    choice(
      seq(
        field('delimiter', token.immediate(':')),
        optional(token.immediate(/[ \t]+/)),
        optional(field('text', $.block_line_text)),
      ),
      field('description', choice(
        $.block_accessibility_description_block,
        $.block_unclosed_accessibility_description_block,
      )),
    ),
  )),

  block_accessibility_description_block: (_) => token(seq('{', /[^}]*/, '}')),

  block_unclosed_accessibility_description_block: (_) => token(prec(
    -10,
    seq('{', /[^}\r\n]*/),
  )),

  block_line_text: (_) => token(prec(5, /[^;\r\n]+/)),

  block_identifier: (_) => token.immediate(prec(
    0,
    /[A-Za-z0-9_.\u00c0-\uffff]+/,
  )),
};

module.exports = { blockConflicts, blockRules };
