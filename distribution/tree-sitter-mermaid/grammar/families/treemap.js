const { terminatedHeader } = require('../shared/header');
const { indentationTransition } = require('../shared/indentation');

const treemapRules = {
  treemap_diagram: ($) => choice(
    seq(
      field('header', $.treemap_header),
      optional(field('body', $.treemap_body)),
    ),
    field('header', alias($._treemap_header_eof, $.treemap_header)),
  ),

  treemap_header: ($) => terminatedHeader(
    $,
    token(prec(20, choice('treemap-beta', 'treemap'))),
  ),

  _treemap_header_eof: ($) => field(
    'keyword',
    alias(token(prec(20, choice('treemap-beta', 'treemap'))), $.diagram_keyword),
  ),

  treemap_body: ($) => repeat1(choice(
    $.comment,
    $._blank_line,
    $.treemap_class_definition,
    $.treemap_item_statement,
    $.treemap_unstructured_statement,
  )),

  treemap_item_statement: ($) => prec.right(seq(
    optional(indentationTransition($, 'treemap')),
    field('item', choice(
      prec(2, $.treemap_leaf),
      $.treemap_section,
    )),
    $._line_ending,
  )),

  treemap_section: ($) => seq(
    field('name', $.quoted_string),
    optional(field('class', $.treemap_class_annotation)),
  ),

  treemap_leaf: ($) => seq(
    field('name', $.quoted_string),
    field('separator', choice(':', ',')),
    field('value', $.treemap_number),
    optional(field('class', $.treemap_class_annotation)),
  ),

  treemap_class_annotation: ($) => seq(':::', field('name', $.identifier)),

  treemap_class_definition: ($) => prec.right(seq(
    field(
      'keyword',
      alias(token(prec(20, 'classDef')), $.statement_keyword),
    ),
    field('name', $.identifier),
    optional(field('style', $.treemap_style_text)),
    optional(';'),
    $._line_ending,
  )),

  treemap_number: (_) => token(/[0-9_.,]+/),

  treemap_style_text: (_) => token(prec(5, /[^;\r\n]+/)),

  treemap_unstructured_statement: ($) => prec.right(seq(
    alias($.unstructured_line, $.unstructured_body),
    $._line_ending,
  )),
};

module.exports = { treemapRules };
