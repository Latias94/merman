// Source translation: Mermaid 11.16.1
// packages/parser/src/language/cynefin/cynefin.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const cynefinRules = {
  cynefin_diagram: ($) => choice(
    seq(
      field('header', alias($._cynefin_colon_header, $.cynefin_header)),
      optional(field('body', $.cynefin_body)),
    ),
    seq(
      field('header', $.cynefin_header),
      optional(seq(
        $._langium_body_boundary,
        optional(field('body', $.cynefin_body)),
      )),
    ),
  ),

  // Bare headers use the shared Langium boundary. The colon form is separate
  // because Mermaid also accepts an immediately adjacent body after it.
  cynefin_header: ($) => seq(
    field(
      'keyword',
      alias(token(prec(20, 'cynefin-beta')), $.diagram_keyword),
    ),
  ),

  _cynefin_colon_header: ($) => seq(
    field(
      'keyword',
      alias(token(prec(20, 'cynefin-beta')), $.diagram_keyword),
    ),
    field('colon', token.immediate(':')),
  ),

  cynefin_body: ($) => choice(
    repeat1($._cynefin_terminated_or_domain_item),
    seq(
      repeat($._cynefin_terminated_or_domain_item),
      $._cynefin_eof_line_item,
    ),
  ),

  _cynefin_line_statement: ($) => choice(
    $.langium_title_statement,
    $.langium_acc_title_statement,
    $.langium_acc_descr_statement,
    $.cynefin_transition_statement,
    $.cynefin_incomplete_transition_statement,
  ),

  _cynefin_terminated_or_domain_item: ($) => choice(
    $.cynefin_domain_block,
    $._langium_newline,
    seq(choice($.comment, $.directive), $._langium_newline),
    seq($._cynefin_line_statement, $._langium_newline),
  ),

  _cynefin_eof_line_item: ($) => choice(
    $.comment,
    $.directive,
    $._cynefin_line_statement,
  ),

  cynefin_domain_block: ($) => prec.right(seq(
    field('domain', $.cynefin_domain_name),
    repeat(choice(
      field('item', $.cynefin_domain_item),
      $.comment,
      $.directive,
      $._langium_newline,
    )),
  )),

  cynefin_domain_item: ($) => seq(
    optional($._langium_inline_space),
    field(
      'label',
      choice($.langium_string, $.langium_unclosed_string),
    ),
  ),

  cynefin_transition_statement: ($) => prec(2, prec.right(seq(
    field('from', $.cynefin_domain_name),
    optional($._langium_inline_space),
    field('operator', $.cynefin_transition_operator),
    optional($._langium_inline_space),
    field('to', alias(
      token.immediate(prec(20, choice(
        'complex',
        'complicated',
        'clear',
        'chaotic',
        'confusion',
      ))),
      $.cynefin_domain_name,
    )),
    optional(seq(
      field('delimiter', ':'),
      optional($._langium_inline_space),
      field('label', choice($.langium_string, $.langium_unclosed_string)),
    )),
    optional(choice(
      field('comment', $.comment),
      field('directive', $.directive),
    )),
  ))),

  cynefin_incomplete_transition_statement: ($) => prec(-10, seq(
    field('from', $.cynefin_domain_name),
    optional($._langium_inline_space),
    field('operator', $.cynefin_transition_operator),
  )),

  cynefin_domain_name: (_) => token(prec(20, choice(
    'complex',
    'complicated',
    'clear',
    'chaotic',
    'confusion',
  ))),

  cynefin_transition_operator: (_) => token.immediate(prec(20, '-->')),
};

module.exports = { cynefinRules };
