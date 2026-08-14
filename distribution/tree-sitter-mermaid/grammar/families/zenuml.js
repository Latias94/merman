const zenumlRules = {
  zenuml_diagram: ($) => seq(
    field('header', $.zenuml_header),
    optional(field('body', $.zenuml_body)),
  ),

  zenuml_header: ($) => field('keyword', alias('zenuml', $.diagram_keyword)),

  zenuml_body: ($) => repeat1($._zenuml_item),

  _zenuml_item: ($) => choice(
    $.zenuml_title_statement,
    $.zenuml_starter_statement,
    $.zenuml_group_statement,
    $.zenuml_participant_statement,
    $.zenuml_control_statement,
    $.zenuml_message_statement,
    $.zenuml_return_statement,
    $.zenuml_comment,
    $._blank_line,
    $.zenuml_unstructured_body,
  ),

  zenuml_title_statement: ($) => prec.right(seq(
    field('keyword', 'title'),
    optional(field('text', $.zenuml_title_text)),
    optional($._line_ending),
  )),

  zenuml_starter_statement: ($) => prec.right(seq(
    field('annotation', $.zenuml_starter_annotation),
    optional(seq('(', optional(field('name', $.zenuml_name)), ')')),
    optional($._line_ending),
  )),

  zenuml_group_statement: ($) => prec.right(seq(
    field('keyword', 'group'),
    optional(field('name', $.zenuml_name)),
    optional(field('body', $.zenuml_brace_block)),
    optional($._line_ending),
  )),

  zenuml_participant_statement: ($) => prec(-2, prec.right(choice(
    seq(
      field('annotation', $.zenuml_annotation),
      field('name', $.zenuml_name),
      optional(seq('as', field('label', $.zenuml_name))),
      optional(field('color', $.zenuml_color)),
      optional($._line_ending),
    ),
    seq(
      field('name', $.zenuml_name),
      'as',
      field('label', $.zenuml_name),
      optional(field('color', $.zenuml_color)),
      optional($._line_ending),
    ),
    seq(
      field('name', $.zenuml_name),
      field('color', $.zenuml_color),
      optional($._line_ending),
    ),
  ))),

  zenuml_control_statement: ($) => prec.right(seq(
    field(
      'keyword',
      choice(
        'if',
        'else',
        'while',
        'for',
        'foreach',
        'forEach',
        'loop',
        'par',
        'opt',
        'critical',
        'section',
        'frame',
        'try',
        'catch',
        'finally',
      ),
    ),
    optional(field('condition', $.zenuml_parenthesized_text)),
    optional(field('body', $.zenuml_brace_block)),
    optional($._line_ending),
  )),

  zenuml_message_statement: ($) => prec(3, prec.right(seq(
    optional(field('assignment', $.zenuml_assignment)),
    choice(
      seq(
        field('source', $.zenuml_name),
        field('arrow', $.zenuml_arrow),
        field('target', $.zenuml_name),
        choice(
          seq('.', field('message', $.zenuml_call_chain)),
          seq(':', field('message', $.zenuml_event_payload)),
        ),
      ),
      seq(
        field('target', $.zenuml_name),
        '.',
        field('message', $.zenuml_call_chain),
      ),
      seq(
        field('target', $.zenuml_name),
        ':',
        field('message', $.zenuml_event_payload),
      ),
    ),
    optional(choice(';', $.zenuml_brace_block, $._line_ending)),
  ))),

  zenuml_return_statement: ($) => prec.right(seq(
    choice('return', $.zenuml_reply_annotation),
    optional(field('value', $.zenuml_expression_text)),
    optional(choice(';', $._line_ending)),
  )),

  zenuml_assignment: ($) => seq(
    field('name', $.zenuml_name),
    '=',
  ),

  zenuml_call_chain: ($) => seq(
    $.zenuml_signature,
    repeat(seq('.', $.zenuml_signature)),
  ),

  zenuml_signature: ($) => seq(
    field('name', $.zenuml_name),
    optional(field('arguments', $.zenuml_parenthesized_text)),
  ),

  zenuml_brace_block: ($) => seq(
    '{',
    repeat($._zenuml_item),
    '}',
  ),

  zenuml_parenthesized_text: ($) => seq(
    '(',
    optional($.zenuml_expression_text),
    ')',
  ),

  zenuml_name: ($) => choice($.zenuml_identifier, $.quoted_string, $.number),

  zenuml_identifier: (_) => token(/[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\u00c0-\uffff]*/),

  zenuml_arrow: (_) => choice('-->', '->', '-'),

  zenuml_starter_annotation: (_) => token(choice('@Starter', '@starter')),

  zenuml_reply_annotation: (_) => token(choice('@Return', '@return', '@Reply', '@reply')),

  zenuml_annotation: (_) => token(/@[A-Za-z_][A-Za-z0-9_]*/),

  zenuml_color: (_) => token(/#[0-9a-fA-F]+/),

  zenuml_event_payload: (_) => token(prec(-1, /[^\r\n]+/)),

  zenuml_expression_text: (_) => token(prec(-2, /[^(){};\r\n]+/)),

  zenuml_title_text: (_) => token.immediate(/[ \t]+[^\r\n]*/),

  zenuml_comment: (_) => token(seq('//', /[^\r\n]*/)),

  zenuml_unstructured_body: ($) => prec.right(seq(
    alias($.unstructured_line, $.unstructured_body),
    optional($._line_ending),
  )),
};

module.exports = { zenumlRules };
