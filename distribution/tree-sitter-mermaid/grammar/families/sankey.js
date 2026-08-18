// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/sankey/parser/sankey.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, /sankey(?:-beta)?/i)), $.diagram_keyword),
);

const recordDelimiter = ($) => field(
  'delimiter',
  alias(',', $.sankey_record_delimiter),
);

const sankeyConflicts = ($) => [
  [$.sankey_quoted_field, $.sankey_unclosed_quoted_field],
];

const sankeyRules = {
  sankey_diagram: ($) => seq(
    field('header', $.sankey_header),
    field('body', $.sankey_body),
  ),

  sankey_header: ($) => seq(
    diagramKeyword($),
    field('terminator', $._line_ending),
  ),

  sankey_body: ($) => choice(
    repeat1($._sankey_line_item),
    seq(
      repeat($._sankey_line_item),
      $._sankey_eof_item,
    ),
  ),

  _sankey_line_item: ($) => choice(
    seq(
      choice(
        $.sankey_record,
        $.sankey_overflow_record,
        $.sankey_incomplete_record,
        $.sankey_unclosed_record,
        $.sankey_malformed_record,
        $.comment,
      ),
      $._line_ending,
    ),
    $._blank_line,
  ),

  _sankey_eof_item: ($) => choice(
    $.sankey_record,
    $.sankey_overflow_record,
    $.sankey_incomplete_record,
    $.sankey_unclosed_record,
    $.sankey_malformed_record,
    $.comment,
  ),

  sankey_record: ($) => prec(30, seq(
    field('source', optional($.sankey_field)),
    recordDelimiter($),
    field('target', optional($.sankey_field)),
    recordDelimiter($),
    field('value', optional($.sankey_field)),
  )),

  sankey_overflow_record: ($) => prec(-20, seq(
    field('source', optional($.sankey_field)),
    recordDelimiter($),
    field('target', optional($.sankey_field)),
    recordDelimiter($),
    field('value', optional($.sankey_field)),
    repeat1(seq(
      recordDelimiter($),
      field('overflow', optional($.sankey_field)),
    )),
  )),

  sankey_incomplete_record: ($) => prec(-30, seq(
    field('source', optional($.sankey_field)),
    recordDelimiter($),
    field('target', optional($.sankey_field)),
  )),

  sankey_unclosed_record: ($) => prec(-40, choice(
    field('source', $.sankey_unclosed_quoted_field),
    seq(
      field('source', optional($.sankey_field)),
      recordDelimiter($),
      field('target', $.sankey_unclosed_quoted_field),
    ),
    seq(
      field('source', optional($.sankey_field)),
      recordDelimiter($),
      field('target', optional($.sankey_field)),
      recordDelimiter($),
      field('value', $.sankey_unclosed_quoted_field),
    ),
  )),

  sankey_malformed_record: ($) => prec(-100, field(
    'recovery',
    alias($.sankey_unquoted_field, $.sankey_malformed_record_text),
  )),

  sankey_field: ($) => choice(
    $.sankey_quoted_field,
    $.sankey_unquoted_field,
  ),

  sankey_quoted_field: ($) => prec(10, seq(
    field('open', alias('"', $.sankey_quote)),
    repeat(field('content', choice(
      $.sankey_escaped_quote,
      $.sankey_quoted_content,
      alias($._line_ending, $.sankey_quoted_line_break),
    ))),
    field('close', alias(token.immediate('"'), $.sankey_quote)),
  )),

  sankey_unclosed_quoted_field: ($) => prec(-10, seq(
    field('open', alias('"', $.sankey_quote)),
    repeat(field('content', choice(
      $.sankey_escaped_quote,
      $.sankey_quoted_content,
    ))),
  )),

  sankey_escaped_quote: (_) => token.immediate(prec(20, '""')),

  sankey_quoted_content: (_) => token.immediate(prec(5, /[^"\r\n\u0000]+/)),

  sankey_unquoted_field: (_) => token(prec(-1, /[^,"\r\n]+/)),
};

module.exports = { sankeyConflicts, sankeyRules };
