// Source translation: Mermaid 11.16.1
// packages/parser/src/language/railroad-abnf/railroad-abnf.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const { railroadMetadataStatements } = require('./railroad-shared');

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(40, 'railroad-abnf-beta')), $.diagram_keyword),
);

const layout = ($) => optional($._railroad_abnf_layout);

const railroadAbnfRules = {
  _railroad_abnf_layout: ($) => repeat1(choice(
    $._line_ending,
    $.comment,
    $.directive,
  )),

  railroad_abnf_diagram: ($) => seq(
    field('header', $.railroad_abnf_header),
    optional(field('body', $.railroad_abnf_body)),
  ),

  railroad_abnf_header: ($) => diagramKeyword($),

  railroad_abnf_body: ($) => repeat1(choice(
    railroadMetadataStatements($),
    $.railroad_abnf_rule,
    $.railroad_abnf_incomplete_rule,
    $.railroad_abnf_comment,
    $.comment,
    $.directive,
    $._line_ending,
  )),

  railroad_abnf_rule: ($) => prec(20, seq(
    field('name', $.railroad_abnf_rule_name),
    layout($),
    field('operator', alias('=', $.railroad_abnf_assignment_operator)),
    layout($),
    field('definition', $.railroad_abnf_alternation),
    layout($),
    field('terminator', ';'),
  )),

  railroad_abnf_incomplete_rule: ($) => prec(-30, seq(
    field('name', $.railroad_abnf_rule_name),
    layout($),
    field('operator', alias('=', $.railroad_abnf_assignment_operator)),
    layout($),
    field('terminator', ';'),
  )),

  railroad_abnf_alternation: ($) => prec.right(seq(
    field('alternative', $.railroad_abnf_concatenation),
    repeat(seq(
      layout($),
      field('operator', alias('/', $.railroad_abnf_alternation_operator)),
      layout($),
      field('alternative', $.railroad_abnf_concatenation),
    )),
  )),

  railroad_abnf_concatenation: ($) => repeat1(field(
    'element',
    $.railroad_abnf_element,
  )),

  railroad_abnf_element: ($) => seq(
    optional(field('repeat', $.railroad_abnf_repeat)),
    field('primary', $.railroad_abnf_primary),
  ),

  railroad_abnf_primary: ($) => choice(
    $.railroad_abnf_string,
    $.railroad_abnf_unclosed_string,
    $.railroad_abnf_numeric_value,
    $.railroad_abnf_reference,
    $.railroad_abnf_group,
    $.railroad_abnf_optional_group,
  ),

  railroad_abnf_reference: ($) => field('name', $.railroad_abnf_rule_name),

  railroad_abnf_group: ($) => seq(
    field('open', '('),
    layout($),
    field('element', $.railroad_abnf_alternation),
    layout($),
    field('close', ')'),
  ),

  railroad_abnf_optional_group: ($) => seq(
    field('open', '['),
    layout($),
    field('element', $.railroad_abnf_alternation),
    layout($),
    field('close', ']'),
  ),

  railroad_abnf_rule_name: (_) => token(prec(20, /[A-Za-z][A-Za-z0-9-]*/)),

  railroad_abnf_string: (_) => token(prec(10, seq('"', /[^"]*/, '"'))),

  railroad_abnf_unclosed_string: (_) => token(prec(-20, seq('"', /[^"\r\n]*/))),

  railroad_abnf_numeric_value: (_) => token(prec(
    20,
    /%[xXdDbB][0-9A-Fa-f]+(?:-[0-9A-Fa-f]+|\.[0-9A-Fa-f]+)*/,
  )),

  railroad_abnf_repeat: (_) => token(prec(20, choice(
    /[0-9]*\*[0-9]*/,
    /[0-9]+/,
  ))),

  // ABNF uses semicolon line comments. This token is only reachable between
  // rules, so the rule terminator remains unambiguous in expression states.
  railroad_abnf_comment: (_) => token(prec(10, /;[^\r\n]*/)),
};

module.exports = { railroadAbnfRules };
