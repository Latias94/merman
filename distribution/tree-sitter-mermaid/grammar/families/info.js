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
    repeat1($._info_terminated_body_item),
    seq(
      repeat($._info_terminated_body_item),
      $._info_eof_body_item,
    ),
  ),

  info_show_statement: ($) => prec.right(seq(
    field(
      'keyword',
      alias(token(prec(20, 'showInfo')), $.statement_keyword),
    ),
    optional(choice(
      field('comment', $.comment),
      field('directive', $.directive),
    )),
  )),

  _info_common_statement: ($) => choice(
    $.langium_title_statement,
    $.langium_acc_title_statement,
    $.langium_acc_descr_statement,
  ),

  _info_body_statement: ($) => choice(
    field('statement', $._info_common_statement),
    field('recovery', $.info_recovery_statement),
  ),

  _info_terminated_body_item: ($) => choice(
    field('show_info', $.info_show_statement),
    $._langium_newline,
    seq(choice($.comment, $.directive), $._langium_newline),
    seq($._info_body_statement, $._langium_newline),
  ),

  _info_eof_body_item: ($) => choice(
    $.comment,
    $.directive,
    $._info_body_statement,
  ),

  info_recovery_statement: ($) => field('text', $.info_recovery_text),

  // Horizontal layout is an extra, not an invalid statement. Requiring the
  // first visible character keeps whitespace-only lines in NEWLINE* while
  // preserving line-local recovery for actual malformed content.
  info_recovery_text: (_) => token(prec(
    1,
    /[^ \t\f\u00a0%\r\n][^\r\n]*/,
  )),
};

module.exports = { infoRules };
