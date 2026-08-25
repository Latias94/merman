// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/venn/parser/venn.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, /venn-beta/i)), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.statement_keyword),
);

const setDelimiter = ($) => field(
  'delimiter',
  alias(',', $.venn_set_delimiter),
);

const valueDelimiter = ($) => field(
  'delimiter',
  alias(':', $.venn_value_delimiter),
);

const vennRules = {
  venn_diagram: ($) => choice(
    seq(
      field('header', $.venn_header),
      optional(field('body', $.venn_body)),
    ),
    field('header', alias($._venn_header_eof, $.venn_header)),
  ),

  venn_header: ($) => seq(
    diagramKeyword($),
    field('terminator', $._line_ending),
  ),

  _venn_header_eof: ($) => diagramKeyword($),

  venn_body: ($) => choice(
    repeat1($._venn_line_item),
    seq(
      repeat($._venn_line_item),
      $._venn_eof_item,
    ),
  ),

  _venn_line_item: ($) => choice(
    seq(
      choice($._venn_statement, $.comment),
      $._line_ending,
    ),
    $._blank_line,
  ),

  _venn_eof_item: ($) => choice($._venn_statement, $.comment),

  _venn_statement: ($) => choice(
    $.venn_title_statement,
    $.venn_set_statement,
    $.venn_union_statement,
    $.venn_text_statement,
    $.venn_indented_text_statement,
    $.venn_style_statement,
    $.venn_incomplete_union_statement,
    $.venn_malformed_statement,
  ),

  venn_title_statement: ($) => prec(40, seq(
    statementKeyword($, /title/i),
    optional(field('text', $.venn_title_text)),
  )),

  venn_set_statement: ($) => prec(40, seq(
    statementKeyword($, /set/i),
    field('expression', $.venn_set_expression),
    optional(field('label', $.venn_label)),
    optional(seq(
      valueDelimiter($),
      field('value', $.venn_number),
    )),
  )),

  venn_union_statement: ($) => prec(40, seq(
    statementKeyword($, /union/i),
    field('expression', $.venn_intersection_expression),
    optional(field('label', $.venn_label)),
    optional(seq(
      valueDelimiter($),
      field('value', $.venn_number),
    )),
  )),

  venn_text_statement: ($) => prec(40, seq(
    statementKeyword($, /text/i),
    field('expression', $.venn_expression),
    field('text', $.venn_text_value),
    optional(field('label', $.venn_label)),
  )),

  venn_indented_text_statement: ($) => prec(50, seq(
    field('keyword', $.venn_indented_text_marker),
    field('text', $.venn_text_value),
    optional(field('label', $.venn_label)),
  )),

  venn_style_statement: ($) => prec(40, seq(
    statementKeyword($, /style/i),
    field('expression', $.venn_expression),
    field('properties', $.venn_style_list),
  )),

  venn_incomplete_union_statement: ($) => prec(-20, seq(
    statementKeyword($, /union/i),
    field('expression', $.venn_incomplete_intersection_expression),
    optional(field('recovery', $.venn_recovery_text)),
  )),

  venn_malformed_statement: ($) => prec(-100, field(
    'recovery',
    $.venn_malformed_text,
  )),

  venn_set_expression: ($) => field('set', $.venn_identifier),

  venn_expression: ($) => choice(
    $.venn_identifier,
    $.venn_intersection_expression,
  ),

  venn_intersection_expression: ($) => seq(
    field('set', $.venn_identifier),
    repeat1(seq(
      setDelimiter($),
      field('set', $.venn_identifier),
    )),
  ),

  venn_incomplete_intersection_expression: ($) => seq(
    field('set', $.venn_identifier),
    repeat(seq(
      setDelimiter($),
      field('set', $.venn_identifier),
    )),
    setDelimiter($),
  ),

  venn_identifier: ($) => choice(
    $.venn_bare_identifier,
    $.venn_quoted_identifier,
  ),

  venn_quoted_identifier: ($) => seq(
    field('open', alias('"', $.venn_quote)),
    optional(field('content', $.venn_quoted_identifier_content)),
    field('close', alias(token.immediate('"'), $.venn_quote)),
  ),

  venn_text_value: ($) => choice($.venn_identifier, $.venn_number),

  venn_label: ($) => choice(
    prec(10, seq(
      field('open', alias('[', $.venn_label_delimiter)),
      optional(field('text', choice(
        $.venn_quoted_label,
        $.venn_unquoted_label,
      ))),
      field('close', alias(token.immediate(']'), $.venn_label_delimiter)),
    )),
    prec(-10, seq(
      field('open', alias('[', $.venn_label_delimiter)),
      optional(field('text', choice(
        $.venn_unclosed_quoted_label,
        $.venn_unclosed_unquoted_label,
      ))),
    )),
  ),

  venn_quoted_label: ($) => seq(
    field('open', alias(token.immediate('"'), $.venn_quote)),
    optional(field('content', $.venn_quoted_label_content)),
    field('close', alias(token.immediate('"'), $.venn_quote)),
  ),

  venn_unclosed_quoted_label: ($) => seq(
    field('open', alias(token.immediate('"'), $.venn_quote)),
    optional(field('content', $.venn_quoted_label_content)),
  ),

  venn_style_list: ($) => seq(
    field('property', $.venn_style_field),
    repeat(seq(
      field('delimiter', alias(',', $.venn_style_delimiter)),
      field('property', $.venn_style_field),
    )),
  ),

  venn_style_field: ($) => seq(
    field('name', $.venn_style_property),
    valueDelimiter($),
    field('value', $.venn_style_value),
  ),

  venn_style_value: ($) => choice(
    $.venn_quoted_style_value,
    repeat1(choice(
      $.venn_color,
      $.venn_number,
      $.venn_style_atom,
    )),
  ),

  venn_quoted_style_value: ($) => seq(
    field('open', alias('"', $.venn_quote)),
    optional(field('content', $.venn_quoted_style_content)),
    field('close', alias(token.immediate('"'), $.venn_quote)),
  ),

  venn_color: (_) => token(prec(30, choice(
    /#[0-9a-fA-F]{3,8}/,
    /rgba\([ \t]*[0-9.]+[ \t]*,[ \t]*[0-9.]+[ \t]*,[ \t]*[0-9.]+[ \t]*,[ \t]*[0-9.]+[ \t]*\)/,
    /rgb\([ \t]*[0-9.]+[ \t]*,[ \t]*[0-9.]+[ \t]*,[ \t]*[0-9.]+[ \t]*\)/,
  ))),

  venn_indented_text_marker: (_) => token(prec(30, seq(/[ \t]+/, /text/i))),

  venn_bare_identifier: (_) => token(prec(10, /[A-Za-z_][A-Za-z0-9_-]*/)),

  venn_style_property: (_) => token(prec(10, /[A-Za-z_][A-Za-z0-9_-]*/)),

  venn_style_atom: (_) => token(prec(5, /[A-Za-z_][A-Za-z0-9_-]*/)),

  venn_number: (_) => token(prec(10, /[+-]?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)/)),

  venn_title_text: (_) => token(prec(
    5,
    /[^\s#;\r\n](?:[^#;\r\n])*/,
  )),

  venn_quoted_identifier_content: (_) => token.immediate(/[^"\r\n]+/),

  venn_quoted_label_content: (_) => token.immediate(/[^"\r\n]+/),

  venn_unquoted_label: (_) => token.immediate(prec(5, /[^\]"\r\n]+/)),

  venn_unclosed_unquoted_label: (_) => token.immediate(prec(-5, /[^\]"\r\n]+/)),

  venn_quoted_style_content: (_) => token.immediate(/[^"\r\n]+/),

  venn_recovery_text: (_) => token(prec(-50, /[^\r\n]+/)),

  venn_malformed_text: (_) => token(prec(-100, /[^\r\n]+/)),
};

module.exports = { vennRules };
