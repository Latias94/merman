// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/requirement/parser/requirementDiagram.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const requirementHeaderPattern = /requirement[dD][iI][aA][gG][rR][aA][mM]/;

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, requirementHeaderPattern)), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.requirement_statement_keyword),
);

const attributeKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.requirement_attribute_keyword),
);

const inlineComment = ($) => choice(
  field('comment', $.comment),
  field('directive', $.directive),
  field('comment', $.requirement_hash_comment),
);

const nameValue = ($) => field('name', $.requirement_name);

const classAnnotation = ($) => optional(field(
  'class',
  $.requirement_class_annotation,
));

const requirementRules = {
  requirement_diagram: ($) => choice(
    seq(
      field('header', $.requirement_header),
      optional(field('body', $.requirement_body)),
    ),
    field(
      'header',
      alias($._requirement_header_eof, $.requirement_header),
    ),
  ),

  requirement_header: ($) => seq(
    diagramKeyword($),
    field('terminator', $._line_ending),
  ),

  _requirement_header_eof: ($) => diagramKeyword($),

  requirement_body: ($) => choice(
    repeat1($._requirement_line_item),
    seq(
      repeat($._requirement_line_item),
      $._requirement_eof_item,
    ),
  ),

  _requirement_line_item: ($) => choice(
    seq($._requirement_statement, optional(inlineComment($)), $._line_ending),
    seq(
      choice($.comment, $.directive, $.requirement_hash_comment),
      $._line_ending,
    ),
    $._blank_line,
  ),

  _requirement_eof_item: ($) => choice(
    seq($._requirement_statement, optional(inlineComment($))),
    $.comment,
    $.directive,
    $.requirement_hash_comment,
  ),

  _requirement_statement: ($) => choice(
    $.requirement_title_statement,
    $.requirement_accessibility_title_statement,
    $.requirement_accessibility_description_statement,
    $.requirement_direction_statement,
    $.requirement_declaration,
    $.requirement_element_declaration,
    $.requirement_relationship_statement,
    $.requirement_incomplete_relationship_statement,
    $.requirement_incomplete_declaration_statement,
    $.requirement_class_definition_statement,
    $.requirement_class_assignment_statement,
    $.requirement_style_statement,
    $.requirement_class_shorthand_statement,
  ),

  requirement_title_statement: ($) => prec(30, seq(
    statementKeyword($, /title/i),
    token.immediate(/[ \t]+/),
    field('text', $.requirement_line_text),
  )),

  requirement_accessibility_title_statement: ($) => prec(30, seq(
    statementKeyword($, /accTitle/i),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.requirement_line_text)),
  )),

  requirement_accessibility_description_statement: ($) => prec(30, seq(
    statementKeyword($, /accDescr/i),
    optional(token.immediate(/[ \t]+/)),
    choice(
      seq(
        field('delimiter', token.immediate(':')),
        optional(token.immediate(/[ \t]+/)),
        optional(field('text', $.requirement_line_text)),
      ),
      field('description', $.requirement_accessibility_description_block),
      field('description', $.requirement_unclosed_accessibility_block),
    ),
  )),

  requirement_accessibility_description_block: ($) => seq(
    repeat($._line_ending),
    field('text', $.requirement_accessibility_block_text),
  ),

  requirement_accessibility_block_text: (_) => token(seq('{', /[^}]*/, '}')),

  requirement_unclosed_accessibility_block: (_) => token(prec(
    -10,
    seq('{', /[^}\r\n]*/),
  )),

  requirement_direction_statement: ($) => prec(30, seq(
    statementKeyword($, /direction/i),
    token.immediate(/[ \t]+/),
    field('direction', $.requirement_direction),
  )),

  requirement_direction: (_) => token(prec(20, choice(
    /TB/i,
    /BT/i,
    /RL/i,
    /LR/i,
  ))),

  requirement_declaration: ($) => prec.right(seq(
    field('kind', $.requirement_kind),
    token.immediate(/[ \t]+/),
    nameValue($),
    classAnnotation($),
    field('open', '{'),
    $._line_ending,
    repeat($._requirement_attribute_line),
    field('close', '}'),
  )),

  requirement_element_declaration: ($) => prec.right(seq(
    statementKeyword($, /element/i),
    token.immediate(/[ \t]+/),
    nameValue($),
    classAnnotation($),
    field('open', '{'),
    $._line_ending,
    repeat($._requirement_element_attribute_line),
    field('close', '}'),
  )),

  _requirement_attribute_line: ($) => choice(
    seq(
      field('attribute', choice(
        $.requirement_attribute,
        $.requirement_malformed_attribute,
      )),
      optional(inlineComment($)),
      $._line_ending,
    ),
    seq(
      choice($.comment, $.directive, $.requirement_hash_comment),
      $._line_ending,
    ),
    $._blank_line,
  ),

  _requirement_element_attribute_line: ($) => choice(
    seq(
      field('attribute', choice(
        $.requirement_element_attribute,
        $.requirement_malformed_element_attribute,
      )),
      optional(inlineComment($)),
      $._line_ending,
    ),
    seq(
      choice($.comment, $.directive, $.requirement_hash_comment),
      $._line_ending,
    ),
    $._blank_line,
  ),

  requirement_attribute: ($) => choice(
    seq(
      attributeKeyword($, /id/i),
      field('delimiter', ':'),
      optional(token.immediate(/[ \t]+/)),
      field('value', choice(
        $.requirement_string,
        $.requirement_unclosed_string,
        $.requirement_attribute_text,
      )),
    ),
    seq(
      attributeKeyword($, /text/i),
      field('delimiter', ':'),
      optional(token.immediate(/[ \t]+/)),
      field('value', choice(
        $.requirement_string,
        $.requirement_unclosed_string,
        $.requirement_attribute_text,
      )),
    ),
    seq(
      attributeKeyword($, /risk/i),
      field('delimiter', ':'),
      optional(token.immediate(/[ \t]+/)),
      field('value', $.requirement_risk),
    ),
    seq(
      attributeKeyword($, /verifyMethod/i),
      field('delimiter', ':'),
      optional(token.immediate(/[ \t]+/)),
      field('value', $.requirement_verify_method),
    ),
  ),

  requirement_element_attribute: ($) => choice(
    seq(
      attributeKeyword($, /type/i),
      field('delimiter', ':'),
      optional(token.immediate(/[ \t]+/)),
      field('value', choice(
        $.requirement_string,
        $.requirement_unclosed_string,
        $.requirement_attribute_text,
      )),
    ),
    seq(
      attributeKeyword($, /docref/i),
      field('delimiter', ':'),
      optional(token.immediate(/[ \t]+/)),
      field('value', choice(
        $.requirement_string,
        $.requirement_unclosed_string,
        $.requirement_attribute_text,
      )),
    ),
  ),

  requirement_malformed_attribute: ($) => prec(-10, field(
    'text',
    $.requirement_malformed_attribute_text,
  )),

  requirement_malformed_element_attribute: ($) => prec(-10, field(
    'text',
    $.requirement_malformed_element_attribute_text,
  )),

  requirement_malformed_attribute_text: (_) => token(prec(
    40,
    /(?:id|text|risk|verifyMethod)[ \t]+[^\r\n}]+/i,
  )),

  requirement_malformed_element_attribute_text: (_) => token(prec(
    40,
    /(?:type|docref)[ \t]+[^\r\n}]+/i,
  )),

  requirement_incomplete_declaration_statement: ($) => prec(-20, seq(
    field('kind', choice(
      $.requirement_kind,
      alias(token(prec(20, /element/i)), $.requirement_statement_keyword),
    )),
    token.immediate(/[ \t]+/),
    nameValue($),
    classAnnotation($),
  )),

  requirement_relationship_statement: ($) => choice(
    seq(
      field('source', $.requirement_reference),
      field('operator', alias('-', $.requirement_relationship_operator)),
      field('relationship', $.requirement_relationship_kind),
      field('operator', alias('->', $.requirement_relationship_operator)),
      optional(token.immediate(/[ \t]+/)),
      field('target', $.requirement_reference),
    ),
    seq(
      field('target', $.requirement_reference),
      field('operator', alias('<-', $.requirement_relationship_operator)),
      field('relationship', $.requirement_relationship_kind),
      field('operator', alias('-', $.requirement_relationship_operator)),
      optional(token.immediate(/[ \t]+/)),
      field('source', $.requirement_reference),
    ),
  ),

  requirement_incomplete_relationship_statement: ($) => prec(-20, choice(
    seq(
      field('source', $.requirement_reference),
      field('operator', alias('-', $.requirement_relationship_operator)),
      field('relationship', $.requirement_relationship_kind),
      field('operator', alias('->', $.requirement_relationship_operator)),
    ),
    seq(
      field('target', $.requirement_reference),
      field('operator', alias('<-', $.requirement_relationship_operator)),
      field('relationship', $.requirement_relationship_kind),
      field('operator', alias('-', $.requirement_relationship_operator)),
    ),
  )),

  requirement_relationship_kind: (_) => token(prec(20, choice(
    /contains/i,
    /copies/i,
    /derives/i,
    /satisfies/i,
    /verifies/i,
    /refines/i,
    /traces/i,
  ))),

  requirement_class_definition_statement: ($) => prec(30, seq(
    statementKeyword($, /classDef/i),
    token.immediate(/[ \t]+/),
    field('class', $.requirement_identifier_list),
    token.immediate(/[ \t]+/),
    field('style', $.requirement_style_declaration),
    repeat(seq(
      field('delimiter', ','),
      field('style', $.requirement_style_declaration),
    )),
  )),

  requirement_class_assignment_statement: ($) => prec(30, seq(
    statementKeyword($, /class/i),
    token.immediate(/[ \t]+/),
    field('target', $.requirement_identifier_list),
    token.immediate(/[ \t]+/),
    field('class', $.requirement_identifier_list),
  )),

  requirement_style_statement: ($) => prec(30, seq(
    statementKeyword($, /style/i),
    token.immediate(/[ \t]+/),
    field('target', $.requirement_identifier_list),
    token.immediate(/[ \t]+/),
    field('style', $.requirement_style_declaration),
    repeat(seq(
      field('delimiter', ','),
      field('style', $.requirement_style_declaration),
    )),
  )),

  requirement_class_shorthand_statement: ($) => prec(20, seq(
    field('target', $.requirement_reference),
    field('class', $.requirement_class_annotation),
  )),

  requirement_class_annotation: ($) => seq(
    field('delimiter', ':::'),
    field('class', $.requirement_style_identifier),
    repeat(seq(
      field('delimiter', ','),
      field('class', $.requirement_style_identifier),
    )),
  ),

  requirement_identifier_list: ($) => seq(
    field('item', $.requirement_style_identifier),
    repeat(seq(
      field('delimiter', ','),
      field('item', $.requirement_style_identifier),
    )),
  ),

  requirement_style_declaration: ($) => seq(
    field('property', $.requirement_style_property),
    field('delimiter', ':'),
    field('value', $.requirement_style_value),
  ),

  requirement_kind: (_) => token(prec(20, choice(
    /requirement/i,
    /functionalRequirement/i,
    /interfaceRequirement/i,
    /performanceRequirement/i,
    /physicalRequirement/i,
    /designConstraint/i,
  ))),

  requirement_risk: (_) => token.immediate(prec(20, choice(
    /low/i,
    /medium/i,
    /high/i,
  ))),

  requirement_verify_method: (_) => token.immediate(prec(20, choice(
    /analysis/i,
    /demonstration/i,
    /inspection/i,
    /test/i,
  ))),

  requirement_name: ($) => choice(
    $.requirement_string,
    $.requirement_unclosed_string,
    $.requirement_unquoted_name,
  ),

  requirement_reference: ($) => choice(
    $.requirement_string,
    $.requirement_unclosed_string,
    $.requirement_unquoted_reference,
  ),

  requirement_string: (_) => token.immediate(prec(
    40,
    /"(?:[^"\\\r\n]|\\.)*"/,
  )),

  requirement_unclosed_string: (_) => token.immediate(prec(
    30,
    /"(?:[^"\\\r\n]|\\.)*/,
  )),

  requirement_unquoted_name: (_) => token(prec(
    5,
    /[A-Za-z0-9_\u00c0-\uffff](?:[^"':,%#{}\r\n<>=-]*[^\s"':,%#{}\r\n<>=-])?/,
  )),

  requirement_unquoted_reference: (_) => token(prec(
    5,
    /[A-Za-z0-9_\u00c0-\uffff](?:[^"':,%#{}\r\n<>=-]*[^\s"':,%#{}\r\n<>=-])?/,
  )),

  requirement_style_identifier: (_) => token(prec(
    1,
    /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\-\u00c0-\uffff]*/,
  )),

  requirement_style_property: (_) => token(prec(
    10,
    /[A-Za-z_][A-Za-z0-9_-]*/,
  )),

  requirement_style_value: (_) => token(prec(
    -5,
    /(?:[^,%\r\n]|%[^%\r\n])+/
  )),

  requirement_line_text: (_) => token(prec(
    20,
    /(?:[^%#\r\n]|%[^%\r\n])+/
  )),

  requirement_attribute_text: (_) => token.immediate(prec(
    20,
    /[^\s%#}\r\n](?:(?:[^%#}\r\n]|%[^%\r\n])*[^\s%#}\r\n])?/
  )),

  requirement_hash_comment: (_) => token(seq('#', /[^\r\n]*/)),
};

module.exports = { requirementRules };
