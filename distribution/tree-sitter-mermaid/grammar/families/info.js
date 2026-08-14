// Source translation: Mermaid 11.16.1
// packages/parser/src/language/info/info.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const infoRules = {
  info_diagram: ($) => seq(
    field('header', $.info_header),
    optional(seq(
      $._langium_body_boundary,
      optional(field('body', $.info_body)),
    )),
  ),

  // The diagram-level boundary accepts `info showInfo` and
  // `info\nshowInfo` while preserving the exact keyword ranges.
  info_header: ($) => seq(
    field(
      'keyword',
      alias(token(prec(20, 'info')), $.diagram_keyword),
    ),
  ),

  info_body: ($) => choice(
    seq(
      repeat($._info_trivia),
      field('show_info', $.info_show_statement),
      repeat($._info_trivia),
      repeat(seq(
        field('statement', $._info_common_statement),
        repeat($._info_trivia),
      )),
    ),
    seq(
      repeat($._info_trivia),
      repeat1(seq(
        field('statement', $._info_common_statement),
        repeat($._info_trivia),
      )),
    ),
    repeat1($._info_trivia),
  ),

  info_show_statement: ($) => field(
    'keyword',
    alias(token(prec(20, 'showInfo')), $.statement_keyword),
  ),

  _info_common_statement: ($) => choice(
    $.langium_title_statement,
    $.langium_acc_title_statement,
    $.langium_acc_descr_statement,
  ),

  _info_trivia: ($) => choice(
    $.comment,
    $.directive,
    $._langium_newline,
  ),
};

module.exports = { infoRules };
