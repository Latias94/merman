// Source translation: Mermaid 11.16.1
// packages/parser/src/language/wardley/wardley.langium and the Mermaid bridge
// at commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const { terminatedHeader } = require('../shared/header');

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.statement_keyword),
);

const wardleyRules = {
  wardley_diagram: ($) => choice(
    seq(
      field('header', $.wardley_header),
      optional(field('body', $.wardley_body)),
    ),
    seq(
      field('header', alias($._wardley_inline_header, $.wardley_header)),
      field('body', $.wardley_body),
    ),
    field('header', alias($._wardley_header_eof, $.wardley_header)),
  ),

  wardley_header: ($) => terminatedHeader(
    $,
    token(prec(20, 'wardley-beta')),
  ),

  _wardley_header_eof: ($) => field(
    'keyword',
    alias(token(prec(20, 'wardley-beta')), $.diagram_keyword),
  ),

  _wardley_inline_header: ($) => seq(
    field(
      'keyword',
      alias(token(prec(20, 'wardley-beta')), $.diagram_keyword),
    ),
    token.immediate(/[ \t]+/),
  ),

  wardley_body: ($) => repeat1(choice(
    $.comment,
    $._blank_line,
    $.wardley_title_statement,
    $.wardley_accessibility_title_statement,
    $.wardley_accessibility_description_statement,
    $.wardley_size_statement,
    $.wardley_evolution_statement,
    $.wardley_anchor_statement,
    $.wardley_component_statement,
    $.wardley_link_statement,
    $.wardley_evolve_statement,
    $.wardley_pipeline_statement,
    $.wardley_note_statement,
    $.wardley_annotations_statement,
    $.wardley_annotation_statement,
    $.wardley_accelerator_statement,
    $.wardley_deaccelerator_statement,
    $.wardley_incomplete_component_statement,
    $.wardley_malformed_statement,
  )),

  wardley_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'title'),
    optional(field(
      'text',
      alias($._radar_wardley_title_text, $.wardley_title_text),
    )),
    optional($._line_ending),
  )),

  wardley_accessibility_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'accTitle'),
    field('colon', ':'),
    optional(field(
      'text',
      alias($._radar_wardley_accessibility_text, $.wardley_accessibility_text),
    )),
    optional($._line_ending),
  )),

  wardley_accessibility_description_statement: ($) => prec.right(choice(
    seq(
      statementKeyword($, 'accDescr'),
      field('colon', ':'),
      optional(field(
        'text',
        alias($._radar_wardley_accessibility_text, $.wardley_accessibility_text),
      )),
      optional($._line_ending),
    ),
    seq(
      statementKeyword($, 'accDescr'),
      field(
        'text',
        alias($._radar_wardley_accessibility_block, $.wardley_accessibility_block),
      ),
      optional($._line_ending),
    ),
  )),

  wardley_size_statement: ($) => prec.right(seq(
    statementKeyword($, 'size'),
    '[',
    field('width', $.wardley_integer),
    ',',
    field('height', $.wardley_integer),
    ']',
    optional($._line_ending),
  )),

  wardley_evolution_statement: ($) => prec.right(seq(
    statementKeyword($, 'evolution'),
    field('stage', $.wardley_evolution_stage),
    repeat1(seq(
      field('operator', $.wardley_arrow),
      field('stage', $.wardley_evolution_stage),
    )),
    optional($._line_ending),
  )),

  wardley_evolution_stage: ($) => seq(
    field('name', $.wardley_name),
    optional(seq('@', field('boundary', $.wardley_decimal))),
    optional(seq('/', field('second_name', $.wardley_name))),
  ),

  wardley_anchor_statement: ($) => prec.right(seq(
    statementKeyword($, 'anchor'),
    field('name', $.wardley_name),
    field('position', $.wardley_position),
    optional($._line_ending),
  )),

  wardley_component_statement: ($) => prec.right(10, seq(
    statementKeyword($, 'component'),
    $._wardley_required_gap,
    field('name', $.wardley_name),
    field('position', $.wardley_position),
    optional(field('label', $.wardley_label_clause)),
    optional(field('decorator', $.wardley_strategy_decorator)),
    optional(field('inertia', $.wardley_inertia_clause)),
    optional($._line_ending),
  )),

  wardley_label_clause: ($) => seq(
    statementKeyword($, 'label'),
    '[',
    field('offset_x', $.wardley_signed_integer),
    ',',
    field('offset_y', $.wardley_signed_integer),
    ']',
  ),

  wardley_strategy_decorator: ($) => seq(
    '(',
    field('strategy', $.wardley_strategy),
    ')',
  ),

  wardley_inertia_clause: ($) => choice(
    statementKeyword($, 'inertia'),
    seq('(', statementKeyword($, 'inertia'), ')'),
  ),

  wardley_link_statement: ($) => prec.right(-5, seq(
    choice(
      prec(20, seq(
        field('source', $.wardley_name),
        field('from_port', $.wardley_link_port),
        optional(field('operator', $.wardley_link_operator)),
        field('target', $.wardley_name),
        optional(field('to_port', $.wardley_link_port)),
      )),
      prec(20, seq(
        field('source', $.wardley_name),
        field('operator', $.wardley_link_operator),
        field('target', $.wardley_name),
        optional(field('to_port', $.wardley_link_port)),
      )),
      prec(-20, seq(
        field('source', alias($.wardley_quoted_name, $.wardley_name)),
        field('target', alias($.wardley_quoted_name, $.wardley_name)),
      )),
    ),
    optional(field('label', $.wardley_link_label)),
    optional($._line_ending),
  )),

  wardley_link_label: ($) => seq(
    ';',
    field('text', $.wardley_link_label_text),
  ),

  wardley_link_label_text: ($) => choice(
    field('value', $.wardley_link_label_value),
    seq(
      $._wardley_required_gap,
      field('value', $.wardley_link_label_value),
    ),
  ),

  wardley_evolve_statement: ($) => prec.right(seq(
    statementKeyword($, 'evolve'),
    field('component', $.wardley_name),
    field('target', $.wardley_decimal),
    optional($._line_ending),
  )),

  wardley_pipeline_statement: ($) => prec.right(seq(
    statementKeyword($, 'pipeline'),
    field('parent', $.wardley_name),
    '{',
    $._line_ending,
    field('body', $.wardley_pipeline_body),
    '}',
    optional($._line_ending),
  )),

  wardley_pipeline_body: ($) => repeat1(choice(
    $.comment,
    $._blank_line,
    $.wardley_pipeline_component_statement,
    $.wardley_pipeline_malformed_statement,
  )),

  wardley_pipeline_component_statement: ($) => prec.right(seq(
    statementKeyword($, 'component'),
    field('name', $.wardley_name),
    '[',
    field('evolution', $.wardley_decimal),
    ']',
    optional(field('label', $.wardley_label_clause)),
    optional($._line_ending),
  )),

  wardley_note_statement: ($) => prec.right(seq(
    statementKeyword($, 'note'),
    field('text', $.quoted_string),
    field('position', $.wardley_position),
    optional($._line_ending),
  )),

  wardley_annotations_statement: ($) => prec.right(seq(
    statementKeyword($, 'annotations'),
    field('position', $.wardley_coordinate_pair),
    optional($._line_ending),
  )),

  wardley_annotation_statement: ($) => prec.right(seq(
    statementKeyword($, 'annotation'),
    field('number', $.wardley_integer),
    ',',
    field('position', $.wardley_coordinate_pair),
    field('text', $.quoted_string),
    optional($._line_ending),
  )),

  wardley_accelerator_statement: ($) => prec.right(seq(
    statementKeyword($, 'accelerator'),
    field('name', $.wardley_name),
    field('position', $.wardley_xy_position),
    optional($._line_ending),
  )),

  wardley_deaccelerator_statement: ($) => prec.right(seq(
    statementKeyword($, 'deaccelerator'),
    field('name', $.wardley_name),
    field('position', $.wardley_xy_position),
    optional($._line_ending),
  )),

  wardley_position: ($) => seq(
    '[',
    field('visibility', $.wardley_decimal),
    ',',
    field('evolution', $.wardley_decimal),
    ']',
  ),

  wardley_coordinate_pair: ($) => seq(
    '[',
    field('x', $.wardley_coordinate_value),
    ',',
    field('y', $.wardley_coordinate_value),
    ']',
  ),

  wardley_xy_position: ($) => seq(
    '[',
    field('x', $.wardley_decimal),
    ',',
    field('y', $.wardley_decimal),
    ']',
  ),

  wardley_coordinate_value: ($) => choice(
    $.wardley_decimal,
    $.wardley_integer,
  ),

  wardley_name: ($) => choice(
    $.quoted_string,
    prec.right(repeat1($._wardley_name_part)),
  ),

  _wardley_name_part: ($) => choice(
    $.wardley_name_word,
    $.wardley_name_hyphen,
    $.wardley_parenthesized_name_part,
  ),

  wardley_quoted_name: ($) => seq($.quoted_string),

  wardley_parenthesized_name_part: ($) => seq(
    '(',
    repeat1(choice($.wardley_name_word, $.wardley_name_hyphen)),
    ')',
  ),

  wardley_incomplete_component_statement: ($) => prec.right(-10, seq(
    statementKeyword($, 'component'),
    optional(seq(
      $._wardley_required_gap,
      optional(field('name', $.wardley_name)),
      optional(field('text', $.wardley_incomplete_component_text)),
    )),
    $._line_ending,
  )),

  wardley_malformed_statement: ($) => prec.right(-100, choice(
    seq(
      field(
        'keyword',
        alias($._radar_wardley_recovery_identifier, $.wardley_unknown_keyword),
      ),
      optional(field('text', $.wardley_malformed_tail)),
      optional($._line_ending),
    ),
    seq(
      field('text', $.wardley_malformed_text),
      optional($._line_ending),
    ),
  )),

  wardley_pipeline_malformed_statement: ($) => prec.right(-100, choice(
    seq(
      field(
        'keyword',
        alias($._radar_wardley_recovery_identifier, $.wardley_unknown_keyword),
      ),
      optional(field('text', $.wardley_malformed_tail)),
      optional($._line_ending),
    ),
    seq(
      field('text', $.wardley_pipeline_malformed_text),
      optional($._line_ending),
    ),
  )),

  wardley_arrow: (_) => token(prec(20, '->')),

  wardley_link_port: (_) => token(prec(20, choice('+<>', '+>', '+<'))),

  wardley_link_operator: (_) => token(prec(20, choice(
    /\+'[^'\r\n]*'(?:<>|<|>)/,
    '-.->',
    '-->',
    '->',
    '>',
  ))),

  wardley_strategy: (_) => choice('build', 'buy', 'outsource', 'market'),

  wardley_decimal: (_) => token(/[0-9]+\.[0-9]+/),

  wardley_integer: (_) => token(/0|[1-9][0-9]*/),

  wardley_signed_integer: (_) => token(/-?(?:0|[1-9][0-9]*)/),

  wardley_name_word: (_) => token(prec(-5, /[A-Za-z0-9_\u00c0-\uffff&]+/)),

  wardley_name_hyphen: (_) => token(prec(-20, '-')),

  _wardley_required_gap: (_) => token.immediate(/[ \t]+/),

  wardley_link_label_value: (_) => token.immediate(prec(5, /[^\s\r\n][^\r\n]*/)),

  wardley_incomplete_component_text: (_) => token(prec(-50, /[^\r\n]+/)),

  wardley_malformed_tail: (_) => token(prec(-10, /[^\r\n]+/)),

  wardley_malformed_text: (_) => token(prec(-100,
    /[^A-Za-z0-9_\u00c0-\uffff\r\n][^\r\n]*/,
  )),

  wardley_pipeline_malformed_text: (_) => token(prec(-100,
    /[^A-Za-z0-9_\u00c0-\uffff}\r\n][^}\r\n]*/,
  )),
};

module.exports = { wardleyRules };
