// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/mindmap/parser/mindmap.jison and
// packages/mermaid/src/docs/syntax/mindmap.md
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const { indentationTransition } = require('../shared/indentation');

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, /mindmap/i)), $.diagram_keyword),
);

const trailingTrivia = ($) => optional(choice($.comment, $.directive));

const shapeDelimiter = ($, delimiter) => alias(
  delimiter,
  $.mindmap_shape_delimiter,
);

const inlineShape = ($, open, close, text) => seq(
  field('open', shapeDelimiter($, open)),
  optional(field('label', choice(
    $.mindmap_markdown_string,
    $.mindmap_quoted_string,
    alias(text, $.mindmap_label_text),
  ))),
  field('close', shapeDelimiter($, close)),
);

const multilineShape = ($, open, close) => seq(
  field('open', shapeDelimiter($, open)),
  $._line_ending,
  repeat1(choice(
    seq(
      optional(field('indentation', $.mindmap_continuation_indentation)),
      field(
        'label',
        alias($._mindmap_multiline_label_line, $.mindmap_multiline_label_text),
      ),
      $._line_ending,
    ),
    $._blank_line,
  )),
  optional(field('indentation', $.mindmap_continuation_indentation)),
  field('close', shapeDelimiter($, close)),
);

const mindmapRules = {
  mindmap_diagram: ($) => seq(
    field('header', $.mindmap_header),
    optional(field('body', $.mindmap_body)),
  ),

  mindmap_header: ($) => seq(
    diagramKeyword($),
    field('terminator', $._line_ending),
  ),

  mindmap_body: ($) => choice(
    repeat1($._mindmap_line_item),
    seq(
      repeat($._mindmap_line_item),
      $._mindmap_eof_item,
    ),
  ),

  _mindmap_line_item: ($) => choice(
    $.mindmap_incomplete_node_statement,
    seq($._mindmap_statement, trailingTrivia($), $._line_ending),
    seq(choice($.comment, $.directive), $._line_ending),
    $._blank_line,
  ),

  _mindmap_eof_item: ($) => choice(
    seq($._mindmap_statement, trailingTrivia($)),
    $.comment,
    $.directive,
  ),

  _mindmap_statement: ($) => choice(
    $.mindmap_icon_statement,
    $.mindmap_class_statement,
    $.mindmap_node_statement,
  ),

  mindmap_node_statement: ($) => prec.right(20, seq(
    optional(indentationTransition($, 'mindmap')),
    field('node', $.mindmap_node),
  )),

  mindmap_incomplete_node_statement: ($) => prec.dynamic(50, prec.right(seq(
    optional(indentationTransition($, 'mindmap')),
    field('node', $.mindmap_incomplete_node),
  ))),

  mindmap_node: ($) => choice(
    prec(20, seq(
      optional(field('id', $.mindmap_node_id)),
      field('shape', $.mindmap_shape),
    )),
    field('label', $.mindmap_plain_label),
  ),

  mindmap_incomplete_node: ($) => seq(
    optional(field('id', $.mindmap_node_id)),
    field('shape', $.mindmap_unclosed_square_shape),
  ),

  mindmap_node_id: ($) => $._mindmap_node_text,

  mindmap_plain_label: ($) => $._mindmap_node_text,

  mindmap_shape: ($) => choice(
    $.mindmap_square_shape,
    $.mindmap_round_shape,
    $.mindmap_circle_shape,
    $.mindmap_bang_shape,
    $.mindmap_cloud_shape,
    $.mindmap_hexagon_shape,
  ),

  mindmap_square_shape: ($) => choice(
    inlineShape($, '[', ']', $._mindmap_square_label_text),
    multilineShape($, '[', ']'),
  ),

  mindmap_round_shape: ($) => choice(
    inlineShape($, '(', ')', $._mindmap_round_label_text),
    multilineShape($, '(', ')'),
  ),

  mindmap_circle_shape: ($) => choice(
    inlineShape($, '((', '))', $._mindmap_round_label_text),
    multilineShape($, '((', '))'),
  ),

  mindmap_bang_shape: ($) => choice(
    inlineShape($, '))', '((', $._mindmap_bang_label_text),
    multilineShape($, '))', '(('),
  ),

  mindmap_cloud_shape: ($) => choice(
    inlineShape($, ')', '(', $._mindmap_bang_label_text),
    multilineShape($, ')', '('),
  ),

  mindmap_hexagon_shape: ($) => choice(
    inlineShape($, '{{', '}}', $._mindmap_hexagon_label_text),
    multilineShape($, '{{', '}}'),
  ),

  mindmap_unclosed_square_shape: ($) => seq(
    field('open', shapeDelimiter($, '[')),
    field(
      'label',
      alias($._mindmap_unclosed_square_tail, $.mindmap_unclosed_label_text),
    ),
  ),

  mindmap_icon_statement: ($) => prec(30, seq(
    field('marker', alias('::icon', $.mindmap_icon_marker)),
    field('open', alias('(', $.mindmap_decorator_delimiter)),
    optional(field('name', $.mindmap_icon_name)),
    field('close', alias(')', $.mindmap_decorator_delimiter)),
  )),

  mindmap_class_statement: ($) => prec(30, seq(
    field('marker', alias(':::', $.mindmap_class_marker)),
    field('classes', $.mindmap_class_list),
  )),

  mindmap_continuation_indentation: (_) => token(prec(30, /[ \t]+/)),

  _mindmap_node_text: (_) => token(prec(
    5,
    /[^\s%:()\[\]{}\r\n](?:[^%()\[\]{}\r\n]|%[^%\r\n])*/,
  )),

  _mindmap_square_label_text: (_) => token.immediate(prec(
    -5,
    /[^\]"\r\n][^\]\r\n]*/,
  )),

  _mindmap_round_label_text: (_) => token.immediate(prec(
    -5,
    /[^\)"\r\n][^\)\r\n]*/,
  )),

  _mindmap_bang_label_text: (_) => token.immediate(prec(
    -5,
    /[^\("\r\n][^\(\r\n]*/,
  )),

  _mindmap_hexagon_label_text: (_) => token.immediate(prec(
    -5,
    /[^\}"\r\n][^\}\r\n]*/,
  )),

  _mindmap_multiline_label_line: (_) => token(prec(-10, /[^\r\n]+/)),

  _mindmap_unclosed_square_tail: (_) => token.immediate(prec(
    50,
    seq(/[^\]\r\n]+/, choice('\r\n', '\n', '\r')),
  )),

  mindmap_quoted_string: (_) => token(prec(40, choice(
    seq('"', /(?:[^"\\\r\n]|\\.)*/, '"'),
    seq("'", /(?:[^'\\\r\n]|\\.)*/, "'"),
  ))),

  mindmap_markdown_string: (_) => token(prec(50, choice(
    /"`[^`]*`"/,
    /`[^`]*`/,
  ))),

  mindmap_icon_name: (_) => token(prec(-5, /[^)\r\n]+/)),

  mindmap_class_list: (_) => token(prec(-5, /[^%\r\n]+/)),
};

module.exports = { mindmapRules };
