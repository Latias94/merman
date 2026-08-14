// Source translation:
// Mermaid 11.16.1 packages/parser/src/language/architecture/{architecture,arch}.langium

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, 'architecture-beta')), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(1, keyword)), $.statement_keyword),
);

const architectureRules = {
  architecture_diagram: ($) => choice(
    seq(
      field('header', $.architecture_header),
      optional(field('body', $.architecture_body)),
    ),
    seq(
      field(
        'header',
        alias($._architecture_inline_header, $.architecture_header),
      ),
      field('body', $.architecture_body),
    ),
    field(
      'header',
      alias($._architecture_header_eof, $.architecture_header),
    ),
  ),

  architecture_header: ($) => seq(
    diagramKeyword($),
    field('terminator', $._line_ending),
  ),

  _architecture_inline_header: ($) => seq(
    diagramKeyword($),
    token.immediate(/[ \t]+/),
  ),

  _architecture_header_eof: ($) => diagramKeyword($),

  architecture_body: ($) => choice(
    repeat1($._architecture_line_item),
    seq(
      repeat($._architecture_line_item),
      $._architecture_eof_item,
    ),
  ),

  _architecture_line_item: ($) => choice(
    seq($._architecture_statement, optional($.comment), $._line_ending),
    seq(choice($.comment, $.directive), $._line_ending),
    $._blank_line,
  ),

  _architecture_eof_item: ($) => choice(
    seq($._architecture_statement, optional($.comment)),
    $.comment,
    $.directive,
  ),

  _architecture_statement: ($) => choice(
    $.architecture_title_statement,
    $.architecture_accessibility_title_statement,
    $.architecture_accessibility_description_statement,
    $.architecture_group_statement,
    $.architecture_service_statement,
    $.architecture_junction_statement,
    $.architecture_alignment_statement,
    $.architecture_edge_statement,
    $.architecture_malformed_edge_statement,
  ),

  architecture_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'title'),
    optional(seq(
      token.immediate(/[ \t]+/),
      optional(field('text', $.architecture_line_text)),
    )),
  )),

  architecture_accessibility_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'accTitle'),
    field('separator', ':'),
    optional(field('text', $.architecture_line_text)),
  )),

  architecture_accessibility_description_statement: ($) => prec.right(choice(
    seq(
      statementKeyword($, 'accDescr'),
      field('block', $.architecture_accessibility_description_block),
    ),
    seq(
      statementKeyword($, 'accDescr'),
      field('separator', ':'),
      optional(field('text', $.architecture_line_text)),
    ),
  )),

  architecture_accessibility_description_block: ($) => seq(
    repeat($._line_ending),
    '{',
    optional(field('text', $.architecture_accessibility_text)),
    '}',
  ),

  architecture_group_statement: ($) => prec(20, seq(
    statementKeyword($, 'group'),
    field('id', $.architecture_identifier),
    optional(field('icon', $.architecture_icon)),
    optional(field('title', choice(
      $.architecture_title,
      $.architecture_unclosed_title,
    ))),
    optional(field('parent', $.architecture_parent_clause)),
  )),

  architecture_service_statement: ($) => prec(20, seq(
    statementKeyword($, 'service'),
    field('id', $.architecture_identifier),
    optional(choice(
      field('icon_text', $.architecture_quoted_string),
      field('icon_text', $.architecture_unclosed_quoted_string),
      field('icon', $.architecture_icon),
    )),
    optional(field('title', choice(
      $.architecture_title,
      $.architecture_unclosed_title,
    ))),
    optional(field('parent', $.architecture_parent_clause)),
  )),

  architecture_junction_statement: ($) => prec(20, seq(
    statementKeyword($, 'junction'),
    field('id', $.architecture_identifier),
    optional(field('parent', $.architecture_parent_clause)),
  )),

  architecture_alignment_statement: ($) => prec(20, seq(
    statementKeyword($, 'align'),
    field('direction', $.architecture_alignment_direction),
    field('member', $.architecture_identifier),
    repeat1(field('member', $.architecture_identifier)),
  )),

  architecture_parent_clause: ($) => seq(
    statementKeyword($, 'in'),
    field('parent', $.architecture_identifier),
  ),

  architecture_edge_statement: ($) => prec(30, seq(
    field('source', $.architecture_edge_endpoint),
    field('source_port', $.architecture_left_port),
    field('arrow', $.architecture_arrow),
    field('target_port', $.architecture_right_port),
    field('target', $.architecture_edge_endpoint),
  )),

  architecture_malformed_edge_statement: ($) => prec(-20, seq(
    field('source', $.architecture_edge_endpoint),
    field('source_port', $.architecture_left_port),
    field('recovery', $.architecture_edge_recovery),
  )),

  architecture_edge_recovery: ($) => choice(
    field('arrow', $.architecture_arrow),
    seq(
      optional(field('arrow', $.architecture_arrow)),
      field('text', $._architecture_edge_recovery_text),
    ),
  ),

  architecture_edge_endpoint: ($) => seq(
    field('id', $.architecture_identifier),
    optional(field('group', $.architecture_group_modifier)),
  ),

  architecture_left_port: ($) => seq(
    field('delimiter', ':'),
    field('direction', $.architecture_port_direction),
  ),

  architecture_right_port: ($) => seq(
    field('direction', $.architecture_port_direction),
    field('delimiter', ':'),
  ),

  architecture_arrow: ($) => seq(
    optional(field('source_arrowhead', $.architecture_arrowhead)),
    field('connector', choice(
      $.architecture_plain_connector,
      $.architecture_titled_connector,
    )),
    optional(field('target_arrowhead', $.architecture_arrowhead)),
  ),

  architecture_plain_connector: (_) => '--',

  architecture_titled_connector: ($) => seq(
    '-',
    field('title', $.architecture_title),
    '-',
  ),

  architecture_arrowhead: (_) => choice('<', '>'),

  architecture_group_modifier: (_) => token('{group}'),

  architecture_port_direction: (_) => choice('L', 'R', 'T', 'B'),

  architecture_alignment_direction: (_) => choice('row', 'column'),

  architecture_icon: ($) => seq(
    '(',
    field('name', $.architecture_icon_name),
    ')',
  ),

  architecture_title: ($) => seq(
    '[',
    field('text', choice(
      $.architecture_quoted_string,
      $.architecture_bare_title,
    )),
    ']',
  ),

  architecture_unclosed_title: ($) => seq(
    '[',
    field('recovery', $.architecture_unclosed_quoted_string),
  ),

  architecture_quoted_string: ($) => prec.dynamic(10, choice(
    seq(
      '"',
      repeat(choice(
        $._architecture_double_quoted_content,
        $._architecture_escape_sequence,
        $._line_ending,
      )),
      '"',
    ),
    seq(
      "'",
      repeat(choice(
        $._architecture_single_quoted_content,
        $._architecture_escape_sequence,
        $._line_ending,
      )),
      "'",
    ),
  )),

  architecture_unclosed_quoted_string: ($) => prec.dynamic(-10, choice(
    seq(
      '"',
      repeat(choice(
        $._architecture_double_quoted_content,
        $._architecture_escape_sequence,
      )),
    ),
    seq(
      "'",
      repeat(choice(
        $._architecture_single_quoted_content,
        $._architecture_escape_sequence,
      )),
    ),
  )),

  architecture_line_text: ($) => repeat1(choice(
    $._architecture_line_text_fragment,
    '%',
  )),

  architecture_identifier: (_) => token(prec(
    1,
    /[A-Za-z0-9_](?:[A-Za-z0-9_-]*[A-Za-z0-9_])?/,
  )),

  architecture_icon_name: (_) => token(/[A-Za-z0-9_:-]+/),

  architecture_bare_title: (_) => token(prec(-1, /[A-Za-z0-9_ ]+/)),

  architecture_accessibility_text: (_) => token(prec(-5, /[^}]+/)),

  _architecture_double_quoted_content: (_) => token.immediate(/[^"\\\r\n]+/),

  _architecture_single_quoted_content: (_) => token.immediate(/[^'\\\r\n]+/),

  _architecture_escape_sequence: (_) => token.immediate(/\\[^\r\n]/),

  _architecture_line_text_fragment: (_) => token(prec(-5, /[^%\r\n]+/)),

  _architecture_edge_recovery_text: (_) => token(prec(-100, /[^\r\n]+/)),
};

const architectureConflicts = ($) => [
  [$.architecture_quoted_string, $.architecture_unclosed_quoted_string],
];

module.exports = { architectureConflicts, architectureRules };
