// Source translation: Mermaid 11.16.1
// packages/parser/src/language/wardley/wardley.langium and the Mermaid bridge
// at commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.statement_keyword),
);

// A spaced target consumes its gap explicitly. The adjacent branch must start
// immediately so Tree-sitter extras never become part of the captured name.
const optionallySpacedNameField = ($, name) => choice(
  seq(
    $._wardley_required_gap,
    field(name, $.wardley_name),
  ),
  field(name, alias($._wardley_immediate_name, $.wardley_name)),
);

const wardleyRules = {
  wardley_diagram: ($) => seq(
    field('header', $.wardley_header),
    optional(seq(
      $._langium_body_boundary,
      optional(field('body', $.wardley_body)),
    )),
  ),

  wardley_header: ($) => field(
    'keyword',
    alias(token(prec(20, 'wardley-beta')), $.diagram_keyword),
  ),

  wardley_body: ($) => choice(
    repeat1($._wardley_terminated_body_item),
    seq(
      repeat($._wardley_terminated_body_item),
      $._wardley_eof_body_item,
    ),
  ),

  _wardley_statement: ($) => choice(
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
  ),

  _wardley_terminated_body_item: ($) => choice(
    $._line_ending,
    seq(choice($.comment, $.directive), $._line_ending),
    seq(
      $._wardley_statement,
      optional(choice($.comment, $.directive)),
      $._line_ending,
    ),
  ),

  _wardley_eof_body_item: ($) => choice(
    $.comment,
    $.directive,
    seq($._wardley_statement, optional(choice($.comment, $.directive))),
  ),

  wardley_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'title'),
    optional(seq(
      $._langium_inline_space,
      optional(field(
        'text',
        alias($._radar_wardley_title_text, $.wardley_title_text),
      )),
    )),
  )),

  wardley_accessibility_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'accTitle'),
    field('colon', ':'),
    optional(field(
      'text',
      alias($._radar_wardley_accessibility_text, $.wardley_accessibility_text),
    )),
  )),

  wardley_accessibility_description_statement: ($) => prec.right(choice(
    seq(
      statementKeyword($, 'accDescr'),
      field('colon', ':'),
      optional(field(
        'text',
        alias($._radar_wardley_accessibility_text, $.wardley_accessibility_text),
      )),
    ),
    seq(
      statementKeyword($, 'accDescr'),
      repeat($._line_ending),
      field(
        'text',
        alias($._radar_wardley_accessibility_block, $.wardley_accessibility_block),
      ),
    ),
  )),

  wardley_size_statement: ($) => prec.right(seq(
    statementKeyword($, 'size'),
    '[',
    field('width', $.wardley_integer),
    ',',
    field('height', $.wardley_integer),
    ']',
  )),

  wardley_evolution_statement: ($) => prec.right(seq(
    statementKeyword($, 'evolution'),
    $._wardley_required_gap,
    field('stage', $.wardley_evolution_stage),
    repeat1(seq(
      field('operator', $.wardley_arrow),
      field('stage', $.wardley_evolution_stage),
    )),
  )),

  wardley_evolution_stage: ($) => seq(
    field('name', $.wardley_name),
    optional(seq('@', field('boundary', $.wardley_decimal))),
    optional(seq('/', field('second_name', $.wardley_name))),
  ),

  wardley_anchor_statement: ($) => prec.right(seq(
    statementKeyword($, 'anchor'),
    $._wardley_required_gap,
    field('name', $.wardley_name),
    field('position', $.wardley_position),
  )),

  wardley_component_statement: ($) => prec.right(10, seq(
    statementKeyword($, 'component'),
    $._wardley_required_gap,
    field('name', $.wardley_name),
    field('position', $.wardley_position),
    optional(field('label', $.wardley_label_clause)),
    optional(field('decorator', $.wardley_strategy_decorator)),
    optional(field('inertia', $.wardley_inertia_clause)),
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
      seq(
        field(
          'source',
          alias($._wardley_quoted_name, $.wardley_name),
        ),
        choice(
          $._wardley_link_port_tail,
          $._wardley_link_operator_tail,
          $._wardley_link_direct_target_tail,
        ),
      ),
      seq(
        field(
          'source',
          alias($._wardley_bare_name, $.wardley_name),
        ),
        choice(
          $._wardley_link_port_tail,
          $._wardley_link_operator_tail,
          $._wardley_link_quoted_target_tail,
        ),
      ),
    ),
    optional(seq(
      optional($._wardley_required_gap),
      field('label', $.wardley_link_label),
    )),
  )),

  _wardley_link_port_tail: ($) => prec.right(20, seq(
    optional($._wardley_required_gap),
    field('from_port', $.wardley_link_port),
    optional(seq(
      optional($._wardley_required_gap),
      field('operator', $.wardley_link_operator),
    )),
    optionallySpacedNameField($, 'target'),
    optional(seq(
      optional($._wardley_required_gap),
      field('to_port', $.wardley_link_port),
    )),
  )),

  _wardley_link_operator_tail: ($) => prec.right(20, seq(
    optional($._wardley_required_gap),
    field('operator', $.wardley_link_operator),
    optionallySpacedNameField($, 'target'),
    optional(seq(
      optional($._wardley_required_gap),
      field('to_port', $.wardley_link_port),
    )),
  )),

  _wardley_link_direct_target_tail: ($) => seq(
    $._wardley_required_gap,
    choice(
      field(
        'target',
        alias($._wardley_immediate_bare_name, $.wardley_name),
      ),
      field('target', alias($._wardley_immediate_string, $.quoted_string)),
    ),
  ),

  _wardley_link_quoted_target_tail: ($) => seq(
    $._wardley_required_gap,
    field('target', alias($._wardley_immediate_string, $.quoted_string)),
  ),

  wardley_link_label: ($) => seq(
    token.immediate(';'),
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
    $._wardley_required_gap,
    field('component', $.wardley_name),
    field('target', $.wardley_decimal),
  )),

  wardley_pipeline_statement: ($) => prec.right(seq(
    statementKeyword($, 'pipeline'),
    $._wardley_required_gap,
    field('parent', $.wardley_name),
    '{',
    $._line_ending,
    field('body', $.wardley_pipeline_body),
    '}',
  )),

  wardley_pipeline_body: ($) => seq(
    repeat($._wardley_pipeline_non_component_item),
    field('component', $.wardley_pipeline_component_statement),
    optional(choice($.comment, $.directive)),
    $._line_ending,
    repeat(choice(
      $._wardley_pipeline_non_component_item,
      seq(
        field('component', $.wardley_pipeline_component_statement),
        optional(choice($.comment, $.directive)),
        $._line_ending,
      ),
    )),
  ),

  _wardley_pipeline_non_component_item: ($) => choice(
    $._line_ending,
    seq(choice($.comment, $.directive), $._line_ending),
    seq($.wardley_pipeline_malformed_statement, $._line_ending),
  ),

  wardley_pipeline_component_statement: ($) => prec.right(seq(
    statementKeyword($, 'component'),
    $._wardley_required_gap,
    field('name', $.wardley_name),
    '[',
    field('evolution', $.wardley_decimal),
    ']',
    optional(field('label', $.wardley_label_clause)),
  )),

  wardley_note_statement: ($) => prec.right(seq(
    statementKeyword($, 'note'),
    $._wardley_required_gap,
    field('text', $._wardley_string),
    field('position', $.wardley_position),
  )),

  wardley_annotations_statement: ($) => prec.right(seq(
    statementKeyword($, 'annotations'),
    field('position', $.wardley_coordinate_pair),
  )),

  wardley_annotation_statement: ($) => prec.right(seq(
    statementKeyword($, 'annotation'),
    $._wardley_required_gap,
    field('number', $.wardley_integer),
    ',',
    field('position', $.wardley_coordinate_pair),
    field('text', $._wardley_string),
  )),

  wardley_accelerator_statement: ($) => prec.right(seq(
    statementKeyword($, 'accelerator'),
    $._wardley_required_gap,
    field('name', $.wardley_name),
    field('position', $.wardley_xy_position),
  )),

  wardley_deaccelerator_statement: ($) => prec.right(seq(
    statementKeyword($, 'deaccelerator'),
    $._wardley_required_gap,
    field('name', $.wardley_name),
    field('position', $.wardley_xy_position),
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
    $._wardley_string,
    $._wardley_bare_name,
  ),

  _wardley_bare_name: ($) => prec.right(repeat1($._wardley_name_part)),

  _wardley_quoted_name: ($) => seq($._wardley_string),

  _wardley_immediate_name: ($) => choice(
    alias($._wardley_immediate_string, $.quoted_string),
    $._wardley_immediate_bare_name,
  ),

  _wardley_immediate_bare_name: ($) => prec.right(seq(
      alias(
        token.immediate(prec(
          100,
          /[A-Za-z](?:[A-Za-z0-9_&]|-[A-Za-z0-9_&])*/,
        )),
        $.wardley_name_word,
      ),
      repeat($._wardley_name_part),
  )),

  _wardley_immediate_string: (_) => token.immediate(prec(10, choice(
    seq('"', /(?:[^"\\]|\\.)*/, '"'),
    seq("'", /(?:[^'\\]|\\.)*/, "'"),
  ))),

  _wardley_string: ($) => alias(
    $._radar_wardley_quoted_string,
    $.quoted_string,
  ),

  _wardley_name_part: ($) => choice(
    $.wardley_name_word,
    $.wardley_name_hyphen,
    $.wardley_parenthesized_name_part,
  ),

  wardley_parenthesized_name_part: ($) => seq(
    choice(
      '(',
      // Langium's NAME_WITH_SPACES keeps ` (name)` inside the current name.
      // Match the gap and opening delimiter together so a link tail cannot
      // claim this boundary first.
      token(prec(1, /[ \t]+\(/)),
    ),
    repeat1(choice($.wardley_name_word, $.wardley_name_hyphen)),
    ')',
  ),

  wardley_incomplete_component_statement: ($) => choice(
    prec.dynamic(20, prec.right(-10, seq(
      statementKeyword($, 'component'),
      $._wardley_required_gap,
      field(
        'name',
        alias(
          $._radar_wardley_unclosed_quoted_string,
          $.wardley_unclosed_quoted_string,
        ),
      ),
      optional(field('text', $.wardley_incomplete_component_text)),
    ))),
    prec.right(-10, seq(
      statementKeyword($, 'component'),
      $._wardley_required_gap,
      field('text', $.wardley_unsupported_component_text),
    )),
    prec.right(-10, statementKeyword($, 'component')),
  ),

  wardley_malformed_statement: ($) => prec.right(-100, choice(
    seq(
      field(
        'keyword',
        alias($.wardley_name_word, $.wardley_unknown_keyword),
      ),
      optional(field('text', $.wardley_malformed_tail)),
    ),
    seq(
      field('text', $.wardley_malformed_text),
    ),
  )),

  wardley_pipeline_malformed_statement: ($) => prec.right(-100, choice(
    seq(
      field(
        'keyword',
        alias($.wardley_name_word, $.wardley_unknown_keyword),
      ),
      optional(field('text', $.wardley_malformed_tail)),
    ),
    seq(
      field('text', $.wardley_pipeline_malformed_text),
    ),
  )),

  wardley_arrow: (_) => token(prec(20, '->')),

  wardley_link_port: (_) => token.immediate(prec(20, choice('+<>', '+>', '+<'))),

  wardley_link_operator: (_) => token.immediate(prec(20, choice(
    /\+'[^'\r\n]*'(?:<>|<|>)/,
    '-.->',
    '-->',
    '->',
    '>',
  ))),

  wardley_strategy: (_) => choice('build', 'buy', 'outsource', 'market'),

  wardley_decimal: (_) => token(/[0-9]+\.[0-9]+/),

  wardley_integer: (_) => token(/0|[1-9][0-9]*/),

  wardley_signed_integer: ($) => choice(
    $.wardley_integer,
    seq(
      field('sign', '-'),
      field('value', $.wardley_integer),
    ),
  ),

  wardley_name_word: (_) => token(prec(
    5,
    /[A-Za-z](?:[A-Za-z0-9_&]|-[A-Za-z0-9_&])*/,
  )),

  wardley_name_hyphen: (_) => token(prec(-20, '-')),

  _wardley_required_gap: ($) => $._langium_inline_space,

  wardley_link_label_value: (_) => token.immediate(prec(5, /[^\s\r\n][^\r\n]*/)),

  wardley_incomplete_component_text: (_) => token(prec(-50, /[^\r\n]+/)),

  wardley_unsupported_component_text: (_) => token(prec(
    -50,
    /[^A-Za-z"'\r\n][^\r\n]*/,
  )),

  wardley_malformed_tail: (_) => token(prec(-10, /[^\r\n]+/)),

  wardley_malformed_text: (_) => token(prec(-100,
    /[^A-Za-z0-9_\u00c0-\uffff\r\n][^\r\n]*/,
  )),

  wardley_pipeline_malformed_text: (_) => token(prec(-100,
    /[^A-Za-z0-9_\u00c0-\uffff}\r\n][^}\r\n]*/,
  )),
};

module.exports = { wardleyRules };
