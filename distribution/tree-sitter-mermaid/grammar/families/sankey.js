const sankeyRules = {
  sankey_diagram: ($) => seq(
    field('header', $.sankey_header),
    optional(field('body', $.sankey_body)),
  ),

  sankey_header: ($) => seq(
    field(
      'keyword',
      alias(
        token(prec(20, /sankey(?:-[bB][eE][tT][aA])?/)),
        $.diagram_keyword,
      ),
    ),
    field('terminator', $._line_ending),
  ),

  sankey_body: ($) => repeat1(choice(
    $.sankey_record,
    $._blank_line,
    $.sankey_unstructured_body,
  )),

  sankey_record: ($) => prec.right(seq(
    field('source', optional($.sankey_field)),
    ',',
    field('target', optional($.sankey_field)),
    ',',
    field('value', optional($.sankey_field)),
    optional($._line_ending),
  )),

  sankey_field: ($) => choice($.sankey_quoted_field, $.sankey_unquoted_field),

  sankey_quoted_field: ($) => seq(
    '"',
    repeat(choice($.sankey_escaped_quote, $.sankey_quoted_content)),
    '"',
  ),

  sankey_escaped_quote: (_) => token('""'),

  sankey_quoted_content: (_) => token(prec(-1, /[^"\u0000]+/)),

  sankey_unquoted_field: (_) => token(prec(-1, /[^,"\r\n]+/)),

  sankey_unstructured_body: ($) => prec.right(seq(
    alias($.unstructured_line, $.unstructured_body),
    optional($._line_ending),
  )),
};

module.exports = { sankeyRules };
