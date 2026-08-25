// Source translation: Mermaid 11.16.1
// packages/parser/src/language/eventmodeling/event-modeling.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(40, 'eventmodeling')), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(30, keyword)), $.event_statement_keyword),
);

const softLineBreaks = ($) => optional(repeat1($._line_ending));

const trailingTrivia = ($) => optional(choice(
  field('comment', $.comment),
  field('comment', $.event_line_comment),
  field('comment', $.event_multiline_comment),
  field('directive', $.directive),
));

const eventModelingRules = {
  event_modeling_diagram: ($) => choice(
    seq(
      field('header', $.event_modeling_header),
      optional(field('body', $.event_modeling_body)),
    ),
    field('header', alias($._event_modeling_header_eof, $.event_modeling_header)),
  ),

  event_modeling_header: ($) => seq(
    diagramKeyword($),
    trailingTrivia($),
    field('terminator', $._line_ending),
  ),

  _event_modeling_header_eof: ($) => seq(
    diagramKeyword($),
    trailingTrivia($),
  ),

  event_modeling_body: ($) => choice(
    repeat1($._event_modeling_line_item),
    seq(repeat($._event_modeling_line_item), $._event_modeling_eof_item),
  ),

  _event_modeling_line_item: ($) => choice(
    seq(
      field('statement', $._event_modeling_statement),
      trailingTrivia($),
      field('terminator', $._statement_terminator),
    ),
    seq(
      field('trivia', choice(
        $.comment,
        $.directive,
        $.event_line_comment,
        $.event_multiline_comment,
      )),
      $._line_ending,
    ),
    $._blank_line,
  ),

  _event_modeling_eof_item: ($) => choice(
    seq(field('statement', $._event_modeling_statement), trailingTrivia($)),
    $.comment,
    $.directive,
    $.event_line_comment,
    $.event_multiline_comment,
  ),

  _event_modeling_statement: ($) => choice(
    $.event_entity_statement,
    $.event_frame_statement,
    $.event_data_statement,
    $.event_note_statement,
    $.event_gwt_statement,
    $.event_title_statement,
    $.event_accessibility_title_statement,
    $.event_accessibility_description_statement,
    $.event_accessibility_description_block,
    $.event_recovered_statement,
    $.event_malformed_statement,
  ),

  event_entity_statement: ($) => seq(
    statementKeyword($, 'entity'),
    field('name', $.event_qualified_name),
  ),

  event_frame_statement: ($) => prec(40, seq(
    field('kind', choice(
      statementKeyword($, 'tf'),
      statementKeyword($, 'timeframe'),
      statementKeyword($, 'rf'),
      statementKeyword($, 'resetframe'),
    )),
    field('frame', $.event_frame_id),
    field('entity_kind', $.event_entity_kind),
    field('entity', $.event_qualified_name),
    repeat(field('source', $.event_frame_relation)),
    optional(field('data_reference', $.event_data_reference)),
    optional(field('payload', $.event_inline_data)),
  )),

  event_frame_relation: ($) => seq(
    field('operator', alias('->>', $.event_relation_operator)),
    field('frame', $.event_frame_id),
  ),

  event_data_reference: ($) => seq(
    field('open', '[['),
    field('name', alias($._event_identifier, $.event_data_name)),
    field('close', ']]'),
  ),

  event_data_statement: ($) => prec(40, seq(
    statementKeyword($, 'data'),
    field('name', alias($._event_identifier, $.event_data_name)),
    softLineBreaks($),
    field('payload', $.event_data_block),
  )),

  event_note_statement: ($) => prec(40, seq(
    statementKeyword($, 'note'),
    field('frame', $.event_frame_id),
    softLineBreaks($),
    field('payload', $.event_data_block),
  )),

  event_gwt_statement: ($) => prec.right(30, seq(
    statementKeyword($, 'gwt'),
    field('frame', $.event_frame_id),
    softLineBreaks($),
    field('given', $.event_gwt_given_group),
    softLineBreaks($),
    optional(seq(
      field('when', $.event_gwt_when_group),
      softLineBreaks($),
    )),
    field('then', $.event_gwt_then_group),
  )),

  event_gwt_given_group: ($) => seq(
    statementKeyword($, 'given'),
    repeat1(seq(
      softLineBreaks($),
      field('clause', $.event_gwt_clause),
    )),
  ),

  event_gwt_when_group: ($) => seq(
    statementKeyword($, 'when'),
    repeat1(seq(
      softLineBreaks($),
      field('clause', $.event_gwt_clause),
    )),
  ),

  event_gwt_then_group: ($) => seq(
    statementKeyword($, 'then'),
    repeat1(seq(
      softLineBreaks($),
      field('clause', $.event_gwt_clause),
    )),
  ),

  event_gwt_clause: ($) => seq(
    field('entity_kind', $.event_entity_kind),
    field('entity', $.event_qualified_name),
  ),

  event_title_statement: ($) => seq(
    statementKeyword($, 'title'),
    optional(field('delimiter', ':')),
    optional(field('text', $.event_line_text)),
  ),

  event_accessibility_title_statement: ($) => seq(
    statementKeyword($, 'accTitle'),
    field('delimiter', ':'),
    optional(field('text', $.event_line_text)),
  ),

  event_accessibility_description_statement: ($) => seq(
    statementKeyword($, 'accDescr'),
    field('delimiter', ':'),
    optional(field('text', $.event_line_text)),
  ),

  event_accessibility_description_block: ($) => seq(
    statementKeyword($, 'accDescr'),
    field('open', '{'),
    optional(field('text', $.event_accessibility_block_text)),
    field('close', '}'),
  ),

  event_qualified_name: ($) => seq(
    field('part', alias($._event_identifier, $.event_name_part)),
    repeat(seq(
      field('separator', '.'),
      field('part', alias($._event_identifier, $.event_name_part)),
    )),
  ),

  _event_identifier: (_) => token(prec(10, /[_A-Za-z][_A-Za-z0-9]*/)),

  event_frame_id: (_) => token(prec(20, /[0-9]{1,3}/)),

  event_entity_kind: (_) => token(prec(25, choice(
    'rmo',
    'readmodel',
    'ui',
    'cmd',
    'command',
    'evt',
    'event',
    'pcr',
    'processor',
  ))),

  event_inline_data: ($) => seq(
    optional(field('type', $.event_data_type)),
    field('value', choice(
      $.event_inline_object,
      alias($.quoted_string, $.event_inline_string),
    )),
  ),

  event_data_block: ($) => seq(
    optional(field('type', $.event_data_type)),
    field('open', '{'),
    repeat(field('content', choice(
      $.event_nested_data_block,
      $.event_data_fragment,
    ))),
    field('close', '}'),
  ),

  event_nested_data_block: ($) => seq(
    field('open', '{'),
    repeat(field('content', choice(
      $.event_nested_data_block,
      $.event_data_fragment,
    ))),
    field('close', '}'),
  ),

  event_data_type: ($) => seq(
    field('open', '`'),
    field('kind', alias(token.immediate(choice(
      'json',
      'jsobj',
      'figma',
      'salt',
      'uri',
      'md',
      'html',
      'text',
    )), $.event_data_type_name)),
    field('close', token.immediate('`')),
  ),

  event_inline_object: (_) => token(prec(5, /\{[^\r\n]*\}/)),

  event_data_fragment: (_) => token(prec(-10, /[^{}]+/)),

  event_line_text: (_) => token(prec(-10, /[^ \t\f\r\n;][^\r\n;]*/)),

  event_accessibility_block_text: (_) => token(prec(-10, /[^}]+/)),

  event_line_comment: (_) => token(prec(20, /\/\/[^\r\n]*/)),

  event_multiline_comment: (_) => token(prec(20, /\/\*[^*]*(?:\*+[^*/][^*]*)*\*+\//)),

  event_recovered_statement: ($) => prec(-40, seq(
    field('kind', choice(
      statementKeyword($, 'entity'),
      statementKeyword($, 'tf'),
      statementKeyword($, 'timeframe'),
      statementKeyword($, 'rf'),
      statementKeyword($, 'resetframe'),
      statementKeyword($, 'data'),
      statementKeyword($, 'note'),
      statementKeyword($, 'gwt'),
      statementKeyword($, 'title'),
      statementKeyword($, 'accTitle'),
      statementKeyword($, 'accDescr'),
    )),
    optional(field('recovery', $.event_recovery_text)),
  )),

  event_recovery_text: (_) => token(prec(-90, /[^ \t\f\r\n;][^\r\n;]*/)),

  event_malformed_statement: (_) => token(prec(-100, /[^\r\n;]+/)),
};

const eventModelingConflicts = ($) => [
  [$.event_gwt_given_group],
  [$.event_gwt_when_group],
  [$.event_gwt_then_group],
];

module.exports = { eventModelingConflicts, eventModelingRules };
