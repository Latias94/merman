// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/ishikawa/parser/ishikawa.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, /ishikawa(?:-beta)?/i)), $.diagram_keyword),
);

const statement = ($) => prec.right(seq(
  optional(field('indentation', $.ishikawa_indentation)),
  field('label', $.ishikawa_label),
));

const ishikawaRules = {
  ishikawa_diagram: ($) => choice(
    seq(
      field('header', $.ishikawa_header),
      optional(field('body', $.ishikawa_body)),
    ),
    seq(
      field('header', alias($._ishikawa_inline_header, $.ishikawa_header)),
      field('body', $.ishikawa_body),
    ),
  ),

  ishikawa_header: ($) => prec(30, seq(
    diagramKeyword($),
    optional(token.immediate(/[ \t]+/)),
    optional(choice($.comment, $.directive)),
    field('terminator', $._line_ending),
  )),

  _ishikawa_inline_header: ($) => seq(
    diagramKeyword($),
    token.immediate(/[ \t]+/),
  ),

  ishikawa_body: ($) => choice(
    seq(
      repeat($._ishikawa_trivia_line),
      field('effect', $.ishikawa_effect_statement),
      optional(seq(
        $._line_ending,
        repeat($._ishikawa_cause_line_item),
        optional($._ishikawa_cause_eof_item),
      )),
    ),
    repeat1($._ishikawa_trivia_line),
    seq(
      repeat($._ishikawa_trivia_line),
      $._ishikawa_trivia_eof_item,
    ),
  ),

  _ishikawa_trivia_line: ($) => choice(
    seq(
      optional($.ishikawa_indentation),
      choice($.comment, $.directive),
      $._line_ending,
    ),
    $.ishikawa_indented_blank_line,
    $._blank_line,
  ),

  _ishikawa_trivia_eof_item: ($) => choice(
    seq(
      optional($.ishikawa_indentation),
      choice($.comment, $.directive),
    ),
    $.ishikawa_indented_blank_eof,
  ),

  _ishikawa_cause_line_item: ($) => choice(
    seq(field('cause', $.ishikawa_cause_statement), $._line_ending),
    $._ishikawa_trivia_line,
  ),

  _ishikawa_cause_eof_item: ($) => choice(
    field('cause', $.ishikawa_cause_statement),
    $._ishikawa_trivia_eof_item,
  ),

  ishikawa_effect_statement: ($) => statement($),

  ishikawa_cause_statement: ($) => statement($),

  ishikawa_indented_blank_line: ($) => seq(
    field('indentation', $.ishikawa_indentation),
    field('terminator', $._line_ending),
  ),

  ishikawa_indented_blank_eof: ($) => field(
    'indentation',
    $.ishikawa_indentation,
  ),

  ishikawa_indentation: (_) => token(prec(20, /[ \t]+/)),

  ishikawa_label: (_) => token(prec(-5, /[^\r\n]+/)),
};

module.exports = { ishikawaRules };
