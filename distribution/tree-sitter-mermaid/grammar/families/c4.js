// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/c4/parser/c4Diagram.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(40, choice(
    'C4Deployment',
    'C4Component',
    'C4Container',
    'C4Dynamic',
    'C4Context',
  ))), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(40, keyword)), $.c4_statement_keyword),
);

const argumentTail = ($) => repeat(seq(
  field('delimiter', ','),
  optional(field('argument', $.c4_argument)),
));

const callDelimiters = ($, body) => seq(
  field('open', '('),
  body,
  field('close', ')'),
);

const c4Rules = {
  c4_diagram: ($) => seq(
    field('header', $.c4_header),
    field('body', $.c4_body),
  ),

  c4_header: ($) => prec(50, seq(
    diagramKeyword($),
    optional(token.immediate(/[ \t]+/)),
    optional(choice($.comment, $.directive)),
    field('terminator', $._line_ending),
  )),

  c4_body: ($) => choice(
    repeat1($._c4_line_item),
    seq(
      repeat($._c4_line_item),
      $._c4_eof_item,
    ),
  ),

  _c4_line_item: ($) => choice(
    seq(
      choice($._c4_statement, $.c4_boundary_statement),
      optional(choice($.comment, $.directive)),
      $._line_ending,
    ),
    seq(choice($.comment, $.directive), $._line_ending),
    $._blank_line,
  ),

  _c4_eof_item: ($) => choice(
    seq(
      choice($._c4_statement, $.c4_boundary_statement),
      optional(choice($.comment, $.directive)),
    ),
    $.comment,
    $.directive,
  ),

  _c4_statement: ($) => choice(
    $.c4_title_statement,
    $.c4_accessibility_description_statement,
    $.c4_accessibility_title_statement,
    $.c4_direction_statement,
    $.c4_entity_declaration,
    $.c4_relationship_statement,
    $.c4_style_update_statement,
    $.c4_incomplete_statement,
  ),

  c4_title_statement: ($) => prec(50, seq(
    statementKeyword($, 'title'),
    token.immediate(/[ \t]+/),
    field('text', $.c4_line_text),
  )),

  c4_accessibility_title_statement: ($) => prec(50, seq(
    statementKeyword($, 'accTitle'),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.c4_line_text)),
  )),

  c4_accessibility_description_statement: ($) => prec(50, choice(
    seq(
      statementKeyword($, 'accDescription'),
      token.immediate(/[ \t]+/),
      field('text', $.c4_line_text),
    ),
    seq(
      statementKeyword($, 'accDescr'),
      optional(token.immediate(/[ \t]+/)),
      choice(
        seq(
          field('delimiter', token.immediate(':')),
          optional(token.immediate(/[ \t]+/)),
          optional(field('text', $.c4_line_text)),
        ),
        field('description', choice(
          $.c4_accessibility_description_block,
          $.c4_unclosed_accessibility_description_block,
        )),
      ),
    ),
  )),

  c4_accessibility_description_block: (_) => token(seq('{', /[^}]*/, '}')),

  c4_unclosed_accessibility_description_block: (_) => token(prec(
    -10,
    seq('{', /[^}\r\n]*/),
  )),

  c4_direction_statement: ($) => prec(50, seq(
    statementKeyword($, 'direction'),
    token.immediate(/[ \t]+/),
    field('direction', $.c4_direction),
  )),

  c4_direction: (_) => choice('TB', 'BT', 'RL', 'LR'),

  c4_entity_declaration: ($) => prec(50, seq(
    field('kind', $.c4_entity_kind),
    callDelimiters($, seq(
      field('id', $.c4_reference),
      argumentTail($),
    )),
  )),

  c4_entity_kind: (_) => token(prec(40, choice(
    'ComponentQueue_Ext',
    'ComponentDb_Ext',
    'ComponentQueue',
    'Component_Ext',
    'ComponentDb',
    'Component',
    'ContainerQueue_Ext',
    'ContainerDb_Ext',
    'ContainerQueue',
    'Container_Ext',
    'ContainerDb',
    'Container',
    'SystemQueue_Ext',
    'SystemDb_Ext',
    'SystemQueue',
    'System_Ext',
    'SystemDb',
    'System',
    'Person_Ext',
    'Person',
  ))),

  c4_boundary_statement: ($) => prec.right(60, seq(
    field('kind', $.c4_boundary_kind),
    callDelimiters($, seq(
      field('id', $.c4_reference),
      argumentTail($),
    )),
    choice(
      seq(
        field('open', '{'),
        $._line_ending,
      ),
      seq(
        $._line_ending,
        field('open', '{'),
        optional($._line_ending),
      ),
    ),
    optional(field('body', $.c4_boundary_body)),
    field('close', '}'),
  )),

  c4_boundary_body: ($) => repeat1($._c4_line_item),

  c4_boundary_kind: (_) => token(prec(40, choice(
    'Enterprise_Boundary',
    'Container_Boundary',
    'System_Boundary',
    'Deployment_Node',
    'Boundary',
    'Node_L',
    'Node_R',
    'Node',
  ))),

  c4_relationship_statement: ($) => prec(50, choice(
    seq(
      field('kind', $.c4_relationship_kind),
      callDelimiters($, seq(
        field('source', $.c4_reference),
        field('delimiter', ','),
        field('target', $.c4_reference),
        argumentTail($),
      )),
    ),
    seq(
      field('kind', alias('RelIndex', $.c4_relationship_kind)),
      callDelimiters($, seq(
        field('index', $.c4_argument),
        field('delimiter', ','),
        field('source', $.c4_reference),
        field('delimiter', ','),
        field('target', $.c4_reference),
        argumentTail($),
      )),
    ),
  )),

  c4_relationship_kind: (_) => token(prec(40, choice(
    'Rel_Right',
    'Rel_Left',
    'Rel_Down',
    'Rel_Back',
    'Rel_Up',
    'Rel_R',
    'Rel_L',
    'Rel_D',
    'Rel_U',
    'BiRel',
    'Rel',
  ))),

  c4_style_update_statement: ($) => prec(50, choice(
    seq(
      field('kind', alias('UpdateElementStyle', $.c4_update_kind)),
      callDelimiters($, seq(
        field('target', $.c4_reference),
        argumentTail($),
      )),
    ),
    seq(
      field('kind', alias('UpdateRelStyle', $.c4_update_kind)),
      callDelimiters($, seq(
        field('source', $.c4_reference),
        field('delimiter', ','),
        field('target', $.c4_reference),
        argumentTail($),
      )),
    ),
    seq(
      field('kind', alias('UpdateLayoutConfig', $.c4_update_kind)),
      callDelimiters($, optional(seq(
        field('argument', $.c4_argument),
        argumentTail($),
      ))),
    ),
  )),

  // These alternatives mirror the known macro signatures but intentionally
  // stop before `)`. They preserve fields typed so far and synchronize at the
  // physical line instead of asking error recovery to insert a delimiter.
  c4_incomplete_statement: ($) => prec(-30, choice(
    seq(
      field('kind', choice($.c4_entity_kind, $.c4_boundary_kind)),
      field('open', '('),
      optional(seq(
        field('id', $.c4_reference),
        argumentTail($),
      )),
    ),
    seq(
      field('kind', $.c4_relationship_kind),
      field('open', '('),
      optional(seq(
        field('source', $.c4_reference),
        optional(seq(
          field('delimiter', ','),
          optional(field('target', $.c4_reference)),
          argumentTail($),
        )),
      )),
    ),
    seq(
      field('kind', alias('RelIndex', $.c4_relationship_kind)),
      field('open', '('),
      optional(seq(
        field('index', $.c4_argument),
        optional(seq(
          field('delimiter', ','),
          optional(field('source', $.c4_reference)),
          optional(seq(
            field('delimiter', ','),
            optional(field('target', $.c4_reference)),
            argumentTail($),
          )),
        )),
      )),
    ),
    seq(
      field('kind', alias('UpdateElementStyle', $.c4_update_kind)),
      field('open', '('),
      optional(seq(
        field('target', $.c4_reference),
        argumentTail($),
      )),
    ),
    seq(
      field('kind', alias('UpdateRelStyle', $.c4_update_kind)),
      field('open', '('),
      optional(seq(
        field('source', $.c4_reference),
        optional(seq(
          field('delimiter', ','),
          optional(field('target', $.c4_reference)),
          argumentTail($),
        )),
      )),
    ),
    seq(
      field('kind', alias('UpdateLayoutConfig', $.c4_update_kind)),
      field('open', '('),
      optional(seq(
        field('argument', $.c4_argument),
        argumentTail($),
      )),
    ),
    field('kind', choice(
      $.c4_entity_kind,
      $.c4_boundary_kind,
      $.c4_relationship_kind,
      $.c4_update_kind,
      alias('RelIndex', $.c4_relationship_kind),
    )),
  )),

  c4_argument: ($) => field('value', choice(
    $.c4_named_argument,
    $.c4_string,
    $.c4_unclosed_string,
    $.c4_unquoted_argument,
  )),

  c4_named_argument: ($) => seq(
    field('sigil', '$'),
    field('name', $.c4_property_name),
    field('operator', '='),
    field('value', choice($.c4_string, $.c4_unclosed_string)),
  ),

  c4_reference: ($) => field('value', choice(
    $.c4_identifier,
    $.c4_string,
  )),

  c4_property_name: (_) => token.immediate(/[A-Za-z_][A-Za-z0-9_]*/),

  c4_identifier: (_) => token(prec(
    5,
    /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_.:\u00c0-\uffff-]*/,
  )),

  c4_string: (_) => token(prec(10, seq(
    '"',
    /(?:[^"\\\r\n]|\\.)*/,
    '"',
  ))),

  c4_unclosed_string: (_) => token(prec(-10, seq(
    '"',
    /(?:[^"\\\r\n]|\\.)*/,
  ))),

  c4_unquoted_argument: (_) => token(prec(
    -5,
    /[^\s,)"$](?:[^,\r\n)]*[^\s,\r\n)])?/,
  )),

  c4_line_text: (_) => token(prec(
    -5,
    /[^%;\r\n](?:[^;\r\n]|%[^%\r\n])*/,
  )),

  c4_update_kind: (_) => token(prec(40, choice(
    'UpdateElementStyle',
    'UpdateRelStyle',
    'UpdateLayoutConfig',
  ))),
};

module.exports = { c4Rules };
