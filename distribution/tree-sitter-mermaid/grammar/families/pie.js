// Source translation: Mermaid 11.16.1
// packages/parser/src/language/pie/pie.langium:4-20 and the imported
// packages/parser/src/language/common/common.langium:6-34 via shared/langium.js.
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const pieStatementEnd = ($) => seq(
  optional(field('comment', $.comment)),
  optional(field('terminator', $._langium_newline)),
);

const pieRules = {
  pie_diagram: ($) => choice(
    prec.dynamic(10, seq(
      field('header', alias($._pie_show_data_header, $.pie_header)),
      optional(seq(
        $._langium_body_boundary,
        optional(field('body', $.pie_body)),
      )),
    )),
    seq(
      field('header', $.pie_header),
      optional(seq(
        $._langium_body_boundary,
        optional(field('body', $.pie_body)),
      )),
    ),
  ),

  // Mermaid permits common statements on the header line. Leaving the line
  // boundary to the body keeps same-line and following-line statements equal.
  pie_header: ($) => field(
    'keyword',
    alias(token(prec(20, 'pie')), $.diagram_keyword),
  ),

  _pie_show_data_header: ($) => seq(
    field('keyword', alias(token(prec(20, 'pie')), $.diagram_keyword)),
    $._pie_show_data_clause,
  ),

  _pie_show_data_clause: ($) => seq(
    token.immediate(/[ \t]+/),
    field(
      'option',
      alias(token.immediate('showData'), $.pie_show_data_option),
    ),
  ),

  pie_body: ($) => repeat1(choice(
    $.langium_title_statement,
    $.langium_acc_title_statement,
    $.langium_acc_descr_statement,
    $.pie_section,
    $.pie_incomplete_section,
    $.comment,
    $.directive,
    $._blank_line,
    $.pie_recovery_statement,
  )),

  pie_section: ($) => prec.right(seq(
    field('label', $.langium_string),
    field('delimiter', alias(':', $.pie_section_delimiter)),
    optional($._langium_inline_space),
    field('value', $.pie_number),
    pieStatementEnd($),
  )),

  // A missing value is an editing intermediate with a stable label field.
  pie_incomplete_section: ($) => prec(-1, seq(
    field('label', $.langium_string),
    field('delimiter', alias(':', $.pie_section_delimiter)),
    optional($._langium_inline_space),
  )),

  // NUMBER_PIE accepts signed integers and decimals. Decimal integer parts
  // may contain leading zeroes; integer-only values may not.
  pie_number: (_) => token(/-?(?:[0-9]+\.[0-9]+|0|[1-9][0-9]*)/),

  // This is a finite, line-local recovery leaf. It cannot consume a valid
  // sibling on the next line and never substitutes for the complete body.
  pie_recovery_statement: ($) => prec.right(seq(
    field('text', $.pie_recovery_text),
    optional(field('terminator', $._langium_newline)),
  )),

  pie_recovery_text: (_) => token(prec(-100, /[^\r\n]+/)),
};

const pieConflicts = ($) => [
  [$.pie_header, $._pie_show_data_header],
];

module.exports = { pieConflicts, pieRules };
