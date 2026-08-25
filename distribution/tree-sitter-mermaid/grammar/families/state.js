// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/state/parser/stateDiagram.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(
    token(prec(40, /stateDiagram(?:-v2)?/i)),
    $.diagram_keyword,
  ),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(30, keyword)), $.state_statement_keyword),
);

const trailingTrivia = ($) => optional(choice(
  field('comment', $.comment),
  field('comment', $.state_hash_comment),
  field('directive', $.directive),
));

const optionalInlineGap = () => optional(token.immediate(/[ \t]+/));

const stateReference = ($, fieldName) => field(fieldName, $.state_endpoint);

const stateRules = {
  state_diagram: ($) => choice(
    seq(
      field('header', $.state_header),
      optional(field('body', $.state_body)),
    ),
    field('header', alias($._state_header_eof, $.state_header)),
  ),

  state_header: ($) => seq(
    diagramKeyword($),
    trailingTrivia($),
    field('terminator', $._line_ending),
  ),

  _state_header_eof: ($) => diagramKeyword($),

  state_body: ($) => choice(
    repeat1($._state_line_item),
    seq(repeat($._state_line_item), $._state_eof_item),
  ),

  _state_line_item: ($) => choice(
    seq(
      field('statement', $._state_statement),
      trailingTrivia($),
      field('terminator', $._statement_terminator),
    ),
    seq(choice($.comment, $.state_hash_comment, $.directive), $._line_ending),
    $._blank_line,
  ),

  _state_eof_item: ($) => choice(
    seq(field('statement', $._state_statement), trailingTrivia($)),
    $.comment,
    $.state_hash_comment,
    $.directive,
  ),

  _state_statement: ($) => choice(
    $.state_composite_declaration,
    $.state_pseudostate_declaration,
    $.state_alias_declaration,
    $.state_named_declaration,
    $.state_jison_split_transition_statement,
    $.state_jison_split_reference_statement,
    $.state_transition_statement,
    $.state_incomplete_transition_statement,
    $.state_description_statement,
    $.state_reference_statement,
    $.state_concurrent_divider,
    $.state_note_statement,
    $.state_floating_note_statement,
    $.state_direction_statement,
    $.state_hide_empty_description_statement,
    $.state_scale_statement,
    $.state_class_definition_statement,
    $.state_class_assignment_statement,
    $.state_style_statement,
    $.state_click_statement,
    $.state_accessibility_title_statement,
    $.state_accessibility_description_statement,
  ),

  state_composite_declaration: ($) => seq(
    statementKeyword($, 'state'),
    field('declaration', choice(
      $.state_alias_clause,
      field('name', alias($._state_identifier, $.state_name)),
    )),
    optional(field('class', $.state_class_annotation)),
    field('open', choice(
      '{',
      seq($._line_ending, '{'),
    )),
    repeat($._state_line_item),
    field('close', '}'),
  ),

  state_pseudostate_declaration: ($) => seq(
    statementKeyword($, 'state'),
    field('name', alias($._state_identifier, $.state_name)),
    field('kind', $.state_pseudostate_kind),
  ),

  state_pseudostate_kind: (_) => token(prec(30, choice(
    '<<fork>>',
    '<<join>>',
    '<<choice>>',
    '[[fork]]',
    '[[join]]',
    '[[choice]]',
  ))),

  state_alias_declaration: ($) => seq(
    statementKeyword($, 'state'),
    field('alias', $.state_alias_clause),
    optional(field('class', $.state_class_annotation)),
    optional(seq(
      field('delimiter', ':'),
      optional(field('description', $.state_description_text)),
    )),
  ),

  state_alias_clause: ($) => seq(
    field('description', $.state_quoted_text),
    statementKeyword($, 'as'),
    field('name', alias($._state_identifier, $.state_name)),
  ),

  state_named_declaration: ($) => seq(
    statementKeyword($, 'state'),
    field('name', alias($._state_identifier, $.state_name)),
    optional(field('class', $.state_class_annotation)),
  ),

  // Mermaid's Jison grammar treats newlines as statements rather than mandatory
  // separators. These two authored forms therefore become adjacent statements
  // in the semantic parser even though they resemble unsupported operators.
  state_jison_split_transition_statement: ($) => prec(60, seq(
    stateReference($, 'source'),
    optionalInlineGap(),
    field('operator', $.state_transition_operator),
    optionalInlineGap(),
    field('compatibility_target', $.state_jison_pipe_target),
    token.immediate(/[ \t]+/),
    stateReference($, 'trailing_state'),
  )),

  state_jison_pipe_target: (_) => token.immediate(prec(40, /\|[^|;%\r\n]*\|/)),

  state_jison_split_reference_statement: ($) => prec(60, seq(
    stateReference($, 'source'),
    token.immediate(/[ \t]+/),
    field('compatibility_operator', alias(
      token.immediate('..'),
      $.state_jison_split_operator,
    )),
    token.immediate(/[ \t]+/),
    stateReference($, 'target'),
  )),

  state_transition_statement: ($) => prec(50, seq(
    stateReference($, 'source'),
    optionalInlineGap(),
    field('operator', $.state_transition_operator),
    optionalInlineGap(),
    stateReference($, 'target'),
    optional(seq(
      field('delimiter', ':'),
      optionalInlineGap(),
      optional(field('label', $.state_description_text)),
    )),
  )),

  state_incomplete_transition_statement: ($) => prec(-30, seq(
    stateReference($, 'source'),
    optionalInlineGap(),
    field('operator', $.state_transition_operator),
    optionalInlineGap(),
    optional(field('recovery', $.state_transition_recovery)),
  )),

  state_transition_recovery: (_) => token(prec(-100, /[^;%\r\n]+/)),

  state_transition_operator: (_) => token.immediate(prec(30, '-->')),

  state_endpoint: ($) => prec.right(seq(
    choice(
      $.state_marker,
      alias($._state_identifier, $.state_reference),
    ),
    optional(field('class', $.state_class_annotation)),
  )),

  state_description_statement: ($) => seq(
    stateReference($, 'state'),
    optionalInlineGap(),
    field('delimiter', ':'),
    optionalInlineGap(),
    optional(field('description', $.state_description_text)),
  ),

  state_description_text: ($) => repeat1(choice(
    $.state_description_fragment,
    $.state_encoded_colon,
    $.state_description_hash,
  )),

  state_description_fragment: (_) => token(prec(-10, /[^;%#\r\n]+/)),
  state_encoded_colon: (_) => token(prec(30, '#colon;')),
  state_description_hash: (_) => token.immediate('#'),

  state_reference_statement: ($) => seq(
    stateReference($, 'state'),
    optional(field('class', $.state_class_annotation)),
  ),

  state_class_annotation: ($) => seq(
    field('operator', ':::'),
    field('class', alias($._state_identifier, $.state_class_name)),
  ),

  state_marker: (_) => token(prec(40, '[*]')),

  state_concurrent_divider: (_) => token(prec(30, '--')),

  state_note_statement: ($) => choice(
    $.state_inline_note,
    $.state_multiline_note,
  ),

  state_inline_note: ($) => seq(
    statementKeyword($, 'note'),
    token.immediate(/[ \t]+/),
    field('position', $.state_note_position),
    token.immediate(/[ \t]+/),
    stateReference($, 'target'),
    field('delimiter', ':'),
    optionalInlineGap(),
    optional(field('text', $.state_note_text)),
  ),

  state_multiline_note: ($) => seq(
    statementKeyword($, 'note'),
    token.immediate(/[ \t]+/),
    field('position', $.state_note_position),
    token.immediate(/[ \t]+/),
    stateReference($, 'target'),
    field('terminator', $._line_ending),
    repeat(field('text', $.state_note_line)),
    field('end', $.state_note_end),
  ),

  state_note_position: (_) => token(prec(30, choice('left of', 'right of'))),
  state_note_text: (_) => token(prec(-10, /[^;%\r\n]+/)),
  state_note_line: ($) => seq(
    optional(token(prec(-50, /[^\r\n]+/))),
    $._line_ending,
  ),
  state_note_end: (_) => token(prec(40, 'end note')),

  state_floating_note_statement: ($) => seq(
    statementKeyword($, 'note'),
    field('text', $.state_quoted_text),
    statementKeyword($, 'as'),
    field('name', alias($._state_identifier, $.state_name)),
  ),

  state_direction_statement: ($) => seq(
    statementKeyword($, 'direction'),
    token.immediate(/[ \t]+/),
    field('direction', alias(
      token.immediate(/(?:TB|BT|RL|LR)/),
      $.state_direction,
    )),
  ),

  state_hide_empty_description_statement: ($) => statementKeyword(
    $,
    'hide empty description',
  ),

  state_scale_statement: ($) => seq(
    statementKeyword($, 'scale'),
    field('width', $.state_scale_width),
    optional(statementKeyword($, 'width')),
  ),

  state_scale_width: (_) => token(/[0-9]+/),

  state_class_definition_statement: ($) => seq(
    statementKeyword($, 'classDef'),
    field('class', choice(
      alias(token(prec(30, 'default')), $.state_class_name),
      alias($._state_identifier, $.state_class_name),
    )),
    field('style', $.state_style_list),
  ),

  state_class_assignment_statement: ($) => seq(
    statementKeyword($, 'class'),
    field('states', $.state_identifier_list),
    field('class', alias($._state_identifier, $.state_class_name)),
  ),

  state_style_statement: ($) => seq(
    statementKeyword($, 'style'),
    field('states', $.state_identifier_list),
    field('style', $.state_style_list),
  ),

  state_identifier_list: ($) => seq(
    field('item', alias($._state_identifier, $.state_reference)),
    repeat(seq(
      field('delimiter', ','),
      field('item', alias($._state_identifier, $.state_reference)),
    )),
  ),

  state_style_list: ($) => seq(
    field('declaration', $.state_style_declaration),
    repeat(seq(
      optional(field('delimiter', ',')),
      field('declaration', $.state_style_declaration),
    )),
    optional(field('delimiter', ',')),
  ),

  state_style_declaration: ($) => seq(
    field('property', $.state_style_property),
    field('delimiter', ':'),
    field('value', $.state_style_value),
  ),

  state_style_property: (_) => token(prec(10, /[A-Za-z_-][A-Za-z0-9_-]*/)),
  state_style_value: (_) => token(prec(-20, /[^,;%\r\n]+/)),

  state_click_statement: ($) => seq(
    statementKeyword($, 'click'),
    stateReference($, 'target'),
    optional(statementKeyword($, 'href')),
    field('url', $.state_quoted_text),
    optional(field('tooltip', $.state_quoted_text)),
  ),

  state_accessibility_title_statement: ($) => seq(
    statementKeyword($, 'accTitle'),
    field('delimiter', ':'),
    optional(field('text', $.state_accessibility_text)),
  ),

  state_accessibility_description_statement: ($) => seq(
    statementKeyword($, 'accDescr'),
    choice(
      seq(
        field('delimiter', ':'),
        optional(field('text', $.state_accessibility_text)),
      ),
      field('description', $.state_accessibility_description_block),
    ),
  ),

  state_accessibility_text: (_) => token(prec(-10, /[^;%\r\n]+/)),

  state_accessibility_description_block: ($) => seq(
    field('open', '{'),
    optional(field('text', $.state_accessibility_block_text)),
    field('close', token.immediate('}')),
  ),

  state_accessibility_block_text: (_) => token.immediate(/[^}]+/),

  state_quoted_text: (_) => token(prec(30, seq(
    '"',
    /(?:[^"\\]|\\.)*/,
    '"',
  ))),

  state_hash_comment: (_) => token(seq('#', /[^\r\n]*/)),

  _state_identifier: (_) => token(prec(
    -5,
    /[^\s:{}\-\[\]"';,%]+/,
  )),
};

const stateConflicts = ($) => [
  [$.state_composite_declaration, $.state_alias_declaration],
  [$.state_composite_declaration, $.state_named_declaration],
];

module.exports = { stateConflicts, stateRules };
