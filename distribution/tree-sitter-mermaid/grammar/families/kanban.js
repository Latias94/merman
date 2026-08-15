// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/kanban/parser/kanban.jison and
// packages/mermaid/src/docs/syntax/kanban.md
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const { indentationTransition } = require('../shared/indentation');

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, /kanban/i)), $.diagram_keyword),
);

const trailingTrivia = ($) => optional(choice($.comment, $.directive));

const shapeDelimiter = ($, delimiter) => alias(
  delimiter,
  $.kanban_shape_delimiter,
);

const metadataDelimiter = ($, delimiter) => alias(
  delimiter,
  $.kanban_metadata_delimiter,
);

const metadataSeparator = ($, separator) => alias(
  separator,
  $.kanban_metadata_separator,
);

const inlineShape = ($, open, close, text) => seq(
  field('open', shapeDelimiter($, open)),
  optional(field('label', choice(
    $.kanban_markdown_string,
    $.kanban_quoted_string,
    alias(text, $.kanban_label_text),
  ))),
  field('close', shapeDelimiter($, close)),
);

const multilineShape = ($, open, close) => seq(
  field('open', shapeDelimiter($, open)),
  $._line_ending,
  repeat1(choice(
    seq(
      optional(field('indentation', $.kanban_continuation_indentation)),
      field(
        'label',
        alias($._kanban_multiline_label_line, $.kanban_multiline_label_text),
      ),
      $._line_ending,
    ),
    $._blank_line,
  )),
  optional(field('indentation', $.kanban_continuation_indentation)),
  field('close', shapeDelimiter($, close)),
);

