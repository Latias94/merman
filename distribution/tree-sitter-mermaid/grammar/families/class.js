// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/class/parser/classDiagram.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(
    token(prec(20, choice('classDiagram-v2', 'classDiagram'))),
    $.diagram_keyword,
  ),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.class_statement_keyword),
);

const classDeclarationKeyword = ($) => field(
  'keyword',
  alias('class', $.class_statement_keyword),
);

const className = ($, fieldName = 'name') => field(
  fieldName,
  alias($._class_identifier, $.class_name),
);

const classReference = ($, fieldName) => field(
  fieldName,
  alias($._class_identifier, $.class_reference),
);

const immediateClassReference = ($, fieldName) => field(
  fieldName,
  alias($._class_immediate_identifier, $.class_reference),
);

const trailingTrivia = ($) => optional(choice(
  field('comment', $.comment),
  field('directive', $.directive),
));

const optionalInlineGap = () => optional(token.immediate(/[ \t]+/));

const classRules = {
  class_diagram: ($) => seq(
    field('header', $.class_header),
    field('body', $.class_body),
  ),

  class_header: ($) => seq(
    diagramKeyword($),
    optional(token.immediate(/[ \t]+/)),
    trailingTrivia($),
    field('terminator', $._line_ending),
  ),

  class_body: ($) => choice(
    repeat1($._class_line_item),
    seq(repeat($._class_line_item), $._class_eof_item),
  ),

  _class_line_item: ($) => choice(
    seq(
      field('statement', $._class_statement),
      trailingTrivia($),
      $._line_ending,
    ),
    seq(choice($.comment, $.directive), $._line_ending),
    $._blank_line,
  ),

  _class_eof_item: ($) => choice(
    seq(field('statement', $._class_statement), trailingTrivia($)),
    $.comment,
    $.directive,
  ),

  _class_statement: ($) => choice(
    $.class_namespace_declaration,
    $.class_declaration,
    $.class_relationship,
    $.class_incomplete_relationship,
    $.class_member_statement,
    $.class_annotation_statement,
    $.class_note_statement,
    $.class_direction_statement,
    $.class_definition_statement,
    $.class_style_statement,
    $.class_css_class_statement,
    $.class_callback_statement,
    $.class_interaction_statement,
    $.class_accessibility_title_statement,
    $.class_accessibility_description_statement,
    $.class_malformed_statement,
  ),

  class_namespace_declaration: ($) => prec.right(seq(
    statementKeyword($, 'namespace'),
    token.immediate(/[ \t]+/),
    field('name', alias($._class_identifier, $.class_namespace_name)),
    optional(field('label', $.class_label)),
    field('open', '{'),
    optional(field('body', $.class_namespace_body)),
    field('close', '}'),
  )),

  class_namespace_body: ($) => repeat1(choice(
    seq(
      field('statement', choice(
        $.class_namespace_declaration,
        $.class_declaration,
        $.class_note_statement,
      )),
      trailingTrivia($),
      $._line_ending,
    ),
    seq(choice($.comment, $.directive), $._line_ending),
    $._blank_line,
  )),

  class_declaration: ($) => prec.right(seq(
    classDeclarationKeyword($),
    token.immediate(/[ \t]+/),
    className($),
    optional(field('label', $.class_label)),
    optional(field('class', $.class_style_annotation)),
    optional(field('annotation', $.class_stereotype)),
    optional(field('members', $.class_member_block)),
  )),

  class_label: ($) => seq(
    field('open', '['),
    field('text', choice($.class_string, $.class_unclosed_string)),
    optional(field('close', ']')),
  ),

  class_style_annotation: ($) => seq(
    field('operator', ':::'),
    field('name', alias($._class_style_identifier, $.class_style_name)),
  ),

  class_stereotype: ($) => seq(
    field('open', '<<'),
    field('name', $.class_annotation_name),
    field('close', '>>'),
  ),

  class_member_block: ($) => seq(
    field('open', '{'),
    repeat(choice(
      seq(field('member', $.class_member), $._line_ending),
      seq(choice($.comment, $.directive), $._line_ending),
      $._blank_line,
    )),
    optional(field('member', $.class_member)),
    field('close', '}'),
  ),

  // Mermaid trims class members before semantic parsing. Keep indentation and
  // trailing layout whitespace outside the named member span.
  class_member: (_) => token(prec(
    -5,
    /[^\s{}\r\n](?:[^{}\r\n]*[^\s{}\r\n])?/,
  )),

  class_relationship: ($) => prec(20, seq(
    classReference($, 'source'),
    optional(field('source_cardinality', $.class_cardinality)),
    field('operator', $.class_relationship_operator),
    optional(seq(
      optionalInlineGap(),
      field('target_cardinality', $.class_cardinality),
    )),
    optionalInlineGap(),
    immediateClassReference($, 'target'),
    optional(seq(
      field('delimiter', ':'),
      optional(field('label', $.class_relationship_label)),
    )),
  )),

  class_incomplete_relationship: ($) => prec(-20, seq(
    classReference($, 'source'),
    optional(field('source_cardinality', $.class_cardinality)),
    field('operator', $.class_relationship_operator),
    optional(field('target_cardinality', $.class_cardinality)),
    optional(field('recovery', $.class_relationship_recovery)),
  )),

  class_cardinality: ($) => $.class_string,

  class_relationship_operator: (_) => token(prec(
    30,
    /(?:<\||\(\)|[o*<>])?(?:--|\.\.)(?:\|>|\(\)|[o*<>])?/,
  )),

  class_relationship_label: (_) => token(prec(
    -5,
    /[^\s\r\n](?:[^\r\n]*[^\s\r\n])?/,
  )),

  class_relationship_recovery: (_) => token(prec(-100, /[^\r\n]+/)),

  class_member_statement: ($) => prec(10, seq(
    classReference($, 'owner'),
    field('delimiter', ':'),
    field('member', $.class_member),
  )),

  class_annotation_statement: ($) => seq(
    field('annotation', $.class_stereotype),
    classReference($, 'target'),
  ),

  class_note_statement: ($) => prec.right(seq(
    statementKeyword($, 'note'),
    optional(seq(
      token.immediate(/[ \t]+/),
      field('relation', alias('for', $.class_note_relation)),
      token.immediate(/[ \t]+/),
      classReference($, 'target'),
    )),
    token.immediate(/[ \t]+/),
    field('text', $.class_note_text),
  )),

  class_direction_statement: ($) => seq(
    statementKeyword($, 'direction'),
    token.immediate(/[ \t]+/),
    field('direction', alias(
      token.immediate(choice('TB', 'BT', 'RL', 'LR')),
      $.class_direction,
    )),
  ),

  class_definition_statement: ($) => seq(
    statementKeyword($, 'classDef'),
    token.immediate(/[ \t]+/),
    field('name', alias($._class_style_identifier, $.class_style_name)),
    repeat(seq(
      optional(token.immediate(/[ \t]+/)),
      ',',
      optional(token.immediate(/[ \t]+/)),
      field('name', alias($._class_style_identifier, $.class_style_name)),
    )),
    token.immediate(/[ \t]+/),
    field('styles', $.class_style_list),
  ),

  class_style_statement: ($) => seq(
    statementKeyword($, 'style'),
    token.immediate(/[ \t]+/),
    classReference($, 'target'),
    token.immediate(/[ \t]+/),
    field('styles', $.class_style_list),
  ),

  class_css_class_statement: ($) => seq(
    statementKeyword($, 'cssClass'),
    token.immediate(/[ \t]+/),
    field('targets', choice($.class_string, $.class_unclosed_string)),
    token.immediate(/[ \t]+/),
    field('class', alias($._class_style_identifier, $.class_style_name)),
  ),

  class_style_list: ($) => seq(
    field('style', $.class_style_item),
    repeat(seq(
      optional(token.immediate(/[ \t]+/)),
      ',',
      optional(token.immediate(/[ \t]+/)),
      field('style', $.class_style_item),
    )),
  ),

  class_style_item: (_) => token(prec(-5, /[^,\r\n]+/)),

  class_callback_statement: ($) => seq(
    field('keyword', $.class_callback_keyword),
    token.immediate(/[ \t]+/),
    classReference($, 'target'),
    token.immediate(/[ \t]+/),
    field('callback', choice($.class_string, $.class_unclosed_string)),
    optional(seq(
      token.immediate(/[ \t]+/),
      field('tooltip', choice($.class_string, $.class_unclosed_string)),
    )),
  ),

  class_callback_keyword: (_) => token(prec(50, 'callback')),

  class_interaction_statement: ($) => choice(
    seq(
      statementKeyword($, 'link'),
      token.immediate(/[ \t]+/),
      classReference($, 'target'),
      token.immediate(/[ \t]+/),
      field('url', choice($.class_string, $.class_unclosed_string)),
      optional(seq(
        token.immediate(/[ \t]+/),
        field('tooltip', choice($.class_string, $.class_unclosed_string)),
      )),
      optional(seq(
        token.immediate(/[ \t]+/),
        field('link_target', $.class_link_target),
      )),
    ),
    seq(
      statementKeyword($, 'click'),
      token.immediate(/[ \t]+/),
      classReference($, 'target'),
      token.immediate(/[ \t]+/),
      field('action', choice($.class_href_action, $.class_call_action)),
      optional(seq(
        token.immediate(/[ \t]+/),
        field('tooltip', choice($.class_string, $.class_unclosed_string)),
      )),
      optional(seq(
        token.immediate(/[ \t]+/),
        field('link_target', $.class_link_target),
      )),
    ),
  ),

  class_href_action: ($) => seq(
    statementKeyword($, 'href'),
    token.immediate(/[ \t]+/),
    field('url', choice($.class_string, $.class_unclosed_string)),
  ),

  class_call_action: ($) => seq(
    statementKeyword($, 'call'),
    token.immediate(/[ \t]+/),
    field('name', $.class_callback_name),
    field('open', token.immediate('(')),
    optional(field('arguments', $.class_callback_arguments)),
    optional(field('close', token.immediate(')'))),
  ),

  class_callback_name: (_) => token.immediate(/[A-Za-z_][A-Za-z0-9_.]*/),

  class_callback_arguments: (_) => token.immediate(/[^)\r\n]+/),

  class_link_target: (_) => choice('_self', '_blank', '_parent', '_top'),

  class_accessibility_title_statement: ($) => seq(
    statementKeyword($, 'accTitle'),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.class_line_text)),
  ),

  class_accessibility_description_statement: ($) => seq(
    statementKeyword($, 'accDescr'),
    choice(
      seq(
        optional(token.immediate(/[ \t]+/)),
        field('delimiter', token.immediate(':')),
        optional(token.immediate(/[ \t]+/)),
        optional(field('text', $.class_line_text)),
      ),
      seq(
        optional(token.immediate(/[ \t]+/)),
        field('description', choice(
          $.class_accessibility_description_block,
          $.class_unclosed_accessibility_description_block,
        )),
      ),
    ),
  ),

  class_accessibility_description_block: (_) => token(seq('{', /[^}]*/, '}')),

  class_unclosed_accessibility_description_block: (_) => token(prec(
    -10,
    seq('{', /[^}\r\n]*/),
  )),

  class_string: ($) => prec.dynamic(10, seq(
    '"',
    repeat(choice($._class_string_content, $._line_ending)),
    token.immediate('"'),
  )),

  class_unclosed_string: ($) => prec.dynamic(-10, seq(
    '"',
    repeat($._class_string_content),
  )),

  _class_string_content: (_) => token.immediate(/[^"\r\n]+/),

  class_note_text: ($) => choice(
    token(prec(20, seq('"', /[^\r\n]*/, '"'))),
    $.class_string,
    $.class_unclosed_string,
  ),

  class_line_text: (_) => token(prec(-5, /[^\r\n]+/)),

  class_annotation_name: (_) => token.immediate(/[A-Za-z0-9_\-\u00c0-\uffff]+/),

  _class_identifier: ($) => choice(
    $.identifier,
    token(prec(5, choice(
      seq('`', /[^`]+/, '`'),
      /[0-9][A-Za-z0-9_.\-\u00c0-\uffff]*(?:~[^~\r\n]+~)?/,
      /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\-\u00c0-\uffff]*\.[A-Za-z0-9_.\-\u00c0-\uffff]+(?:~[^~\r\n]+~)?/,
      /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_.\-\u00c0-\uffff]*~[^~\r\n]+~/,
    ))),
  ),

  _class_immediate_identifier: ($) => choice(
    alias(
      token.immediate(/[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\-\u00c0-\uffff]*/),
      $.identifier,
    ),
    token.immediate(prec(5, choice(
      seq('`', /[^`]+/, '`'),
      /[0-9][A-Za-z0-9_.\-\u00c0-\uffff]*(?:~[^~\r\n]+~)?/,
      /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\-\u00c0-\uffff]*\.[A-Za-z0-9_.\-\u00c0-\uffff]+(?:~[^~\r\n]+~)?/,
      /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_.\-\u00c0-\uffff]*~[^~\r\n]+~/,
    ))),
  ),

  _class_style_identifier: (_) => token.immediate(/[A-Za-z_][A-Za-z0-9_\-]*/),

  class_malformed_statement: ($) => prec(-100, field(
    'text',
    $.class_malformed_text,
  )),

  class_malformed_text: (_) => token(prec(-100, /[^\r\n]+/)),
};

const classConflicts = ($) => [
  [$.class_string, $.class_unclosed_string],
];

module.exports = { classConflicts, classRules };
