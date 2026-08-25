// Source translation: Mermaid 11.16.1
// packages/parser/src/language/railroad-ebnf/railroad-ebnf.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const {
  railroadCStyleBlockCommentToken,
  railroadEscapedStringToken,
  railroadIdentifierToken,
  railroadMetadataStatements,
  railroadSuffixQuantifierToken,
  railroadUnclosedEscapedStringToken,
} = require('./railroad-shared');

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(40, 'railroad-ebnf-beta')), $.diagram_keyword),
);

const layout = ($) => optional($._railroad_ebnf_layout);

const railroadEbnfRules = {
  _railroad_ebnf_layout: ($) => repeat1(choice(
    $._line_ending,
    $.comment,
    $.directive,
    $.railroad_ebnf_block_comment,
    $.railroad_ebnf_iso_comment,
  )),

  railroad_ebnf_diagram: ($) => seq(
    field('header', $.railroad_ebnf_header),
    optional(field('body', $.railroad_ebnf_body)),
  ),

  railroad_ebnf_header: ($) => diagramKeyword($),

  railroad_ebnf_body: ($) => repeat1(choice(
    railroadMetadataStatements($),
    $.railroad_ebnf_rule,
    $.railroad_ebnf_incomplete_rule,
    $.railroad_ebnf_block_comment,
    $.railroad_ebnf_iso_comment,
    $.comment,
    $.directive,
    $._line_ending,
  )),

  railroad_ebnf_rule: ($) => prec(20, seq(
    field('name', $.railroad_ebnf_identifier),
    layout($),
    field('operator', $.railroad_ebnf_assignment_operator),
    layout($),
    field('definition', $.railroad_ebnf_choice),
    layout($),
    field('terminator', ';'),
  )),

  railroad_ebnf_incomplete_rule: ($) => prec(-30, seq(
    field('name', $.railroad_ebnf_identifier),
    layout($),
    field('operator', $.railroad_ebnf_assignment_operator),
    layout($),
    field('terminator', ';'),
  )),

  railroad_ebnf_assignment_operator: (_) => token(prec(20, choice('::=', '='))),

  railroad_ebnf_choice: ($) => prec.right(seq(
    field('alternative', $.railroad_ebnf_sequence),
    repeat(seq(
      layout($),
      field('operator', alias('|', $.railroad_ebnf_choice_operator)),
      layout($),
      field('alternative', $.railroad_ebnf_sequence),
    )),
  )),

  railroad_ebnf_sequence: ($) => prec.right(seq(
    field('element', $.railroad_ebnf_term),
    repeat(seq(
      layout($),
      optional(field('delimiter', ',')),
      layout($),
      field('element', $.railroad_ebnf_term),
    )),
  )),

  railroad_ebnf_term: ($) => prec.right(seq(
    field('base', $.railroad_ebnf_primary),
    repeat(field('postfix', $.railroad_ebnf_postfix)),
  )),

  railroad_ebnf_primary: ($) => choice(
    $.railroad_ebnf_terminal,
    $.railroad_ebnf_unclosed_terminal,
    $.railroad_ebnf_reference,
    $.railroad_ebnf_special_sequence,
    $.railroad_ebnf_group,
    $.railroad_ebnf_optional_group,
    $.railroad_ebnf_repetition_group,
  ),

  railroad_ebnf_terminal: ($) => field('value', $.railroad_ebnf_string),

  railroad_ebnf_unclosed_terminal: ($) => field(
    'value',
    $.railroad_ebnf_unclosed_string,
  ),

  railroad_ebnf_reference: ($) => field('name', $.railroad_ebnf_identifier),

  railroad_ebnf_special_sequence: ($) => seq(
    field('open', '?'),
    field('text', $.railroad_ebnf_special_text),
    field('close', token.immediate('?')),
  ),

  railroad_ebnf_group: ($) => seq(
    field('open', '('),
    layout($),
    field('element', $.railroad_ebnf_choice),
    layout($),
    field('close', ')'),
  ),

  railroad_ebnf_optional_group: ($) => seq(
    field('open', '['),
    layout($),
    field('element', $.railroad_ebnf_choice),
    layout($),
    field('close', ']'),
  ),

  railroad_ebnf_repetition_group: ($) => seq(
    field('open', '{'),
    layout($),
    field('element', $.railroad_ebnf_choice),
    layout($),
    field('close', '}'),
  ),

  railroad_ebnf_postfix: ($) => choice(
    $.railroad_ebnf_quantifier,
    $.railroad_ebnf_exception,
  ),

  railroad_ebnf_quantifier: (_) => railroadSuffixQuantifierToken(),

  railroad_ebnf_exception: ($) => seq(
    field('operator', alias('-', $.railroad_ebnf_exception_operator)),
    layout($),
    field('except', $.railroad_ebnf_primary),
  ),

  railroad_ebnf_identifier: (_) => railroadIdentifierToken(),

  railroad_ebnf_string: (_) => railroadEscapedStringToken(),

  railroad_ebnf_unclosed_string: (_) => railroadUnclosedEscapedStringToken(),

  // Tree-sitter regexes do not support the Langium lookahead. Structuring the
  // delimiters separately keeps the same non-empty, non-semicolon language.
  railroad_ebnf_special_text: (_) => token.immediate(
    /[ \t\r\n]*[^?; \t\r\n](?:[^?;]*[^?; \t\r\n])?[ \t\r\n]*/,
  ),

  railroad_ebnf_block_comment: (_) => railroadCStyleBlockCommentToken(),

  railroad_ebnf_iso_comment: (_) => token(prec(
    40,
    /\(\*[^*]*(?:\*+[^*)][^*]*)*\*+\)/,
  )),
};

const railroadEbnfConflicts = ($) => [
  [$.railroad_ebnf_sequence],
];

module.exports = { railroadEbnfConflicts, railroadEbnfRules };
