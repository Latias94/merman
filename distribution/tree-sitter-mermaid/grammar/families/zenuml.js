// Source translation:
// - Mermaid 11.16.1 @ 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf
//   packages/mermaid-zenuml/src/{detector,parser,zenumlRenderer}.ts
// - @zenuml/core 3.50.1 @ 38404ccc14243ed54ab45b804b2eb6f2ca73af36
//   src/g4/sequenceLexer.g4 and src/g4/sequenceParser.g4
//
// The companion parser deliberately accepts incomplete editor states. This
// translation keeps those states local to the construct that introduced them;
// it does not use an opaque whole-line or whole-body fallback.

const keywordChoice = (keywords) => (
  keywords.length === 1
    ? keywords[0]
    : choice(...keywords)
);

const diagramKeyword = ($) => field(
  'keyword',
  alias(keywordChoice(['zenuml']), $.diagram_keyword),
);

const statementKeyword = ($, ...keywords) => field(
  'keyword',
  alias(keywordChoice(keywords), $.zenuml_statement_keyword),
);

const controlKeyword = ($, ...keywords) => field(
  'keyword',
  alias(keywordChoice(keywords), $.zenuml_control_keyword),
);

const operatorToken = ($, ...operators) => alias(
  operators.length === 1
    ? operators[0]
    : choice(...operators),
  $.zenuml_operator,
);

const binaryExpression = ($, precedence, ...operators) => prec.left(precedence, seq(
  field('left', $.zenuml_expression),
  field('operator', operatorToken($, ...operators)),
  field('right', $.zenuml_expression),
));

const modifiers = ($) => repeat(field('modifier', $.zenuml_modifier));

