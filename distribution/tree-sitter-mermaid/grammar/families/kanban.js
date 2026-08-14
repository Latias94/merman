const { terminatedHeader } = require('../shared/header');

const kanbanRules = {
  kanban_diagram: ($) => seq(
    field('header', $.kanban_header),
    optional(field('body', $.kanban_body)),
  ),

  kanban_header: ($) => terminatedHeader($, token(prec(20, 'kanban'))),

  kanban_body: ($) => repeat1(choice(
    $.comment,
    $._blank_line,
    $.kanban_style_statement,
    $.kanban_icon_statement,
    $.kanban_class_statement,
    $.kanban_item_statement,
    $.kanban_unstructured_statement,
  )),

  kanban_item_statement: ($) => prec.right(seq(
    optional(field('indentation', $.kanban_indentation)),
    field('item', $.kanban_item),
    optional(field('metadata', $.kanban_metadata)),
    optional($._line_ending),
  )),

  kanban_item: ($) => choice(
    prec(2, seq(
      optional(field('id', $.identifier)),
      field('label', $.kanban_square_label),
    )),
    field('label', choice(
      alias($.identifier, $.kanban_bare_label),
      $.kanban_bare_label,
    )),
  ),

  kanban_square_label: ($) => seq('[', optional(field('text', $.kanban_label_text)), ']'),

  kanban_metadata: ($) => seq(
    '@{',
    repeat(choice(
      $.kanban_metadata_pair,
      ',',
      $._line_ending,
    )),
    '}',
  ),

  kanban_metadata_pair: ($) => seq(
    field('key', $.identifier),
    ':',
    field('value', $.kanban_metadata_value),
  ),

  kanban_style_statement: ($) => prec.right(seq(
    field('keyword', 'style'),
    field('target', $.identifier),
    field('style', $.kanban_style_text),
    optional($._line_ending),
  )),

  kanban_icon_statement: ($) => prec.right(seq(
    '::icon',
    '(',
    optional(field('name', $.kanban_decorator_text)),
    ')',
    optional($._line_ending),
  )),

  kanban_class_statement: ($) => prec.right(seq(
    ':::',
    field('classes', $.kanban_decorator_text),
    optional($._line_ending),
  )),

  kanban_indentation: (_) => token(prec(30, /[ \t]+/)),

  kanban_label_text: (_) => token(prec(-5, /[^\]\r\n]+/)),

  kanban_bare_label: (_) => token(prec(0, /[^\s%@:\[][^@\[\r\n]*/)),

  kanban_metadata_value: (_) => token(prec(-5, /[^,}\r\n]+/)),

  kanban_style_text: (_) => token(prec(-5, /[^\r\n]+/)),

  kanban_decorator_text: (_) => token(prec(-5, /[^)\r\n]+/)),

  kanban_unstructured_statement: ($) => prec.right(seq(
    alias($.unstructured_line, $.unstructured_body),
    optional($._line_ending),
  )),
};

module.exports = { kanbanRules };
