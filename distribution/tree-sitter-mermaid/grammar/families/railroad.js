// Source translation: Mermaid 11.16.1
// packages/parser/src/language/railroad/railroad.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const {
  railroadCStyleBlockCommentToken,
  railroadEscapedStringToken,
  railroadIdentifierToken,
  railroadMetadataStatements,
  railroadUnclosedEscapedStringToken,
} = require('./railroad-shared');

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(40, 'railroad-beta')), $.diagram_keyword),
);

const constructorKeyword = ($, keyword) => field(
  'kind',
  alias(token(prec(30, keyword)), $.railroad_constructor_keyword),
);

const layout = ($) => optional($._railroad_layout);

const unaryConstructor = ($, keyword) => seq(
  constructorKeyword($, keyword),
  layout($),
  field('open', '('),
  layout($),
  field('element', $.railroad_expression),
  layout($),
  field('close', ')'),
);

const stringConstructor = ($, keyword, fieldName) => seq(
  constructorKeyword($, keyword),
  layout($),
  field('open', '('),
  layout($),
  field(fieldName, choice($.railroad_string, $.railroad_unclosed_string)),
  layout($),
  field('close', ')'),
);

const variadicConstructor = ($, keyword, fieldName) => seq(
  constructorKeyword($, keyword),
  layout($),
  field('open', '('),
  layout($),
  field(fieldName, $.railroad_expression),
  repeat(seq(
    layout($),
    field('delimiter', ','),
    layout($),
    field(fieldName, $.railroad_expression),
  )),
  layout($),
  field('close', ')'),
);

const railroadRules = {
  _railroad_layout: ($) => repeat1(choice(
    $._line_ending,
    $.comment,
    $.directive,
    $.railroad_block_comment,
  )),

  railroad_diagram: ($) => seq(
    field('header', $.railroad_header),
    optional(field('body', $.railroad_body)),
  ),

  railroad_header: ($) => diagramKeyword($),

  railroad_body: ($) => repeat1(choice(
    railroadMetadataStatements($),
    $.railroad_rule,
    $.railroad_incomplete_rule,
    $.comment,
    $.directive,
    $.railroad_block_comment,
    $._line_ending,
  )),

  railroad_rule: ($) => prec(20, seq(
    field('name', $.railroad_identifier),
    layout($),
    field('operator', alias('=', $.railroad_assignment_operator)),
    layout($),
    field('definition', $.railroad_expression),
    layout($),
    field('terminator', ';'),
  )),

  // A missing definition is a common editing intermediate. Keep the recovery
  // anchored by the normal rule name, assignment, and semicolon instead of
  // accepting the rest of the line as an opaque fallback.
  railroad_incomplete_rule: ($) => prec(-30, seq(
    field('name', $.railroad_identifier),
    layout($),
    field('operator', alias('=', $.railroad_assignment_operator)),
    layout($),
    field('terminator', ';'),
  )),

  railroad_expression: ($) => choice(
    $.railroad_sequence,
    $.railroad_choice,
    $.railroad_optional,
    $.railroad_repetition,
    $.railroad_terminal,
    $.railroad_reference,
    $.railroad_special,
  ),

  railroad_sequence: ($) => variadicConstructor($, 'sequence', 'element'),

  railroad_choice: ($) => variadicConstructor($, 'choice', 'alternative'),

  railroad_optional: ($) => unaryConstructor($, 'optional'),

  railroad_repetition: ($) => unaryConstructor(
    $,
    choice('zeroOrMore', 'oneOrMore'),
  ),

  railroad_terminal: ($) => stringConstructor($, 'terminal', 'value'),

  railroad_reference: ($) => stringConstructor($, 'nonterminal', 'name'),

  railroad_special: ($) => stringConstructor($, 'special', 'text'),

  railroad_identifier: (_) => railroadIdentifierToken(),

  railroad_string: (_) => railroadEscapedStringToken(),

  railroad_unclosed_string: (_) => railroadUnclosedEscapedStringToken(),

  railroad_block_comment: (_) => railroadCStyleBlockCommentToken(),
};

module.exports = { railroadRules };
