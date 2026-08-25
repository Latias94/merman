// Source translation: Mermaid 11.16.1
// packages/parser/src/language/railroad-peg/railroad-peg.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const {
  railroadEscapedStringToken,
  railroadIdentifierToken,
  railroadMetadataStatements,
  railroadSuffixQuantifierToken,
  railroadUnclosedEscapedStringToken,
} = require('./railroad-shared');

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(40, 'railroad-peg-beta')), $.diagram_keyword),
);

const layout = ($) => optional($._railroad_peg_layout);

const railroadPegRules = {
  _railroad_peg_layout: ($) => repeat1(choice(
    $._line_ending,
    $.comment,
    $.directive,
    $.railroad_peg_comment,
  )),

  railroad_peg_diagram: ($) => seq(
    field('header', $.railroad_peg_header),
    optional(field('body', $.railroad_peg_body)),
  ),

  railroad_peg_header: ($) => diagramKeyword($),

  railroad_peg_body: ($) => repeat1(choice(
    railroadMetadataStatements($),
    $.railroad_peg_rule,
    $.railroad_peg_incomplete_rule,
    $.railroad_peg_comment,
    $.comment,
    $.directive,
    $._line_ending,
  )),

  railroad_peg_rule: ($) => prec(20, seq(
    field('name', $.railroad_peg_identifier),
    layout($),
    field('operator', alias('<-', $.railroad_peg_assignment_operator)),
    layout($),
    field('definition', $.railroad_peg_ordered_choice),
    layout($),
    field('terminator', ';'),
  )),

  railroad_peg_incomplete_rule: ($) => prec(-30, seq(
    field('name', $.railroad_peg_identifier),
    layout($),
    field('operator', alias('<-', $.railroad_peg_assignment_operator)),
    layout($),
    field('terminator', ';'),
  )),

  railroad_peg_ordered_choice: ($) => prec.right(seq(
    field('alternative', $.railroad_peg_sequence),
    repeat(seq(
      layout($),
      field('operator', alias('/', $.railroad_peg_choice_operator)),
      layout($),
      field('alternative', $.railroad_peg_sequence),
    )),
  )),

  railroad_peg_sequence: ($) => repeat1(field(
    'element',
    $.railroad_peg_prefix,
  )),

  railroad_peg_prefix: ($) => seq(
    optional(field('operator', $.railroad_peg_prefix_operator)),
    field('suffix', $.railroad_peg_suffix),
  ),

  railroad_peg_prefix_operator: (_) => token(prec(20, choice('&', '!'))),

  railroad_peg_suffix: ($) => seq(
    field('primary', $.railroad_peg_primary),
    optional(field('operator', $.railroad_peg_suffix_operator)),
  ),

  railroad_peg_suffix_operator: (_) => railroadSuffixQuantifierToken(),

  railroad_peg_primary: ($) => choice(
    $.railroad_peg_literal,
    $.railroad_peg_unclosed_literal,
    $.railroad_peg_reference,
    $.railroad_peg_group,
    $.railroad_peg_any,
  ),

  railroad_peg_literal: ($) => field('value', $.railroad_peg_string),

  railroad_peg_unclosed_literal: ($) => field(
    'value',
    $.railroad_peg_unclosed_string,
  ),

  railroad_peg_reference: ($) => field('name', $.railroad_peg_identifier),

  railroad_peg_group: ($) => seq(
    field('open', '('),
    layout($),
    field('element', $.railroad_peg_ordered_choice),
    layout($),
    field('close', ')'),
  ),

  railroad_peg_any: (_) => '.',

  railroad_peg_identifier: (_) => railroadIdentifierToken(),

  railroad_peg_string: (_) => railroadEscapedStringToken(),

  railroad_peg_unclosed_string: (_) => railroadUnclosedEscapedStringToken(),

  railroad_peg_comment: (_) => token(prec(20, /#[^\r\n]*/)),
};

module.exports = { railroadPegRules };
