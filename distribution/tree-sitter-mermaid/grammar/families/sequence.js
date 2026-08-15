// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/sequence/parser/sequenceDiagram.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(50, /sequenceDiagram/i)), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(40, keyword)), $.sequence_statement_keyword),
);

const blockKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(40, keyword)), $.sequence_block_keyword),
);

const actorReference = ($, fieldName) => field(
  fieldName,
  $.sequence_endpoint,
);

const trailingTrivia = ($) => optional(choice(
  field('comment', $.comment),
  field('comment', $.sequence_hash_comment),
  field('directive', $.directive),
));

const sequenceRules = {
  sequence_diagram: ($) => choice(
    seq(
      field('header', $.sequence_header),
      optional(field('body', $.sequence_body)),
    ),
    field('header', alias($._sequence_header_eof, $.sequence_header)),
  ),

  sequence_header: ($) => seq(
    diagramKeyword($),
    trailingTrivia($),
    field('terminator', $._statement_terminator),
  ),

  _sequence_header_eof: ($) => seq(
    diagramKeyword($),
    trailingTrivia($),
  ),

  sequence_body: ($) => choice(
    repeat1($._sequence_line_item),
    seq(repeat($._sequence_line_item), $._sequence_eof_item),
  ),

  _sequence_line_item: ($) => choice(
    seq(
      field('statement', $._sequence_statement),
      trailingTrivia($),
      field('terminator', $._statement_terminator),
    ),
    seq(
      field('trivia', choice($.comment, $.directive, $.sequence_hash_comment)),
      $._line_ending,
    ),
    $._blank_line,
  ),

  _sequence_eof_item: ($) => choice(
    seq(field('statement', $._sequence_statement), trailingTrivia($)),
    $.comment,
    $.directive,
    $.sequence_hash_comment,
  ),

  _sequence_statement: ($) => choice(
    $.sequence_participant_declaration,
    $.sequence_create_participant_statement,
    $.sequence_destroy_participant_statement,
    $.sequence_message_statement,
    $.sequence_incomplete_message_statement,
    $.sequence_activation_statement,
    $.sequence_note_statement,
    $.sequence_autonumber_statement,
    $.sequence_loop_block,
    $.sequence_opt_block,
    $.sequence_alt_block,
    $.sequence_par_block,
    $.sequence_critical_block,
    $.sequence_break_block,
    $.sequence_rect_block,
    $.sequence_box_block,
    $.sequence_title_statement,
    $.sequence_accessibility_title_statement,
    $.sequence_accessibility_description_statement,
    $.sequence_accessibility_description_block,
    $.sequence_actor_metadata_statement,
    $.sequence_recovered_statement,
    $.sequence_malformed_statement,
  ),

  sequence_participant_declaration: ($) => prec(40, seq(
    field('kind', choice(
      statementKeyword($, /participant/i),
      statementKeyword($, /actor/i),
    )),
    field('name', alias($._sequence_actor_identifier, $.sequence_participant_name)),
    optional(field('config', $.sequence_participant_config)),
    optional(seq(
      statementKeyword($, /as/i),
      field('label', $.sequence_line_text),
    )),
  )),

  sequence_create_participant_statement: ($) => seq(
    statementKeyword($, /create/i),
    field('participant', $.sequence_participant_declaration),
  ),

  sequence_destroy_participant_statement: ($) => seq(
    statementKeyword($, /destroy/i),
    actorReference($, 'participant'),
  ),

  sequence_message_statement: ($) => prec(50, seq(
    actorReference($, 'source'),
    optional(field('source_connection', $.sequence_central_connection)),
    field('operator', $.sequence_message_operator),
    optional(field('target_connection', $.sequence_central_connection)),
    optional(field('activation', $.sequence_inline_activation)),
    actorReference($, 'target'),
    field('delimiter', ':'),
    optional(field('message', $.sequence_message_text)),
  )),

  sequence_incomplete_message_statement: ($) => prec(-30, seq(
    actorReference($, 'source'),
    optional(field('source_connection', $.sequence_central_connection)),
    field('operator', $.sequence_message_operator),
    optional(field('target_connection', $.sequence_central_connection)),
    optional(field('activation', $.sequence_inline_activation)),
    optional(actorReference($, 'target')),
    optional(field('recovery', $.sequence_recovery_text)),
  )),

  sequence_endpoint: ($) => alias(
    $._sequence_actor_identifier,
    $.sequence_actor_reference,
  ),

  _sequence_actor_identifier: (_) => token(prec(
    30,
    /[A-Za-z0-9_\u00c0-\uffff](?:[A-Za-z0-9_=.\u00c0-\uffff]|-[A-Za-z0-9_=.\u00c0-\uffff])*/,
  )),

  sequence_participant_config: (_) => token(prec(30, /@\{[^}\r\n]*\}/)),

  sequence_message_operator: (_) => token(prec(35, choice(
    '<<-->>',
    '<<->>',
    '-->>',
    '->>',
    '-->',
    '->',
    '--x',
    '-x',
    '--)',
    '-)',
    '--|\\',
    '--|/',
    '--\\\\',
    '--//',
    '/|--',
    '\\|--',
    '//--',
    '\\\\--',
    '-|\\',
    '-|/',
    '-\\\\',
    '-//',
    '/|-',
    '\\|-',
    '//-',
    '\\\\-',
  ))),

  sequence_central_connection: (_) => token(prec(40, '()')),

  sequence_inline_activation: (_) => token(prec(30, choice('+', '-'))),

  sequence_message_text: (_) => token(prec(-5, /[^#; \t\f\r\n][^#;\r\n]*/)),

  sequence_activation_statement: ($) => seq(
    field('action', choice(
      statementKeyword($, /activate/i),
      statementKeyword($, /deactivate/i),
    )),
    actorReference($, 'participant'),
  ),

  sequence_note_statement: ($) => seq(
    statementKeyword($, /note/i),
    field('placement', $.sequence_note_placement),
    actorReference($, 'participant'),
    optional(seq(
      field('separator', ','),
      actorReference($, 'participant'),
    )),
    field('delimiter', ':'),
    optional(field('text', $.sequence_note_text)),
  ),

  sequence_note_placement: (_) => token(prec(35, choice(
    /left[ \t]+of/i,
    /right[ \t]+of/i,
    /over/i,
  ))),

  sequence_note_text: (_) => token(prec(-5, /[^#; \t\f\r\n][^#;\r\n]*/)),

  sequence_autonumber_statement: ($) => seq(
    statementKeyword($, /autonumber/i),
    optional(choice(
      field('action', alias(token(/off/i), $.sequence_autonumber_action)),
      seq(
        field('start', $.sequence_number),
        optional(field('step', $.sequence_number)),
      ),
    )),
  ),

  sequence_number: (_) => token(prec(20, /(?:[0-9]+(?:\.[0-9]{1,2})?|\.[0-9]{1,2})/)),

  sequence_loop_block: ($) => seq(
    blockKeyword($, /loop/i),
    optional(field('label', $.sequence_block_label)),
    field('open_terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
    field('end', alias(token(prec(40, /end/i)), $.sequence_block_end)),
  ),

  sequence_opt_block: ($) => seq(
    blockKeyword($, /opt/i),
    optional(field('label', $.sequence_block_label)),
    field('open_terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
    field('end', alias(token(prec(40, /end/i)), $.sequence_block_end)),
  ),

  sequence_alt_block: ($) => seq(
    blockKeyword($, /alt/i),
    optional(field('label', $.sequence_block_label)),
    field('open_terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
    repeat(field('branch', $.sequence_else_branch)),
    field('end', alias(token(prec(40, /end/i)), $.sequence_block_end)),
  ),

  sequence_else_branch: ($) => seq(
    blockKeyword($, /else/i),
    optional(field('label', $.sequence_block_label)),
    field('terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
  ),

  sequence_par_block: ($) => seq(
    field('kind', choice(
      blockKeyword($, /par_over/i),
      blockKeyword($, /par/i),
    )),
    optional(field('label', $.sequence_block_label)),
    field('open_terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
    repeat(field('branch', $.sequence_and_branch)),
    field('end', alias(token(prec(40, /end/i)), $.sequence_block_end)),
  ),

  sequence_and_branch: ($) => seq(
    blockKeyword($, /and/i),
    optional(field('label', $.sequence_block_label)),
    field('terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
  ),

  sequence_critical_block: ($) => seq(
    blockKeyword($, /critical/i),
    optional(field('label', $.sequence_block_label)),
    field('open_terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
    repeat(field('branch', $.sequence_option_branch)),
    field('end', alias(token(prec(40, /end/i)), $.sequence_block_end)),
  ),

  sequence_option_branch: ($) => seq(
    blockKeyword($, /option/i),
    optional(field('label', $.sequence_block_label)),
    field('terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
  ),

  sequence_break_block: ($) => seq(
    blockKeyword($, /break/i),
    optional(field('label', $.sequence_block_label)),
    field('open_terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
    field('end', alias(token(prec(40, /end/i)), $.sequence_block_end)),
  ),

  sequence_rect_block: ($) => seq(
    blockKeyword($, /rect/i),
    optional(field('label', $.sequence_block_label)),
    field('open_terminator', $._statement_terminator),
    repeat(field('statement', $._sequence_line_item)),
    field('end', alias(token(prec(40, /end/i)), $.sequence_block_end)),
  ),

  sequence_box_block: ($) => seq(
    blockKeyword($, /box/i),
    optional(field('label', $.sequence_block_label)),
    field('open_terminator', $._statement_terminator),
    repeat(field('participant', seq(
      field('declaration', $.sequence_participant_declaration),
      field('terminator', $._statement_terminator),
    ))),
    field('end', alias(token(prec(40, /end/i)), $.sequence_block_end)),
  ),

  sequence_block_label: (_) => token(prec(-5, /[^#; \t\f\r\n][^#;\r\n]*/)),

  sequence_title_statement: ($) => seq(
    statementKeyword($, /title:?/i),
    optional(field('text', $.sequence_line_text)),
  ),

  sequence_accessibility_title_statement: ($) => seq(
    statementKeyword($, /accTitle/i),
    field('delimiter', ':'),
    optional(field('text', $.sequence_line_text)),
  ),

  sequence_accessibility_description_statement: ($) => seq(
    statementKeyword($, /accDescr/i),
    field('delimiter', ':'),
    optional(field('text', $.sequence_line_text)),
  ),

  sequence_accessibility_description_block: ($) => seq(
    statementKeyword($, /accDescr/i),
    field('open', '{'),
    optional(field('text', $.sequence_accessibility_block_text)),
    field('close', '}'),
  ),

  sequence_actor_metadata_statement: ($) => seq(
    field('kind', choice(
      statementKeyword($, /links/i),
      statementKeyword($, /link/i),
      statementKeyword($, /properties/i),
      statementKeyword($, /details/i),
    )),
    actorReference($, 'participant'),
    field('delimiter', ':'),
    optional(field('value', $.sequence_line_text)),
  ),

  sequence_line_text: (_) => token(prec(-5, /[^#; \t\f\r\n][^#;\r\n]*/)),

  sequence_accessibility_block_text: (_) => token(prec(-5, /[^}]+/)),

  sequence_hash_comment: (_) => token(prec(25, /#[^\r\n]*/)),

  sequence_recovered_statement: ($) => prec(-50, seq(
    field('keyword', alias(token(choice(
      /participant/i,
      /actor/i,
      /create/i,
      /destroy/i,
      /activate/i,
      /deactivate/i,
      /note/i,
      /autonumber/i,
      /title:?/i,
      /accTitle/i,
      /accDescr/i,
    )), $.sequence_recovery_keyword)),
    optional(field('recovery', $.sequence_recovery_text)),
  )),

  sequence_recovery_text: (_) => token(prec(-90, /[^ \t\f\r\n;][^\r\n;]*/)),

  sequence_malformed_statement: ($) => prec(-100, choice(
    seq(
      field('head', alias(
        $._sequence_actor_identifier,
        $.sequence_unknown_statement_head,
      )),
      optional(field('recovery', $.sequence_recovery_text)),
    ),
    field('recovery', $.sequence_invalid_line),
  )),

  sequence_invalid_line: (_) => token(prec(
    -100,
    /[^A-Za-z0-9_\u00c0-\uffff \t\f\r\n;][^\r\n;]*/,
  )),
};

module.exports = { sequenceRules };