const kanbanRules = {
  kanban_diagram: ($) => seq(
    field('header', $.kanban_header),
    optional(field('body', $.kanban_body)),
  ),

  kanban_header: ($) => seq(
    diagramKeyword($),
    field('terminator', $._line_ending),
  ),

  kanban_body: ($) => choice(
    repeat1($._kanban_line_item),
    seq(
      repeat($._kanban_line_item),
      $._kanban_eof_item,
    ),
  ),

  _kanban_line_item: ($) => choice(
    $.kanban_incomplete_item_statement,
    seq($._kanban_statement, trailingTrivia($), $._line_ending),
    seq(choice($.comment, $.directive), $._line_ending),
    $._blank_line,
  ),

  _kanban_eof_item: ($) => choice(
    seq($._kanban_statement, trailingTrivia($)),
    $.comment,
    $.directive,
  ),

  _kanban_statement: ($) => choice(
    $.kanban_icon_statement,
    $.kanban_class_statement,
    $.kanban_item_statement,
  ),

  kanban_item_statement: ($) => prec.right(20, seq(
    optional(indentationTransition($, 'kanban')),
    field('item', $.kanban_item),
    optional(field('metadata', $.kanban_metadata)),
  )),

  kanban_incomplete_item_statement: ($) => prec.dynamic(50, prec.right(seq(
    optional(indentationTransition($, 'kanban')),
    field('item', $.kanban_incomplete_item),
  ))),

  kanban_item: ($) => choice(
    prec(20, seq(
      optional(field('id', $.kanban_item_id)),
      field('shape', $.kanban_shape),
    )),
    field('label', $.kanban_plain_label),
  ),

  kanban_incomplete_item: ($) => seq(
    optional(field('id', $.kanban_item_id)),
    field('shape', $.kanban_unclosed_square_shape),
  ),

  kanban_item_id: ($) => $._kanban_item_text,

  kanban_plain_label: ($) => $._kanban_item_text,

  kanban_shape: ($) => choice(
    $.kanban_square_shape,
    $.kanban_round_shape,
    $.kanban_circle_shape,
    $.kanban_bang_shape,
    $.kanban_cloud_shape,
    $.kanban_hexagon_shape,
  ),

  kanban_square_shape: ($) => choice(
    inlineShape($, '[', ']', $._kanban_square_label_text),
    multilineShape($, '[', ']'),
  ),

  kanban_round_shape: ($) => choice(
    inlineShape($, '(', ')', $._kanban_round_label_text),
    multilineShape($, '(', ')'),
  ),

  kanban_circle_shape: ($) => choice(
    inlineShape($, '((', '))', $._kanban_round_label_text),
    multilineShape($, '((', '))'),
  ),

  kanban_bang_shape: ($) => choice(
    inlineShape($, '))', '((', $._kanban_bang_label_text),
    multilineShape($, '))', '(('),
  ),

  kanban_cloud_shape: ($) => choice(
    inlineShape($, ')', '(', $._kanban_bang_label_text),
    multilineShape($, ')', '('),
  ),

  kanban_hexagon_shape: ($) => choice(
    inlineShape($, '{{', '}}', $._kanban_hexagon_label_text),
    multilineShape($, '{{', '}}'),
  ),

  kanban_unclosed_square_shape: ($) => seq(
    field('open', shapeDelimiter($, '[')),
    field(
      'label',
      alias($._kanban_unclosed_square_tail, $.kanban_unclosed_label_text),
    ),
  ),

  kanban_metadata: ($) => seq(
    field('open', metadataDelimiter($, '@{')),
    repeat(choice(
      $.kanban_metadata_pair,
      metadataSeparator($, ','),
      $._line_ending,
    )),
    field('close', metadataDelimiter($, '}')),
  ),

  kanban_metadata_pair: ($) => seq(
    field('key', $.kanban_metadata_key),
    field('separator', metadataSeparator($, ':')),
    field('value', $.kanban_metadata_value),
  ),

  kanban_metadata_value: ($) => choice(
    $.kanban_markdown_string,
    $.kanban_quoted_string,
    $.kanban_metadata_bare_value,
  ),

  kanban_icon_statement: ($) => prec(30, seq(
    field('marker', alias('::icon', $.kanban_icon_marker)),
    field('open', alias('(', $.kanban_decorator_delimiter)),
    optional(field('name', $.kanban_icon_name)),
    field('close', alias(')', $.kanban_decorator_delimiter)),
  )),

  kanban_class_statement: ($) => prec(30, seq(
    field('marker', alias(':::', $.kanban_class_marker)),
    field('classes', $.kanban_class_list),
  )),

  kanban_continuation_indentation: (_) => token(prec(30, /[ \t]+/)),

  _kanban_item_text: (_) => token(prec(
    5,
    /[^\s%@:()\[\]{}\r\n](?:[^%@()\[\]{}\r\n]|%[^%\r\n])*/,
  )),

  _kanban_square_label_text: (_) => token.immediate(prec(
    -5,
    /[^\]"\r\n][^\]\r\n]*/,
  )),

  _kanban_round_label_text: (_) => token.immediate(prec(
    -5,
    /[^\)"\r\n][^\)\r\n]*/,
  )),

  _kanban_bang_label_text: (_) => token.immediate(prec(
    -5,
    /[^\("\r\n][^\(\r\n]*/,
  )),

  _kanban_hexagon_label_text: (_) => token.immediate(prec(
    -5,
    /[^\}"\r\n][^\}\r\n]*/,
  )),

  _kanban_multiline_label_line: (_) => token(prec(-10, /[^\r\n]+/)),

  _kanban_unclosed_square_tail: (_) => token.immediate(prec(
    50,
    seq(/[^\]\r\n]+/, choice('\r\n', '\n', '\r')),
  )),

  kanban_quoted_string: (_) => token(prec(40, choice(
    seq('"', /(?:[^"\\\r\n]|\\.)*/, '"'),
    seq("'", /(?:[^'\\\r\n]|\\.)*/, "'"),
  ))),

  kanban_markdown_string: (_) => token(prec(50, choice(
    /"`[^`]*`"/,
    /`[^`]*`/,
  ))),

  kanban_metadata_key: (_) => token(prec(
    20,
    /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_-\u00c0-\uffff]*/,
  )),

  kanban_metadata_bare_value: (_) => token(prec(
    -5,
    /[^\s,%}\r\n](?:(?:[^,%}\r\n]|%[^%\r\n])*[^\s,%}\r\n])?/,
  )),

  kanban_icon_name: (_) => token(prec(-5, /[^)\r\n]+/)),

  kanban_class_list: (_) => token(prec(-5, /[^%\r\n]+/)),
};

module.exports = { kanbanRules };
