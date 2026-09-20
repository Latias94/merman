// Source translation: Mermaid 12.0.0, commit 98a0945418c76238f15df2afaddbba4272656c3b.
// packages/mermaid/src/diagrams/usecase/parser/usecase.{parser,tokens}.ts
// Semantic resolution and validation remain owned by Merman Core.

// Equal lexical precedence preserves upstream longer_alt: actorUser is one id.
const keyword = ($, text) => field(
  'keyword', alias(token(text), $.usecase_statement_keyword),
);
const identifier = ($, name = 'id') => field(name, $.usecase_identifier);
const terminator = ($) => choice($._line_ending, $._end_of_input);
const list = (rule) => seq(rule, repeat(seq(',', rule)));
const operator = (text) => token(prec(30, text));

const usecaseRules = {
  usecase_diagram: ($) => seq(
    field('header', $.usecase_header),
    optional(field('body', $.usecase_body)),
  ),

  usecase_header: ($) => seq(
    field('keyword', alias(token(prec(50, 'usecase-beta')), $.diagram_keyword)),
    terminator($),
  ),

  usecase_body: ($) => repeat1(choice(
    seq($._usecase_statement, terminator($)),
    seq($.comment, terminator($)),
    $._blank_line,
  )),

  _usecase_statement: ($) => choice(
    $.usecase_actor_statement,
    $.usecase_node_statement,
    $.usecase_edge_statement,
    $.usecase_boundary_statement,
    $.usecase_note_statement,
    $.usecase_json_statement,
    $.usecase_direction_statement,
    $.usecase_class_definition_statement,
    $.usecase_class_statement,
    $.usecase_style_statement,
    $.usecase_accessibility_title_statement,
    $.usecase_accessibility_description_statement,
  ),

  // Upstream identifiers are ASCII word characters, including digit-leading ids.
  usecase_identifier: (_) => token(/\w+/),

  usecase_actor_statement: ($) => seq(
    keyword($, 'actor'),
    list($.usecase_actor),
  ),

  usecase_actor: ($) => seq(
    choice(
      seq(identifier($), optional($.usecase_actor_label)),
      field('label', $.usecase_string),
    ),
    optional(field('metadata', $.usecase_metadata)),
    optional(field('stereotype', $.usecase_stereotype)),
    optional(field('classes', $.usecase_class_suffix)),
  ),

  usecase_actor_label: ($) => seq('(', field('text', $.usecase_label), ')'),

  usecase_node_statement: ($) => field('node', $.usecase_node),

  usecase_node: ($) => seq(
    $._usecase_node_name,
    optional(field('metadata', $.usecase_metadata)),
    optional(field('stereotype', $.usecase_stereotype)),
    optional(field('classes', $.usecase_class_suffix)),
  ),

  _usecase_node_name: ($) => choice(
    seq(identifier($), optional(field('label', $.usecase_node_label))),
    field('label', $.usecase_string),
  ),

  usecase_node_label: ($) => choice(
    seq('(', field('text', $.usecase_label), ')'),
    seq('[', field('text', $.usecase_label), ']'),
  ),

  usecase_label: ($) => choice(
    $.usecase_string,
    repeat1($._usecase_label_component),
  ),

  _usecase_label_component: ($) => choice(
    $.usecase_identifier,
    alias(token(/[^\t\r\n !-~]+/), $.usecase_label_text),
    alias(token(/(?:\d+\.\d+|\.\d+)(?:[A-Za-z]+)?/), $.usecase_label_text),
    alias(token(/[!#$%&*+,./:;=?^_|~\\`-]/), $.usecase_label_text),
    alias('@', $.usecase_label_text),
    alias(token(prec(-1, choice('<', '>'))), $.usecase_label_text),
  ),

  // Plain strings have no escape processing upstream; markdown strings may span lines.
  usecase_string: (_) => token(choice(
    /"[^"\r\n]*"/,
    /'[^'\r\n]*'/,
    /"`(?:[^`]|`[^"])*`"/,
  )),

  usecase_stereotype: ($) => seq(
    '<<', field('text', alias(token(/(?:[^>\r\n]|>[^>\r\n])+/), $.usecase_stereotype_text)), '>>',
  ),

  usecase_class_suffix: ($) => prec.right(seq(':::', list(identifier($, 'class')))),

  usecase_metadata: ($) => seq(
    '@{', repeat($._line_ending),
    optional(seq(
      $.usecase_metadata_property,
      repeat(seq($._usecase_metadata_separator, $.usecase_metadata_property)),
      optional(','), repeat($._line_ending),
    )),
    '}',
  ),

  usecase_metadata_property: ($) => seq(
    field('key', choice($.usecase_identifier, $.usecase_string)), ':',
    field('value', choice($.usecase_identifier, $.usecase_string)),
  ),

  _usecase_metadata_separator: ($) => choice(
    seq(',', repeat($._line_ending)),
    seq(repeat1($._line_ending), optional(seq(',', repeat($._line_ending)))),
  ),

  usecase_edge_statement: ($) => seq(
    choice(
      field('source', $.usecase_node),
      seq(keyword($, 'actor'), field('source', $.usecase_actor)),
    ),
    optional(seq(identifier($, 'edge_id'), '@')),
    field('operator', $.usecase_operator),
    field('target', $.usecase_node),
  ),

  usecase_operator: ($) => choice(
    operator(/--+>/),
    operator('<--'), operator(/<---+/),
    operator('--'), operator(/---+/),
    operator('--o'), operator('o--'), operator('--x'), operator('x--'), operator('--|>'),
    seq(operator('..>'), ':', field('relation', alias(token(/include|extend/i), $.usecase_relation_kind))),
    seq(
      operator('--'), field('label', $.usecase_edge_label),
      choice(operator(/--+>/), operator('--'), operator(/---+/), operator('--o'), operator('--x')),
    ),
    seq(
      choice(operator('<--'), operator('o--'), operator('x--')),
      field('label', $.usecase_edge_label), choice(operator('--'), operator(/---+/)),
    ),
  ),

  usecase_edge_label: ($) => $.usecase_label,

  usecase_boundary_statement: ($) => seq(
    keyword($, 'systemBoundary'),
    $._usecase_node_name,
    optional(field('metadata', $.usecase_metadata)),
    optional(field('classes', $.usecase_class_suffix)),
    $._line_ending,
    optional(field('body', $.usecase_boundary_body)),
    field('end', $.usecase_end_statement),
  ),

  usecase_boundary_body: ($) => repeat1(choice(
    seq(choice($.usecase_actor_statement, $.usecase_node_statement), $._line_ending),
    seq($.comment, $._line_ending),
    $._blank_line,
  )),

  usecase_end_statement: ($) => keyword($, 'end'),

  usecase_note_statement: ($) => seq(
    keyword($, 'note'), keyword($, 'for'), identifier($, 'target'),
    field('label', $.usecase_label),
  ),

  usecase_json_statement: ($) => seq(
    keyword($, 'json'), identifier($), '@',
    field('value', $.usecase_json_object),
    optional(field('classes', $.usecase_class_suffix)),
  ),

  // Keep nested JSON structural rather than consuming through the next closing brace.
  usecase_json_object: ($) => seq(
    '{', repeat($._line_ending),
    optional(seq(
      $.usecase_json_property,
      repeat(seq(',', repeat($._line_ending), $.usecase_json_property)),
    )), '}',
  ),

  usecase_json_property: ($) => seq(
    field('key', $.usecase_json_string), ':', repeat($._line_ending),
    field('value', $._usecase_json_value), repeat($._line_ending),
  ),

  _usecase_json_value: ($) => choice(
    $.usecase_json_object, $.usecase_json_array, $.usecase_json_string,
    $.usecase_json_number, $.usecase_json_literal,
  ),

  usecase_json_string: (_) => token(/"(?:[^"\\\x00-\x1f]|\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4}))*"/),
  usecase_json_number: (_) => token(/-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/),
  usecase_json_literal: (_) => choice('true', 'false', 'null'),

  usecase_json_array: ($) => seq(
    '[', repeat($._line_ending),
    optional(seq(
      $._usecase_json_value, repeat($._line_ending),
      repeat(seq(',', repeat($._line_ending), $._usecase_json_value, repeat($._line_ending))),
    )), ']',
  ),

  usecase_direction_statement: ($) => seq(
    keyword($, 'direction'), field('direction', alias(choice('TD', 'TB', 'BT', 'LR', 'RL'), $.direction)),
  ),

  usecase_class_definition_statement: ($) => seq(
    keyword($, 'classDef'), list(identifier($, 'class')), $.usecase_styles,
  ),

  usecase_class_statement: ($) => seq(
    keyword($, 'class'), list(identifier($, 'target')), list(identifier($, 'class')),
  ),

  usecase_style_statement: ($) => seq(
    keyword($, 'style'), identifier($, 'target'), $.usecase_styles,
  ),

  usecase_styles: ($) => list($.usecase_style_property),

  usecase_style_property: ($) => seq(
    field('key', alias(token(/(?:--)?[A-Za-z_][\w-]*/), $.usecase_style_name)), ':',
    field('value', $.usecase_style_value),
  ),

  usecase_style_value: (_) => token(/(?:\\,|[^,\r\n])+/),

  usecase_accessibility_title_statement: ($) => seq(
    keyword($, 'accTitle'), ':', optional(field('text', $.usecase_accessibility_text)),
  ),

  usecase_accessibility_description_statement: ($) => seq(
    keyword($, 'accDescr'), choice(
      seq(':', optional(field('text', $.usecase_accessibility_text))),
      seq('{', optional(field('text', alias(token(/[^}]+/), $.usecase_accessibility_text))), '}'),
    ),
  ),

  usecase_accessibility_text: (_) => token(/[^\r\n]+/),
};

const usecaseConflicts = ($) => [
  [$.usecase_operator],
];

module.exports = { usecaseConflicts, usecaseRules };
