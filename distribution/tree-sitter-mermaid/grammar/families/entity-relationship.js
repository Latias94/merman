// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/er/parser/erDiagram.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(
    token(prec(20, /erDiagram/i)),
    $.diagram_keyword,
  ),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.er_statement_keyword),
);

const entityName = ($, fieldName = 'name') => field(
  fieldName,
  alias($._er_entity_name, $.er_entity_name),
);

const entityReference = ($, fieldName) => field(
  fieldName,
  alias($._er_entity_name, $.er_entity_reference),
);

const trailingTrivia = ($) => optional(choice(
  field('comment', $.comment),
  field('directive', $.directive),
));

const optionalInlineGap = () => optional(token.immediate(/[ \t]+/));

const attributeComment = ($) => field('comment', choice(
  alias(
    token(seq('"', /[^"\r\n]*/, '"')),
    $.er_quoted_text,
  ),
  alias(
    token(prec(-10, seq('"', /[^"\r\n]*/))),
    $.er_unclosed_quoted_text,
  ),
));

const entityRelationshipRules = {
  entity_relationship_diagram: ($) => choice(
    seq(
      field('header', $.entity_relationship_header),
      optional(field('body', $.entity_relationship_body)),
    ),
    field(
      'header',
      alias($._entity_relationship_header_eof, $.entity_relationship_header),
    ),
  ),

  entity_relationship_header: ($) => seq(
    diagramKeyword($),
    optional(token.immediate(/[ \t]+/)),
    trailingTrivia($),
    field('terminator', $._er_line_ending),
  ),

  _entity_relationship_header_eof: ($) => seq(
    diagramKeyword($),
    optional(token.immediate(/[ \t]+/)),
  ),

  entity_relationship_body: ($) => choice(
    repeat1($._er_line_item),
    seq(repeat($._er_line_item), $._er_eof_item),
  ),

  _er_line_item: ($) => choice(
    seq(
      field('statement', $._er_statement),
      trailingTrivia($),
      $._er_line_ending,
    ),
    seq(choice($.comment, $.directive), $._er_line_ending),
    $._blank_line,
  ),

  _er_eof_item: ($) => choice(
    seq(field('statement', $._er_statement), trailingTrivia($)),
    $.comment,
    $.directive,
  ),

  _er_statement: ($) => choice(
    $.er_relationship,
    $.er_incomplete_relationship,
    $.er_alias_declaration,
    $.er_entity_declaration,
    $.er_title_statement,
    $.er_accessibility_title_statement,
    $.er_accessibility_description_statement,
    $.er_direction_statement,
    $.er_definition_statement,
    $.er_class_assignment_statement,
    $.er_style_statement,
    $.er_malformed_statement,
  ),

  er_entity_declaration: ($) => prec.right(60, seq(
    entityName($),
    optional(field('alias', $.er_entity_alias)),
    optional(choice(
      field('class', $.er_class_annotation),
      seq(
        token.immediate(/[ \t]+/),
        field('class', $.er_class_annotation),
      ),
    )),
    optional(choice(
      field('attributes', $.er_attribute_block),
      seq(
        token.immediate(/[ \t]+/),
        field('attributes', $.er_attribute_block),
      ),
    )),
  )),

  // Upstream's error recovery deliberately tolerates a leading word before an
  // alias declaration (the parser tests use `buzz` and `fizz`). Keep that
  // editing shape explicit instead of losing the actual aliased entity in a
  // generic recovery line.
  er_alias_declaration: ($) => prec(80, seq(
    field('prefix', alias($._er_entity_name, $.er_alias_prefix)),
    token.immediate(/[ \t]+/),
    entityName($),
    field('alias', $.er_entity_alias),
    optional(field('class', $.er_class_annotation)),
  )),

  er_entity_alias: ($) => seq(
    field('open', '['),
    field('text', choice(
      $.er_quoted_text,
      $.er_unclosed_quoted_text,
      $.er_alias_text,
    )),
    optional(field('close', ']')),
  ),

  er_class_annotation: ($) => seq(
    field('operator', ':::'),
    optional(token.immediate(/[ \t]+/)),
    field('class', $.er_identifier_list),
  ),

  er_attribute_block: ($) => seq(
    field('open', '{'),
    repeat($._er_attribute_block_line),
    optional(seq(
      optionalInlineGap(),
      choice(
        field('attribute', $.er_attribute),
        field('recovery', $.er_incomplete_attribute),
        field('recovery', $.er_malformed_attribute),
      ),
    )),
      field('close', alias(token(prec(100, /[ \t]*}/)), '}')),
  ),

  _er_attribute_block_line: ($) => seq(
    optionalInlineGap(),
    choice(
      seq(field('attribute', $.er_attribute), $._er_line_ending),
      seq(field('recovery', $.er_incomplete_attribute), $._er_line_ending),
      seq(field('recovery', $.er_malformed_attribute), $._er_line_ending),
      seq(choice($.comment, $.directive), $._er_line_ending),
      $._er_line_ending,
    ),
  ),

  er_attribute: ($) => prec.right(20, seq(
    field('type', alias($._er_attribute_word, $.er_attribute_type)),
    optional(field('optional', token.immediate('?'))),
    token.immediate(/[ \t]+/),
    field('name', alias($._er_attribute_word, $.er_attribute_name)),
    optional(seq(
      token.immediate(/[ \t]+/),
      optional(choice(
        seq(
          field('keys', $.er_attribute_key_list),
          optional(seq(
            token.immediate(/[ \t]+/),
            attributeComment($),
          )),
        ),
        attributeComment($),
      )),
    )),
  )),

  er_attribute_key_list: ($) => seq(
    field('key', $.er_attribute_key),
    repeat(seq(
      token.immediate(','),
      optional(token.immediate(/[ \t]+/)),
      field('key', $.er_attribute_key),
    )),
  ),

  er_attribute_key: (_) => token.immediate(prec(30, /(?:PK|FK|UK)/i)),

  er_incomplete_attribute: ($) => prec(-20, field(
    'type',
    alias($._er_attribute_word, $.er_attribute_type),
  )),

  er_malformed_attribute: ($) => prec(-100, field(
    'text',
    $.er_malformed_attribute_text,
  )),

  er_malformed_attribute_text: (_) => token(prec(-100, /[^{}|:\r\n]+/)),

  _er_line_ending: (_) => token(/[ \t]*(?:\r\n|\n|\r)/),

  er_relationship: ($) => prec(40, seq(
    entityReference($, 'source'),
    optional(field('source_class', $.er_class_annotation)),
    optionalInlineGap(),
    field('source_cardinality', $.er_cardinality),
    field('operator', $.er_relationship_operator),
    optionalInlineGap(),
    field('target_cardinality', $.er_cardinality),
    optionalInlineGap(),
    entityReference($, 'target'),
    optional(field('target_class', $.er_class_annotation)),
    field('delimiter', ':'),
    optional(field('role', choice(
      $.er_quoted_text,
      $.er_unclosed_quoted_text,
      $.er_role_text,
    ))),
  )),

  er_incomplete_relationship: ($) => prec(-20, seq(
    entityReference($, 'source'),
    optional(field('source_class', $.er_class_annotation)),
    optionalInlineGap(),
    field('source_cardinality', $.er_cardinality),
    field('operator', $.er_relationship_operator),
    optionalInlineGap(),
    field('target_cardinality', $.er_cardinality),
    optional(field('recovery', $.er_relationship_recovery)),
  )),

  er_cardinality: (_) => token.immediate(prec(30, choice(
    /one[ \t]+or[ \t]+zero/i,
    /zero[ \t]+or[ \t]+one/i,
    /one[ \t]+or[ \t]+(?:more|many)/i,
    /zero[ \t]+or[ \t]+(?:more|many)/i,
    /many\((?:0|1)\)/i,
    /only[ \t]+one/i,
    /many/i,
    /one/i,
    '1+',
    '0+',
    '||',
    '|o',
    'o|',
    '}o',
    'o{',
    '}|',
    '|{',
    '1',
    'u',
  ))),

  er_relationship_operator: (_) => token(prec(30, choice(
    /optionally[ \t]+to/i,
    /to/i,
    '--',
    '..',
    '.-',
    '-.',
  ))),

  er_relationship_recovery: (_) => token(prec(-100, /[^\r\n]+/)),

  er_title_statement: ($) => seq(
    statementKeyword($, /title/i),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.er_line_text)),
  ),

  er_accessibility_title_statement: ($) => seq(
    statementKeyword($, /accTitle/i),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.er_line_text)),
  ),

  er_accessibility_description_statement: ($) => seq(
    statementKeyword($, /accDescr/i),
    choice(
      seq(
        optional(token.immediate(/[ \t]+/)),
        field('delimiter', token.immediate(':')),
        optional(token.immediate(/[ \t]+/)),
        optional(field('text', $.er_line_text)),
      ),
      seq(
        optional(token.immediate(/[ \t]+/)),
        field('description', choice(
          $.er_accessibility_description_block,
          $.er_unclosed_accessibility_description_block,
        )),
      ),
    ),
  ),

  er_accessibility_description_block: (_) => token(seq('{', /[^}]*/, '}')),

  er_unclosed_accessibility_description_block: (_) => token(prec(
    -10,
    seq('{', /[^}\r\n]*/),
  )),

  er_direction_statement: ($) => seq(
    statementKeyword($, /direction/i),
    token.immediate(/[ \t]+/),
    field('direction', alias(
      token.immediate(/(?:TB|BT|RL|LR)/i),
      $.er_direction,
    )),
  ),

  er_definition_statement: ($) => seq(
    statementKeyword($, /classDef/i),
    token.immediate(/[ \t]+/),
    field('classes', $.er_identifier_list),
    token.immediate(/[ \t]+/),
    field('styles', $.er_style_list),
  ),

  er_class_assignment_statement: ($) => seq(
    statementKeyword($, /class/i),
    token.immediate(/[ \t]+/),
    field('targets', $.er_identifier_list),
    token.immediate(/[ \t]+/),
    field('classes', $.er_identifier_list),
  ),

  er_style_statement: ($) => seq(
    statementKeyword($, /style/i),
    token.immediate(/[ \t]+/),
    field('targets', $.er_identifier_list),
    token.immediate(/[ \t]+/),
    field('styles', $.er_style_list),
  ),

  er_identifier_list: ($) => seq(
    field('name', alias($._er_style_identifier, $.er_style_name)),
    repeat(seq(
      ',',
      field('name', alias($._er_style_identifier, $.er_style_name)),
    )),
  ),

  er_style_list: ($) => seq(
    field('style', $.er_style_item),
    repeat(seq(',', field('style', $.er_style_item))),
  ),

  er_style_item: (_) => token(prec(-5, /[^,\r\n]+/)),

  er_quoted_text: (_) => token(prec(10, seq('"', /[^"\r\n]*/, '"'))),

  er_unclosed_quoted_text: (_) => token(prec(-10, seq('"', /[^"\r\n]*/))),

  er_alias_text: (_) => token(prec(-5, /[^\]\r\n]+/)),

  er_role_text: (_) => token(prec(-5, /[^ \t"\r\n][^\r\n]*/)),

  er_line_text: (_) => token(prec(-5, /[^\r\n]+/)),

  _er_entity_name: (_) => token(prec(-1, choice(
    seq('"', /[^"%\\\r\n]+/, '"'),
    /(?:[^\x00-\x7f]|[A-Za-z0-9_.*\-])+/,
  ))),

  _er_attribute_word: (_) => token.immediate(prec(-1, choice(
    seq('`', /[^`\r\n]+/, '`'),
    /[*A-Za-z_\u00c0-\uffff][A-Za-z0-9_\-*\[\]().,~\u00c0-\uffff]*/,
  ))),

  _er_style_identifier: (_) => token(prec(-1, /[A-Za-z0-9_.*\-\u0080-\uffff]+/)),

  er_malformed_statement: ($) => prec(-100, field(
    'text',
    $.er_malformed_text,
  )),

  er_malformed_text: (_) => token(prec(-100, /[^\r\n]+/)),
};

module.exports = {
  entityRelationshipRules,
};
