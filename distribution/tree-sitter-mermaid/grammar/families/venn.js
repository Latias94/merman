const vennRules = {
  venn_diagram: ($) => choice(
    seq(
      field('header', $.venn_header),
      optional(field('body', $.venn_body)),
    ),
    field('header', alias($._venn_header_eof, $.venn_header)),
  ),

  venn_header: ($) => seq(
    field('keyword', alias('venn-beta', $.diagram_keyword)),
    field('terminator', $._line_ending),
  ),

  _venn_header_eof: ($) => field(
    'keyword',
    alias('venn-beta', $.diagram_keyword),
  ),

  venn_body: ($) => repeat1(choice(
    $.venn_set_statement,
    $.venn_union_statement,
    $.venn_text_statement,
    $.venn_indented_text_statement,
    $.venn_style_statement,
    $.venn_title_statement,
    $.comment,
    $._blank_line,
    $.venn_unstructured_body,
  )),

  venn_set_statement: ($) => prec.right(seq(
    field('keyword', 'set'),
    field('set', $.venn_identifier),
    optional(field('label', $.venn_bracket_label)),
    optional(seq(':', field('value', $.number))),
    optional($._line_ending),
  )),

  venn_union_statement: ($) => prec.right(seq(
    field('keyword', 'union'),
    field('sets', $.venn_identifier_list),
    optional(field('label', $.venn_bracket_label)),
    optional(seq(':', field('value', $.number))),
    optional($._line_ending),
  )),

  venn_text_statement: ($) => prec.right(seq(
    field('keyword', 'text'),
    field('sets', $.venn_identifier_list),
    field('text', $.venn_text_value),
    optional(field('label', $.venn_bracket_label)),
    optional($._line_ending),
  )),

  venn_indented_text_statement: ($) => prec.right(seq(
    field('keyword', $.venn_indented_text_marker),
    field('text', $.venn_text_value),
    optional(field('label', $.venn_bracket_label)),
    optional($._line_ending),
  )),

  venn_style_statement: ($) => prec.right(seq(
    field('keyword', 'style'),
    field('sets', $.venn_identifier_list),
    field('properties', $.venn_style_list),
    optional($._line_ending),
  )),

  venn_title_statement: ($) => prec.right(seq(
    field('keyword', 'title'),
    optional(field('text', $.venn_line_text)),
    optional($._line_ending),
  )),

  venn_identifier_list: ($) => seq(
    $.venn_identifier,
    repeat(seq(',', $.venn_identifier)),
  ),

  venn_identifier: ($) => choice($.identifier, $.quoted_string),

  venn_text_value: ($) => choice($.identifier, $.quoted_string, $.number),

  venn_bracket_label: ($) => seq(
    '[',
    field('text', choice($.quoted_string, $.venn_bracket_text)),
    ']',
  ),

  venn_bracket_text: (_) => token(prec(-1, /[^\]"\r\n]+/)),

  venn_style_list: ($) => seq(
    $.venn_style_field,
    repeat(seq(',', $.venn_style_field)),
  ),

  venn_style_field: ($) => seq(
    field('name', $.identifier),
    ':',
    field('value', $.venn_style_value),
  ),

  venn_style_value: ($) => choice(
    $.quoted_string,
    $.venn_color,
    $.number,
    $.identifier,
  ),

  venn_color: (_) => token(choice(
    /#[0-9a-fA-F]{3,8}/,
    /rgba\([0-9., \t]+\)/,
    /rgb\([0-9., \t]+\)/,
  )),

  venn_indented_text_marker: (_) => token(prec(30, seq(/[ \t]+/, 'text'))),

  venn_line_text: (_) => token(prec(-5, /[^\r\n]+/)),

  venn_unstructured_body: ($) => prec.right(seq(
    alias($.unstructured_line, $.unstructured_body),
    optional($._line_ending),
  )),
};

module.exports = { vennRules };
