// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/quadrant-chart/parser/quadrant.jison and
// packages/mermaid/src/diagrams/quadrant-chart/quadrantDb.ts
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, /quadrantChart/i)), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.statement_keyword),
);

const trailingTrivia = ($) => optional(choice($.comment, $.directive));

const quadrantChartRules = {
  quadrant_chart_diagram: ($) => choice(
    seq(
      field('header', $.quadrant_chart_header),
      optional(field('body', $.quadrant_chart_body)),
    ),
    seq(
      field(
        'header',
        alias($._quadrant_chart_inline_header, $.quadrant_chart_header),
      ),
      field('body', $.quadrant_chart_body),
    ),
    field(
      'header',
      alias($._quadrant_chart_header_eof, $.quadrant_chart_header),
    ),
  ),

  quadrant_chart_header: ($) => seq(
    diagramKeyword($),
    field('terminator', $._line_ending),
  ),

  _quadrant_chart_inline_header: ($) => seq(
    diagramKeyword($),
    token.immediate(/[ \t]+/),
  ),

  _quadrant_chart_header_eof: ($) => diagramKeyword($),

  quadrant_chart_body: ($) => choice(
    repeat1($._quadrant_chart_line_item),
    seq(
      repeat($._quadrant_chart_line_item),
      $._quadrant_chart_eof_item,
    ),
  ),

  _quadrant_chart_line_item: ($) => choice(
    seq(
      $._quadrant_chart_statement,
      trailingTrivia($),
      $._quadrant_chart_terminator,
    ),
    seq(choice($.comment, $.directive), $._quadrant_chart_terminator),
    $._blank_line,
    ';',
  ),

  _quadrant_chart_eof_item: ($) => choice(
    seq($._quadrant_chart_statement, trailingTrivia($)),
    $.comment,
    $.directive,
  ),

  _quadrant_chart_terminator: ($) => choice($._line_ending, ';'),

  _quadrant_chart_statement: ($) => choice(
    $.quadrant_chart_title_statement,
    $.quadrant_chart_accessibility_title_statement,
    $.quadrant_chart_accessibility_description_statement,
    $.quadrant_chart_axis_statement,
    $.quadrant_chart_quadrant_statement,
    $.quadrant_chart_point_statement,
    $.quadrant_chart_class_definition_statement,
    $.quadrant_chart_incomplete_axis_statement,
    $.quadrant_chart_incomplete_quadrant_statement,
    $.quadrant_chart_malformed_point_statement,
    $.quadrant_chart_malformed_statement,
  ),

  quadrant_chart_title_statement: ($) => prec(40, seq(
    statementKeyword($, /title/i),
    token.immediate(/[ \t]+/),
    field('text', $.quadrant_chart_line_text),
  )),

  quadrant_chart_accessibility_title_statement: ($) => prec(40, seq(
    statementKeyword($, /accTitle/i),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.quadrant_chart_accessibility_line_text)),
  )),

  quadrant_chart_accessibility_description_statement: ($) => prec(40, seq(
    statementKeyword($, /accDescr/i),
    optional(token.immediate(/[ \t]+/)),
    choice(
      seq(
        field('delimiter', token.immediate(':')),
        optional(token.immediate(/[ \t]+/)),
        optional(field('text', $.quadrant_chart_accessibility_line_text)),
      ),
      field('description', choice(
        $.quadrant_chart_accessibility_description_block,
        $.quadrant_chart_unclosed_accessibility_description_block,
      )),
    ),
  )),

  quadrant_chart_axis_statement: ($) => prec.right(30, seq(
    field('axis', $.quadrant_chart_axis),
    token.immediate(/[ \t]+/),
    field('start', $.quadrant_chart_axis_text),
    optional(seq(
      field('delimiter', $.quadrant_chart_axis_delimiter),
      optional(field('end', $.quadrant_chart_axis_text)),
    )),
  )),

  quadrant_chart_quadrant_statement: ($) => prec(30, seq(
    field('quadrant', $.quadrant_chart_quadrant),
    token.immediate(/[ \t]+/),
    field('label', $.quadrant_chart_label),
  )),

  quadrant_chart_point_statement: ($) => prec(20, seq(
    field('label', $.quadrant_chart_point_label),
    optional(field('class', $.quadrant_chart_class_clause)),
    field(
      'delimiter',
      alias(':', $.quadrant_chart_point_delimiter),
    ),
    optional(token.immediate(/[ \t]+/)),
    field('coordinates', $.quadrant_chart_coordinates),
    optional(token.immediate(/[ \t]+/)),
    optional(field('styles', $.quadrant_chart_style_list)),
  )),

  quadrant_chart_class_clause: ($) => seq(
    field(
      'delimiter',
      alias(':::', $.quadrant_chart_class_delimiter),
    ),
    field('name', $.quadrant_chart_class_name),
  ),

  quadrant_chart_coordinates: ($) => seq(
    field('open', '['),
    optional(token.immediate(/[ \t]+/)),
    field('x', $.quadrant_chart_coordinate),
    field('delimiter', ','),
    optional(token.immediate(/[ \t]+/)),
    field('y', $.quadrant_chart_coordinate),
    optional(token.immediate(/[ \t]+/)),
    field('close', ']'),
  ),

  quadrant_chart_style_list: ($) => seq(
    field('style', $.quadrant_chart_style),
    repeat(seq(
      field('delimiter', ','),
      optional(token.immediate(/[ \t]+/)),
      field('style', $.quadrant_chart_style),
    )),
  ),

  quadrant_chart_style: ($) => seq(
    field('name', $.quadrant_chart_style_name),
    field('delimiter', ':'),
    optional(token.immediate(/[ \t]+/)),
    field('value', $.quadrant_chart_style_value),
  ),

  quadrant_chart_class_definition_statement: ($) => prec(30, seq(
    statementKeyword($, /classDef/i),
    token.immediate(/[ \t]+/),
    field('name', $.quadrant_chart_class_name),
    token.immediate(/[ \t]+/),
    field('styles', $.quadrant_chart_style_list),
  )),

  quadrant_chart_incomplete_axis_statement: ($) => prec(-10, seq(
    field('axis', $.quadrant_chart_axis),
  )),

  quadrant_chart_incomplete_quadrant_statement: ($) => prec(-10, seq(
    field('quadrant', $.quadrant_chart_quadrant),
  )),

  quadrant_chart_malformed_point_statement: ($) => prec(-20, seq(
    field('label', $.quadrant_chart_point_label),
    optional(field('class', $.quadrant_chart_class_clause)),
    field(
      'delimiter',
      alias(':', $.quadrant_chart_point_delimiter),
    ),
    optional(token.immediate(/[ \t]+/)),
    field('recovery', choice(
      $.quadrant_chart_malformed_coordinates,
      $.quadrant_chart_point_recovery_text,
    )),
  )),

  quadrant_chart_malformed_statement: ($) => prec(-100, field(
    'text',
    alias(
      $.quadrant_chart_bare_point_label,
      $.quadrant_chart_malformed_text,
    ),
  )),

  quadrant_chart_malformed_coordinates: ($) => choice(
    seq(
      field('open', '['),
      optional(token.immediate(/[ \t]+/)),
      field('x', $.quadrant_chart_invalid_coordinate),
      optional(seq(
        field('delimiter', ','),
        optional(token.immediate(/[ \t]+/)),
        field('y', choice(
          $.quadrant_chart_coordinate,
          $.quadrant_chart_invalid_coordinate,
        )),
      )),
      optional(token.immediate(/[ \t]+/)),
      optional(field('close', ']')),
    ),
    seq(
      field('open', '['),
      optional(token.immediate(/[ \t]+/)),
      field('x', $.quadrant_chart_coordinate),
      field('delimiter', ','),
      optional(token.immediate(/[ \t]+/)),
      field('y', $.quadrant_chart_invalid_coordinate),
      optional(token.immediate(/[ \t]+/)),
      optional(field('close', ']')),
    ),
    seq(
      field('open', '['),
      optional(token.immediate(/[ \t]+/)),
      field('x', $.quadrant_chart_coordinate),
      token.immediate(/[ \t]+/),
      field('y', $.quadrant_chart_coordinate),
      optional(token.immediate(/[ \t]+/)),
      field('close', ']'),
    ),
    seq(
      field('open', '['),
      optional(token.immediate(/[ \t]+/)),
      field('x', $.quadrant_chart_coordinate),
      field('delimiter', ','),
      optional(token.immediate(/[ \t]+/)),
      field('y', $.quadrant_chart_coordinate),
    ),
  ),

  quadrant_chart_axis: (_) => token(prec(30, choice(/x-axis/i, /y-axis/i))),

  quadrant_chart_quadrant: (_) => token(prec(
    30,
    choice(/quadrant-1/i, /quadrant-2/i, /quadrant-3/i, /quadrant-4/i),
  )),

  quadrant_chart_axis_delimiter: (_) => token(prec(30, /--+>/)),

  quadrant_chart_axis_text: ($) => choice(
    $.quadrant_chart_markdown_text,
    $.quadrant_chart_quoted_text,
    $.quadrant_chart_unclosed_quoted_text,
    repeat1(choice(
      $._quadrant_chart_axis_text_chunk,
      $._quadrant_chart_axis_text_hyphen,
    )),
  ),

  quadrant_chart_label: ($) => choice(
    $.quadrant_chart_markdown_text,
    $.quadrant_chart_quoted_text,
    $.quadrant_chart_unclosed_quoted_text,
    $.quadrant_chart_bare_label,
  ),

  quadrant_chart_point_label: ($) => choice(
    $.quadrant_chart_markdown_text,
    $.quadrant_chart_quoted_text,
    $.quadrant_chart_unclosed_quoted_text,
    $.quadrant_chart_bare_point_label,
  ),

  quadrant_chart_markdown_text: (_) => token(seq(
    '"`',
    /[^`"\r\n]+/,
    '`"',
  )),

  quadrant_chart_quoted_text: (_) => token(prec(10, seq(
    '"',
    /[^"\r\n]*/,
    '"',
  ))),

  quadrant_chart_unclosed_quoted_text: (_) => token(prec(-10, seq(
    '"',
    /[^"\r\n]*/,
  ))),

  quadrant_chart_accessibility_description_block: (_) => token(seq(
    '{',
    /[^}]*/,
    '}',
  )),

  quadrant_chart_unclosed_accessibility_description_block: (_) => token(prec(-10, seq(
    '{',
    /[^}\r\n]*/,
  ))),

  quadrant_chart_coordinate: (_) => token.immediate(prec(
    30,
    /(?:1|0(?:\.[0-9]+)?)/,
  )),

  quadrant_chart_invalid_coordinate: (_) => token.immediate(prec(40, choice(
    /1\.[0-9]+/,
    /[2-9][0-9]*(?:\.[0-9]+)?/,
    /\.[0-9]+/,
    /-[0-9]+(?:\.[0-9]+)?/,
  ))),

  quadrant_chart_class_name: (_) => token(prec(20, /[A-Za-z_][A-Za-z0-9_-]*/)),

  quadrant_chart_style_name: (_) => token.immediate(prec(20, choice(
    'radius',
    'color',
    'stroke-color',
    'stroke-width',
  ))),

  quadrant_chart_style_value: (_) => token.immediate(prec(
    5,
    /[^\s,%\r\n](?:[^,%\r\n]*[^\s,%\r\n])?/,
  )),

  quadrant_chart_line_text: (_) => token(prec(
    -5,
    /(?:[^%;\r\n]|%[^%\r\n])+/
  )),

  quadrant_chart_accessibility_line_text: (_) => token(prec(
    -5,
    /(?:[^%;\r\n]|%[^%\r\n])+/
  )),

  quadrant_chart_bare_label: (_) => token(prec(
    5,
    /[^\s":%;\r\n](?:[^":%;\r\n]*[^\s":%;\r\n])?/
  )),

  quadrant_chart_bare_point_label: (_) => token(prec(
    5,
    /[^\s":%;\r\n](?:[^":%;\r\n]*[^\s":%;\r\n])?/
  )),

  _quadrant_chart_axis_text_chunk: (_) => token(prec(-5, /[^"-%;\s]+/)),

  _quadrant_chart_axis_text_hyphen: (_) => token(prec(-10, '-')),

  quadrant_chart_point_recovery_text: (_) => token(prec(
    -50,
    /(?:[^%\r\n]|%[^%\r\n])+/
  )),

};

module.exports = { quadrantChartRules };
