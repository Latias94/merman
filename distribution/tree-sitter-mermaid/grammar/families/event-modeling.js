const eventModelingRules = {
  event_modeling_diagram: ($) => choice(
    seq(
      field('header', $.event_modeling_header),
      optional(field('body', $.event_modeling_body)),
    ),
    field(
      'header',
      alias($._event_modeling_header_eof, $.event_modeling_header),
    ),
  ),

  event_modeling_header: ($) => seq(
    field('keyword', alias('eventmodeling', $.diagram_keyword)),
    field('terminator', $._line_ending),
  ),

  _event_modeling_header_eof: ($) => field(
    'keyword',
    alias('eventmodeling', $.diagram_keyword),
  ),

  event_modeling_body: ($) => repeat1(choice(
    $.event_entity_statement,
    $.event_frame_statement,
    $.event_data_statement,
    $.event_note_statement,
    $.event_gwt_statement,
    $.event_title_statement,
    $.comment,
    $._blank_line,
    $.event_unstructured_body,
  )),

  event_entity_statement: ($) => prec.right(seq(
    field('keyword', 'entity'),
    field('name', $.event_qualified_name),
    optional($._line_ending),
  )),

  event_frame_statement: ($) => prec.right(seq(
    field('keyword', choice('tf', 'timeframe', 'rf', 'resetframe')),
    field('frame', $.event_frame_id),
    field('entity_kind', $.event_entity_kind),
    field('entity', $.event_qualified_name),
    repeat(seq('->>', field('source', $.event_frame_id))),
    optional(seq('[[', field('data', $.identifier), ']]')),
    optional(field('payload', $.event_inline_data)),
    optional($._line_ending),
  )),

  event_data_statement: ($) => prec.right(seq(
    field('keyword', 'data'),
    field('name', $.identifier),
    field('payload', $.event_data_block),
    optional($._line_ending),
  )),

  event_note_statement: ($) => prec.right(seq(
    field('keyword', 'note'),
    field('frame', $.event_frame_id),
    field('payload', $.event_data_block),
    optional($._line_ending),
  )),

  event_gwt_statement: ($) => prec.right(seq(
    field('keyword', 'gwt'),
    field('frame', $.event_frame_id),
    'given',
    repeat1(field('given', $.event_gwt_clause)),
    optional(seq('when', repeat1(field('when', $.event_gwt_clause)))),
    'then',
    repeat1(field('then', $.event_gwt_clause)),
    optional($._line_ending),
  )),

  event_gwt_clause: ($) => seq(
    field('entity_kind', $.event_entity_kind),
    field('entity', $.event_qualified_name),
  ),

  event_title_statement: ($) => prec.right(seq(
    field('keyword', choice('title', 'accTitle', 'accDescr')),
    optional(':'),
    optional(field('text', $.event_line_text)),
    optional($._line_ending),
  )),

  event_qualified_name: ($) => seq(
    $.identifier,
    repeat(seq('.', $.identifier)),
  ),

  event_frame_id: (_) => token(/[0-9]{1,3}/),

  event_entity_kind: (_) => choice(
    'rmo',
    'readmodel',
    'ui',
    'cmd',
    'command',
    'evt',
    'event',
    'pcr',
    'processor',
  ),

  event_inline_data: ($) => seq(
    optional($.event_data_type),
    choice($.event_inline_object, $.quoted_string),
  ),

  event_data_block: ($) => seq(
    optional($.event_data_type),
    '{',
    repeat(choice($.event_nested_block, $.event_block_content)),
    '}',
  ),

  event_nested_block: ($) => seq(
    '{',
    repeat(choice($.event_nested_block, $.event_block_content)),
    '}',
  ),

  event_data_type: ($) => seq(
    '`',
    field('kind', choice('json', 'jsobj', 'figma', 'salt', 'uri', 'md', 'html', 'text')),
    '`',
  ),

  event_inline_object: (_) => token(seq('{', /[^}\r\n]*/, '}')),

  event_block_content: (_) => token(prec(-2, /[^{}]+/)),

  event_line_text: (_) => token(prec(-5, /[^\r\n]+/)),

  event_unstructured_body: ($) => prec.right(seq(
    alias($.unstructured_line, $.unstructured_body),
    optional($._line_ending),
  )),
};

module.exports = { eventModelingRules };