const zenumlRules = {
  zenuml_diagram: ($) => seq(
    field('header', $.zenuml_header),
    optional(field('body', $.zenuml_body)),
  ),

  zenuml_header: ($) => diagramKeyword($),

  zenuml_body: ($) => choice(
    repeat1($._zenuml_body_line),
    seq(
      repeat($._zenuml_body_line),
      $._zenuml_body_final_item,
    ),
  ),

  _zenuml_body_line: ($) => choice(
    prec.right(70, seq(
      field('declaration', $._zenuml_declaration),
      field('terminator', $._statement_terminator),
    )),
    prec.right(10, seq(
      field('statement', $._zenuml_statement),
      field('terminator', $._statement_terminator),
    )),
    prec.right(10, seq(
      field('comment', $.zenuml_comment),
      $._line_ending,
    )),
    $._blank_line,
  ),

  _zenuml_body_final_item: ($) => choice(
    prec.right(60, field('declaration', $._zenuml_declaration)),
    prec.right(-10, field('statement', $._zenuml_statement)),
    prec.right(-10, field('comment', $.zenuml_comment)),
  ),

  _zenuml_declaration: ($) => choice(
    $.zenuml_title_statement,
    $.zenuml_group_declaration,
    $.zenuml_starter_declaration,
    $.zenuml_participant_declaration,
  ),

  _zenuml_statement: ($) => choice(
    $.zenuml_if_statement,
    $.zenuml_parallel_statement,
    $.zenuml_optional_statement,
    $.zenuml_critical_statement,
    $.zenuml_section_statement,
    $.zenuml_loop_statement,
    $.zenuml_try_statement,
    $.zenuml_reference_statement,
    $.zenuml_creation_statement,
    $.zenuml_return_statement,
    $.zenuml_reply_message_statement,
    $.zenuml_async_message_statement,
    $.zenuml_sync_message_statement,
    $.zenuml_incomplete_message_statement,
    $.zenuml_divider_statement,
  ),

  zenuml_title_statement: ($) => prec.right(70, seq(
    statementKeyword($, 'title'),
    optional(seq(
      token.immediate(/[ \t]+/),
      optional(field('text', $.zenuml_title_text)),
    )),
  )),

  zenuml_title_text: (_) => token.immediate(/[^\r\n]+/),

  zenuml_group_declaration: ($) => prec.right(70, seq(
    statementKeyword($, 'group'),
    optional(field('name', $.zenuml_name)),
    optional(field('body', choice(
      $.zenuml_group_block,
      $.zenuml_unclosed_group_block,
    ))),
  )),

  zenuml_group_block: ($) => prec(30, seq(
    field('open', '{'),
    repeat($._zenuml_group_line),
    optional($._zenuml_group_final_item),
    field('close', '}'),
  )),

  zenuml_unclosed_group_block: ($) => prec(-50, seq(
    field('open', '{'),
    repeat($._zenuml_group_line),
    optional($._zenuml_group_final_item),
  )),

  _zenuml_group_line: ($) => choice(
    prec.right(10, seq(
      field('participant', $.zenuml_participant_declaration),
      field('terminator', $._statement_terminator),
    )),
    prec.right(10, seq(field('comment', $.zenuml_comment), $._line_ending)),
    $._blank_line,
  ),

  _zenuml_group_final_item: ($) => choice(
    prec.right(-10, field('participant', $.zenuml_participant_declaration)),
    prec.right(-10, field('comment', $.zenuml_comment)),
  ),

  zenuml_starter_declaration: ($) => prec.right(80, seq(
    field('annotation', $.zenuml_starter_annotation),
    optional(seq(
      field('open', '('),
      optional(field('participant', $.zenuml_name)),
      optional(field('close', ')')),
    )),
  )),

  zenuml_participant_declaration: ($) => prec.dynamic(20, choice(
    prec.right(60, seq(
      optional(field('type', $.zenuml_participant_annotation)),
      optional(field('stereotype', $.zenuml_stereotype)),
      optional(field('emoji', $.zenuml_emoji)),
      field('name', $.zenuml_name),
      optional(field('width', $.zenuml_width)),
      optional(field('alias', $.zenuml_alias_clause)),
      optional(field('color', $.zenuml_color)),
    )),
    field('stereotype', $.zenuml_stereotype),
    field('type', $.zenuml_participant_annotation),
  )),

  zenuml_alias_clause: ($) => prec.right(seq(
    field(
      'keyword',
      alias('as', $.zenuml_statement_keyword),
    ),
    optional(seq(
      token.immediate(/[ \t]+/),
      field('label', $.zenuml_name),
    )),
  )),

  zenuml_stereotype: ($) => prec.right(seq(
    field('open', choice('<<', '<')),
    optional(field('name', $.zenuml_name)),
    optional(field('close', choice('>>', '>'))),
  )),

  zenuml_emoji: ($) => seq(
    field('open', '['),
    field('name', $.zenuml_name),
    field('close', ']'),
  ),

  zenuml_width: ($) => field('value', $.zenuml_number),

  zenuml_if_statement: ($) => prec.right(60, seq(
    controlKeyword($, 'if'),
    field('condition', choice(
      $.zenuml_condition_clause,
      $.zenuml_unclosed_condition_clause,
    )),
    optional(field('body', $._zenuml_any_block)),
    repeat(seq(
      repeat($._line_ending),
      field('branch', $.zenuml_else_if_clause),
    )),
    optional(seq(
      repeat($._line_ending),
      field('branch', $.zenuml_else_clause),
    )),
  )),

  zenuml_else_if_clause: ($) => prec.right(seq(
    controlKeyword($, 'else'),
    controlKeyword($, 'if'),
    field('condition', choice(
      $.zenuml_condition_clause,
      $.zenuml_unclosed_condition_clause,
    )),
    optional(field('body', $._zenuml_any_block)),
  )),

  zenuml_else_clause: ($) => prec.right(seq(
    controlKeyword($, 'else'),
    optional(field('body', $._zenuml_any_block)),
  )),

  zenuml_parallel_statement: ($) => prec.right(50, seq(
    controlKeyword($, 'par'),
    optional(field('condition', choice(
      $.zenuml_condition_clause,
      $.zenuml_unclosed_condition_clause,
    ))),
    optional(field('body', $._zenuml_any_block)),
  )),

  zenuml_optional_statement: ($) => prec.right(50, seq(
    controlKeyword($, 'opt'),
    optional(field('condition', choice(
      $.zenuml_condition_clause,
      $.zenuml_unclosed_condition_clause,
    ))),
    optional(field('body', $._zenuml_any_block)),
  )),

  zenuml_critical_statement: ($) => prec.right(50, seq(
    controlKeyword($, 'critical'),
    optional(field('condition', choice(
      $.zenuml_condition_clause,
      $.zenuml_unclosed_condition_clause,
    ))),
    optional(field('body', $._zenuml_any_block)),
  )),

  zenuml_section_statement: ($) => prec.right(50, choice(
    seq(
      controlKeyword($, 'section', 'frame'),
      optional(field('condition', choice(
        $.zenuml_condition_clause,
        $.zenuml_unclosed_condition_clause,
      ))),
      optional(field('body', $._zenuml_any_block)),
    ),
    field('body', $._zenuml_any_block),
  )),

  zenuml_loop_statement: ($) => prec.right(50, seq(
    controlKeyword($, 'while', 'for', 'foreach', 'forEach', 'loop'),
    optional(field('condition', choice(
      $.zenuml_condition_clause,
      $.zenuml_unclosed_condition_clause,
    ))),
    optional(field('body', $._zenuml_any_block)),
  )),

  zenuml_try_statement: ($) => prec.right(60, seq(
    controlKeyword($, 'try'),
    field('body', $._zenuml_any_block),
    repeat(seq(
      repeat($._line_ending),
      field('catch', $.zenuml_catch_clause),
    )),
    optional(seq(
      repeat($._line_ending),
      field('finally', $.zenuml_finally_clause),
    )),
  )),

  zenuml_catch_clause: ($) => seq(
    controlKeyword($, 'catch'),
    optional(field('exception', choice(
      $.zenuml_argument_list,
      $.zenuml_unclosed_argument_list,
    ))),
    field('body', $._zenuml_any_block),
  ),

  zenuml_finally_clause: ($) => seq(
    controlKeyword($, 'finally'),
    field('body', $._zenuml_any_block),
  ),

  zenuml_reference_statement: ($) => prec.right(50, seq(
    controlKeyword($, 'ref'),
    field('participants', choice(
      $.zenuml_reference_list,
      $.zenuml_unclosed_reference_list,
    )),
  )),

  zenuml_reference_list: ($) => prec(30, seq(
    field('open', '('),
    optional(seq(
      field('participant', $.zenuml_name),
      repeat(seq(
        field('delimiter', ','),
        optional(field('participant', $.zenuml_name)),
      )),
    )),
    field('close', ')'),
  )),

  zenuml_unclosed_reference_list: ($) => prec(-40, seq(
    field('open', '('),
    optional(seq(
      field('participant', $.zenuml_name),
      repeat(seq(
        field('delimiter', ','),
        optional(field('participant', $.zenuml_name)),
      )),
    )),
  )),

  zenuml_creation_statement: ($) => prec.right(55, seq(
    modifiers($),
    optional(field('assignment', $.zenuml_assignment)),
    statementKeyword($, 'new'),
    optional(field('constructor', $.zenuml_construct)),
    optional(field('arguments', choice(
      $.zenuml_argument_list,
      $.zenuml_unclosed_argument_list,
    ))),
    optional(field('body', $._zenuml_any_block)),
  )),

  zenuml_sync_message_statement: ($) => prec.dynamic(10, prec.right(45, seq(
    modifiers($),
    optional(field('assignment', $.zenuml_assignment)),
    choice(
      seq(
        optional(seq(
          field('source', $.zenuml_endpoint),
          field('arrow', $.zenuml_arrow),
        )),
        field('target', $.zenuml_endpoint),
        field('delimiter', '.'),
        field('message', $.zenuml_call_chain),
      ),
      field('message', $.zenuml_signature),
    ),
    optional(field('body', $._zenuml_any_block)),
  ))),

  zenuml_async_message_statement: ($) => prec.right(50, seq(
    modifiers($),
    optional(seq(
      field('source', $.zenuml_endpoint),
      field('arrow', $.zenuml_arrow),
    )),
    field('target', $.zenuml_endpoint),
    field('delimiter', ':'),
    optional(seq(
      optional(token.immediate(/[ \t]+/)),
      field('message', $.zenuml_event_payload),
    )),
  )),

  zenuml_reply_message_statement: ($) => prec.right(55, seq(
    field('source', $.zenuml_endpoint),
    field('arrow', $.zenuml_return_arrow),
    optional(field('target', $.zenuml_endpoint)),
    optional(seq(
      field('delimiter', ':'),
      optional(seq(
        optional(token.immediate(/[ \t]+/)),
        field('message', $.zenuml_event_payload),
      )),
    )),
  )),

  zenuml_return_statement: ($) => prec.right(65, choice(
    seq(
      controlKeyword($, 'return'),
      optional(field('value', $.zenuml_expression)),
    ),
    seq(
      field('annotation', $.zenuml_reply_annotation),
      optional(seq(
        repeat($._line_ending),
        field('message', choice(
          $.zenuml_reply_message_statement,
          $.zenuml_async_message_statement,
        )),
      )),
    ),
  )),

  zenuml_incomplete_message_statement: ($) => prec.right(-30, choice(
    seq(
      field('source', $.zenuml_endpoint),
      field('arrow', choice($.zenuml_arrow, alias('-', $.zenuml_arrow))),
      optional(field('target', $.zenuml_endpoint)),
    ),
    seq(
      field('target', $.zenuml_endpoint),
      field('delimiter', '.'),
    ),
    seq(
      field('target', $.zenuml_endpoint),
      field('delimiter', ':'),
    ),
  )),

  zenuml_divider_statement: ($) => prec.right(40, seq(
    field('operator', alias(token(prec(70, /==+/)), $.zenuml_operator)),
    optional(field('text', $.zenuml_divider_text)),
  )),

  zenuml_divider_text: (_) => token.immediate(/[^\r\n]+/),

  _zenuml_any_block: ($) => choice(
    $.zenuml_block,
    $.zenuml_unclosed_block,
  ),

  zenuml_block: ($) => prec(30, seq(
    field('open', '{'),
    repeat($._zenuml_block_line),
    optional($._zenuml_block_final_item),
    field('close', '}'),
  )),

  zenuml_unclosed_block: ($) => prec(-50, seq(
    field('open', '{'),
    repeat($._zenuml_block_line),
    optional($._zenuml_block_final_item),
  )),

  _zenuml_block_line: ($) => choice(
    prec.right(10, seq(
      field('statement', $._zenuml_statement),
      field('terminator', $._statement_terminator),
    )),
    prec.right(10, seq(field('comment', $.zenuml_comment), $._line_ending)),
    $._blank_line,
  ),

  _zenuml_block_final_item: ($) => choice(
    prec.right(-10, field('statement', $._zenuml_statement)),
    prec.right(-10, field('comment', $.zenuml_comment)),
  ),

  zenuml_condition_clause: ($) => prec(30, seq(
    field('open', '('),
    optional(field('value', $.zenuml_expression)),
    field('close', ')'),
  )),

  zenuml_unclosed_condition_clause: ($) => prec(-40, seq(
    field('open', '('),
    optional(field('value', $.zenuml_expression)),
  )),

  zenuml_assignment: ($) => prec.right(45, choice(
    seq(
      field('type', $.zenuml_name),
      field('assignee', $.zenuml_assignee),
      field('operator', alias('=', $.zenuml_assignment_operator)),
    ),
    seq(
      field('assignee', $.zenuml_assignee),
      field('operator', alias('=', $.zenuml_assignment_operator)),
    ),
  )),

  zenuml_assignee: ($) => seq(
    field('item', $._zenuml_assignment_target),
    repeat(seq(
      field('delimiter', ','),
      field('item', $._zenuml_assignment_target),
    )),
  ),

  _zenuml_assignment_target: ($) => choice(
    $.zenuml_identifier,
    $.zenuml_digit_leading_name,
    $.zenuml_string,
    $.zenuml_unclosed_string,
    $.zenuml_number,
    $.zenuml_number_unit,
    $.zenuml_money,
    $.zenuml_boolean,
    $.zenuml_nil,
    alias(keywordChoice(['new']), $.zenuml_statement_keyword),
  ),

  zenuml_construct: ($) => field('name', $.zenuml_name),

  zenuml_endpoint: ($) => seq(
    optional(field('emoji', $.zenuml_emoji)),
    field('name', $.zenuml_name),
  ),

  zenuml_call_chain: ($) => prec.right(seq(
    field('call', $.zenuml_signature),
    repeat(seq(
      field('delimiter', '.'),
      field('call', $.zenuml_signature),
    )),
  )),

  zenuml_signature: ($) => seq(
    optional(field('emoji', $.zenuml_emoji)),
    field('name', $.zenuml_name),
    optional(field('arguments', choice(
      $.zenuml_argument_list,
      $.zenuml_unclosed_argument_list,
    ))),
  ),

  zenuml_argument_list: ($) => prec.right(30, seq(
    field('open', '('),
    optional(seq(
      field('argument', $.zenuml_argument),
      repeat(seq(
        field('delimiter', ','),
        optional(field('argument', $.zenuml_argument)),
      )),
    )),
    field('close', ')'),
  )),

  zenuml_unclosed_argument_list: ($) => prec.right(-40, seq(
    field('open', '('),
    optional(seq(
      field('argument', $.zenuml_argument),
      repeat(seq(
        field('delimiter', ','),
        optional(field('argument', $.zenuml_argument)),
      )),
    )),
  )),

  zenuml_argument: ($) => choice(
    $.zenuml_named_argument,
    $.zenuml_declaration_argument,
    $.zenuml_expression,
  ),

  zenuml_named_argument: ($) => prec.right(45, seq(
    field('name', choice($.zenuml_identifier, $.zenuml_digit_leading_name)),
    field('operator', alias('=', $.zenuml_assignment_operator)),
    optional(field('value', $.zenuml_expression)),
  )),

  zenuml_declaration_argument: ($) => prec(35, seq(
    field('type', $.zenuml_name),
    field('name', choice($.zenuml_identifier, $.zenuml_digit_leading_name)),
  )),

  zenuml_expression: ($) => prec(10, choice(
    $.zenuml_binary_expression,
    $.zenuml_unary_expression,
    $.zenuml_parenthesized_expression,
    $.zenuml_assignment_expression,
    $.zenuml_creation_expression,
    $.zenuml_call_expression,
    $.zenuml_text_expression,
    $.zenuml_identifier,
    $.zenuml_digit_leading_name,
    $.zenuml_string,
    $.zenuml_unclosed_string,
    $.zenuml_number,
    $.zenuml_number_unit,
    $.zenuml_money,
    $.zenuml_boolean,
    $.zenuml_nil,
  )),

  zenuml_binary_expression: ($) => choice(
    binaryExpression($, 1, '||'),
    binaryExpression($, 2, '&&'),
    binaryExpression($, 3, '==', '!='),
    binaryExpression($, 4, '<=', '>=', '<', '>'),
    binaryExpression($, 5, '+', '-'),
    binaryExpression($, 6, '*', '/', '%'),
    binaryExpression($, 4, 'in'),
  ),

  zenuml_unary_expression: ($) => prec.right(7, seq(
    field('operator', operatorToken($, '-', '!')),
    field('operand', $.zenuml_expression),
  )),

  zenuml_parenthesized_expression: ($) => seq(
    field('open', '('),
    field('value', $.zenuml_expression),
    field('close', ')'),
  ),

  zenuml_assignment_expression: ($) => prec.right(1, seq(
    field('assignment', $.zenuml_assignment),
    field('value', $.zenuml_expression),
  )),

  zenuml_creation_expression: ($) => prec.right(8, seq(
    statementKeyword($, 'new'),
    optional(field('constructor', $.zenuml_construct)),
    optional(field('arguments', choice(
      $.zenuml_argument_list,
      $.zenuml_unclosed_argument_list,
    ))),
  )),

  zenuml_call_expression: ($) => prec.right(8, choice(
    seq(
      field('target', $.zenuml_endpoint),
      field('delimiter', '.'),
      field('message', $.zenuml_call_chain),
    ),
    seq(
      field('name', $.zenuml_name),
      field('arguments', choice(
        $.zenuml_argument_list,
        $.zenuml_unclosed_argument_list,
      )),
    ),
  )),

  zenuml_text_expression: ($) => prec(-30, seq(
    field('word', choice(
      $.zenuml_identifier,
      $.zenuml_digit_leading_name,
      $.zenuml_number_unit,
    )),
    repeat1(field('word', choice(
      $.zenuml_identifier,
      $.zenuml_digit_leading_name,
      $.zenuml_number_unit,
    ))),
  )),

  zenuml_name: ($) => prec(1, choice(
    $.zenuml_identifier,
    $.zenuml_digit_leading_name,
    $.zenuml_string,
    $.zenuml_unclosed_string,
  )),

  zenuml_identifier: (_) => token(prec(
    0,
    /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\u00c0-\uffff]*/,
  )),

  zenuml_digit_leading_name: (_) => token(prec(
    40,
    /[0-9]+[A-Za-z\u00c0-\uffff][A-Za-z0-9_\u00c0-\uffff]*/,
  )),

  zenuml_number: (_) => token(prec(35, choice(
    /[0-9]+\.[0-9]*/,
    /\.[0-9]+/,
    /[0-9]+/,
  ))),

  zenuml_number_unit: (_) => token(prec(
    45,
    /(?:[0-9]+\.[0-9]*|\.[0-9]+|[0-9]+)(?:milliseconds|millisecond|ms|seconds|second|secs|sec|s|minutes|minute|mins|min|hours|hour|hrs|hr|h|days|day|d|weeks|week|w|KiB|MiB|GiB|TiB|KB|MB|GB|TB|kb|mb|gb|tb|B|rem|em|px|mm|cm|km|m|mg|kg|g)/,
  )),

  zenuml_money: (_) => token(prec(45, /\$(?:[0-9]+\.[0-9]*|\.[0-9]+|[0-9]+)/)),

  zenuml_boolean: (_) => keywordChoice(['true', 'false']),

  zenuml_nil: (_) => keywordChoice(['nil', 'null']),

  zenuml_string: (_) => token(prec(50, seq(
    '"',
    repeat(choice('""', /[^"\r\n]/)),
    '"',
  ))),

  zenuml_unclosed_string: (_) => token(prec(-20, seq(
    '"',
    repeat(choice('""', /[^"\r\n]/)),
  ))),

  zenuml_arrow: (_) => token(prec(70, '->')),

  zenuml_return_arrow: (_) => token(prec(75, '-->')),

  zenuml_starter_annotation: (_) => choice('@Starter', '@starter'),

  zenuml_reply_annotation: (_) => choice('@Return', '@return', '@Reply', '@reply'),

  zenuml_participant_annotation: (_) => token(/@[A-Za-z0-9_]*/),

  zenuml_modifier: (_) => keywordChoice(['const', 'readonly', 'static', 'await']),

  zenuml_color: (_) => token(prec(60, /#[0-9a-fA-F]+/)),

  zenuml_event_payload: (_) => token.immediate(/[^\r\n]+/),

  zenuml_comment: (_) => token(prec(75, seq('//', /[^\r\n]*/))),
};

module.exports = { zenumlRules };
