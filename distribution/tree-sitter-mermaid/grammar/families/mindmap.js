const { terminatedHeader } = require('../shared/header');
const { indentationTransition } = require('../shared/indentation');

const mindmapRules = {
  mindmap_diagram: ($) => seq(
    field('header', $.mindmap_header),
    optional(field('body', $.mindmap_body)),
  ),

  mindmap_header: ($) => terminatedHeader($, token(prec(20, 'mindmap'))),

  mindmap_body: ($) => repeat1(choice(
    $.comment,
    $._blank_line,
    $.mindmap_icon_statement,
    $.mindmap_class_statement,
    $.mindmap_node_statement,
  )),

  mindmap_node_statement: ($) => prec.right(seq(
    optional(indentationTransition($, 'mindmap')),
    field('node', $.mindmap_node),
    $._line_ending,
  )),

  mindmap_icon_statement: ($) => prec.right(seq(
    '::icon',
    '(',
    field('name', optional($.mindmap_decorator_text)),
    ')',
    $._line_ending,
  )),

  mindmap_class_statement: ($) => prec.right(seq(
    ':::',
    field('classes', $.mindmap_decorator_text),
    $._line_ending,
  )),

  mindmap_node: ($) => choice(
    prec(2, seq(
      optional(field('id', $.identifier)),
      field('shape', $.mindmap_shape),
    )),
    field('label', $.mindmap_plain_label),
  ),

  mindmap_plain_label: ($) => prec(-1, repeat1(choice(
    $.identifier,
    $.number,
    $.quoted_string,
    $.mindmap_label_punctuation,
  ))),

  mindmap_shape: ($) => choice(
    $.mindmap_square_shape,
    $.mindmap_round_shape,
    $.mindmap_circle_shape,
    $.mindmap_bang_shape,
    $.mindmap_cloud_shape,
    $.mindmap_hexagon_shape,
  ),

  mindmap_square_shape: ($) => seq('[', field('label', optional($.mindmap_shape_text)), ']'),

  mindmap_round_shape: ($) => seq('(', field('label', optional($.mindmap_shape_text)), ')'),

  mindmap_circle_shape: ($) => seq('((', field('label', optional($.mindmap_shape_text)), '))'),

  mindmap_bang_shape: ($) => seq('))', field('label', optional($.mindmap_shape_text)), '(('),

  mindmap_cloud_shape: ($) => seq(')', field('label', optional($.mindmap_shape_text)), '('),

  mindmap_hexagon_shape: ($) => seq('{{', field('label', optional($.mindmap_shape_text)), '}}'),

  mindmap_shape_text: (_) => token(prec(-5, /[^\]\)\}\r\n]+/)),

  mindmap_label_punctuation: (_) => token(prec(-5, /[^\s\w\u00c0-\uffff()\[\]{}%:\r\n]+/)),

  mindmap_decorator_text: (_) => token(prec(-5, /[^)\r\n]+/)),
};

module.exports = { mindmapRules };
