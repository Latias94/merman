// Source translation: Mermaid 11.16.1
// packages/parser/src/language/treemap/treemap.langium:44-89
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const { terminatedHeader } = require('../shared/header');

const treemapIndentation = ($) => choice(
  alias($._treemap_start, $.treemap_indentation_start),
  alias($._treemap_indent, $.treemap_indentation_indent),
  alias($._treemap_reindent, $.treemap_indentation_reindent),
  alias($._treemap_dedent, $.treemap_indentation_dedent),
  $.treemap_indentation_overflow,
);

const treemapName = ($) => choice(
  alias($.langium_string, $.treemap_quoted_name),
  alias($.langium_unclosed_string, $.treemap_unclosed_name),
);

const trailingTrivia = ($) => optional(field(
  'trivia',
  choice($.comment, $.directive),
));

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

  treemap_body: ($) => choice(
    repeat1($._treemap_line_item),
    seq(
      repeat($._treemap_line_item),
      $._treemap_eof_item,
    ),
  ),

  _treemap_line_item: ($) => choice(
    seq(
      field('statement', $._treemap_statement),
      trailingTrivia($),
      $._line_ending,
    ),
    seq(field('trivia', choice($.comment, $.directive)), $._line_ending),
    $._blank_line,
  ),

  _treemap_eof_item: ($) => choice(
    seq(field('statement', $._treemap_statement), trailingTrivia($)),
    $.comment,
    $.directive,
  ),

  _treemap_statement: ($) => choice(
    $.langium_title_statement,
    $.langium_acc_title_statement,
    $.langium_acc_descr_statement,
    $.treemap_class_definition,
    $.treemap_item_statement,
  ),

  treemap_item_statement: ($) => prec.right(20, seq(
    optional(field('indentation', treemapIndentation($))),
    field('item', choice(
      $.treemap_leaf,
      $.treemap_incomplete_leaf,
      $.treemap_malformed_leaf,
      $.treemap_section,
    )),
  )),

  treemap_section: ($) => prec(-10, seq(
    field('name', treemapName($)),
    optional(field('class', $.treemap_class_annotation)),
  )),

  treemap_leaf: ($) => prec(40, seq(
    field('name', treemapName($)),
    field('separator', $.treemap_value_separator),
    field('value', $.treemap_number),
    optional(field('class', $.treemap_class_annotation)),
  )),

  treemap_incomplete_leaf: ($) => prec(-10, seq(
    field('name', treemapName($)),
    field('separator', $.treemap_value_separator),
  )),

  treemap_malformed_leaf: ($) => prec(-20, seq(
    field('name', treemapName($)),
    field('separator', $.treemap_value_separator),
    field('value', $.treemap_invalid_value),
  )),

  treemap_class_annotation: ($) => seq(
    field('marker', $.treemap_class_marker),
    field('name', $.treemap_class_name),
  ),

  treemap_class_definition: ($) => prec.right(seq(
    field(
      'keyword',
      alias(token(prec(50, 'classDef')), $.statement_keyword),
    ),
    token.immediate(/[ \t]+/),
    field('name', $.treemap_class_name),
    optional(seq(
      token.immediate(/[ \t]+/),
      field('style', $.treemap_style_text),
    )),
    optional(';'),
  )),

  treemap_value_separator: (_) => token(prec(40, choice(':', ','))),

  treemap_class_marker: (_) => token(prec(50, ':::')),

  treemap_class_name: (_) => token(/[A-Za-z_][A-Za-z0-9_]*/),

  treemap_number: (_) => token(/[0-9_.,]+/),

  treemap_style_text: (_) => token.immediate(prec(5, /[^;\r\n]+/)),

  // This recovery token is reachable only after a quoted name and value
  // separator, so it cannot replace a valid sibling or a whole diagram body.
  treemap_invalid_value: (_) => token(prec(
    -50,
    /[^0-9_., \t\r\n][^\r\n]*/,
  )),
};

module.exports = { treemapRules };
