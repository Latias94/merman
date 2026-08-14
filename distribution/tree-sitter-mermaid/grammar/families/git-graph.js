// Translated from Mermaid 11.16.1 at commit
// 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf:
// packages/parser/src/language/gitGraph/{gitGraph,reference}.langium.

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(keyword, $.git_graph_statement_keyword),
);

const clauseKeyword = ($, keyword) => field(
  'keyword',
  alias(keyword, $.git_graph_clause_keyword),
);

const clauseSeparator = ($) => field(
  'separator',
  alias(token.immediate(':'), $.git_graph_clause_separator),
);

const quotedClause = ($, keyword, fieldName) => seq(
  clauseKeyword($, keyword),
  clauseSeparator($),
  optional($._langium_inline_space),
  field(fieldName, $.langium_string),
);

const gitGraphRules = {
  git_graph_diagram: ($) => choice(
    seq(
      field('header', alias($._git_graph_colon_header, $.git_graph_header)),
      optional(field('body', $.git_graph_body)),
    ),
    seq(
      field('header', $.git_graph_header),
      optional(seq(
        $._langium_body_boundary,
        optional(field('body', $.git_graph_body)),
      )),
    ),
  ),

  git_graph_header: ($) => field(
    'keyword',
    alias(token(prec(20, 'gitGraph')), $.diagram_keyword),
  ),

  _git_graph_colon_header: ($) => seq(
    field(
      'keyword',
      alias(token(prec(20, 'gitGraph')), $.diagram_keyword),
    ),
    choice(
      field(
        'separator',
        alias(token.immediate(':'), $.git_graph_header_separator),
      ),
      seq(
        $._langium_inline_space,
        choice(
          field(
            'separator',
            alias(token.immediate(':'), $.git_graph_header_separator),
          ),
          seq(
            field(
              'direction',
              alias(token.immediate(/(?:LR|TB|BT)/), $.git_graph_direction),
            ),
            optional($._langium_inline_space),
            field(
              'separator',
              alias(token.immediate(':'), $.git_graph_header_separator),
            ),
          ),
        ),
      ),
    ),
  ),

  git_graph_body: ($) => choice(
    repeat1($._git_graph_terminated_body_item),
    seq(
      repeat($._git_graph_terminated_body_item),
      $._git_graph_eof_body_item,
    ),
  ),

  _git_graph_terminated_body_item: ($) => choice(
    $._langium_newline,
    seq($.comment, $._langium_newline),
    seq($.directive, $._langium_newline),
    $.langium_title_statement,
    $.langium_acc_title_statement,
    $.langium_acc_descr_statement,
    seq(
      $._git_graph_statement,
      optional(field('comment', $.comment)),
      $._langium_newline,
    ),
  ),

  _git_graph_eof_body_item: ($) => choice(
    $.comment,
    $.directive,
    seq(
      $._git_graph_statement,
      optional(field('comment', $.comment)),
    ),
  ),

  _git_graph_statement: ($) => choice(
    $.git_graph_commit_statement,
    $.git_graph_branch_statement,
    $.git_graph_incomplete_branch_statement,
    $.git_graph_merge_statement,
    $.git_graph_incomplete_merge_statement,
    $.git_graph_checkout_statement,
    $.git_graph_incomplete_checkout_statement,
    $.git_graph_cherry_pick_statement,
    $.git_graph_malformed_statement,
  ),

  git_graph_commit_statement: ($) => prec.right(seq(
    statementKeyword($, 'commit'),
    repeat(choice(
      $.git_graph_id_clause,
      $.git_graph_message_clause,
      $.git_graph_tag_clause,
      $.git_graph_type_clause,
    )),
  )),

  git_graph_branch_statement: ($) => prec.right(seq(
    statementKeyword($, 'branch'),
    field('name', choice($.git_graph_reference, $.langium_string)),
    optional(field('order', $.git_graph_order_clause)),
  )),

  git_graph_incomplete_branch_statement: ($) => prec(
    -10,
    statementKeyword($, 'branch'),
  ),

  git_graph_merge_statement: ($) => prec.right(seq(
    statementKeyword($, 'merge'),
    field('branch', choice($.git_graph_reference, $.langium_string)),
    repeat(choice(
      $.git_graph_id_clause,
      $.git_graph_tag_clause,
      $.git_graph_type_clause,
    )),
  )),

  git_graph_incomplete_merge_statement: ($) => prec(
    -10,
    statementKeyword($, 'merge'),
  ),

  git_graph_checkout_statement: ($) => prec.right(seq(
    field(
      'keyword',
      alias(choice('checkout', 'switch'), $.git_graph_statement_keyword),
    ),
    field('branch', choice($.git_graph_reference, $.langium_string)),
  )),

  git_graph_incomplete_checkout_statement: ($) => prec(-10, field(
    'keyword',
    alias(choice('checkout', 'switch'), $.git_graph_statement_keyword),
  )),

  git_graph_cherry_pick_statement: ($) => prec.right(seq(
    statementKeyword($, 'cherry-pick'),
    repeat(choice(
      $.git_graph_id_clause,
      $.git_graph_tag_clause,
      $.git_graph_parent_clause,
    )),
  )),

  // This recovery node is restricted to one physical line and has lower
  // lexical precedence than every valid GitGraph keyword. It preserves later
  // siblings without becoming a whole-body or valid-statement fallback.
  git_graph_malformed_statement: ($) => prec(-100, field(
    'text',
    $.git_graph_malformed_text,
  )),

  git_graph_id_clause: ($) => quotedClause($, 'id', 'id'),

  git_graph_message_clause: ($) => choice(
    quotedClause($, 'msg', 'message'),
    field('message', $.langium_string),
  ),

  git_graph_tag_clause: ($) => quotedClause($, 'tag', 'tag'),

  git_graph_parent_clause: ($) => quotedClause($, 'parent', 'parent'),

  git_graph_type_clause: ($) => seq(
    clauseKeyword($, 'type'),
    clauseSeparator($),
    optional($._langium_inline_space),
    field('type', $.git_graph_commit_type),
  ),

  git_graph_order_clause: ($) => seq(
    clauseKeyword($, 'order'),
    clauseSeparator($),
    optional($._langium_inline_space),
    field('value', $.git_graph_integer),
  ),

  git_graph_commit_type: (_) => choice('NORMAL', 'REVERSE', 'HIGHLIGHT'),

  git_graph_integer: (_) => token(choice('0', /[1-9][0-9]*/)),

  // Langium's `\\w` is ASCII in this JavaScript grammar. The source rule's
  // final `[-\\w]` deliberately permits a trailing dash, but not `.` or `/`.
  git_graph_reference: (_) => token(prec(
    5,
    /[A-Za-z0-9_](?:[-./A-Za-z0-9_]*[-A-Za-z0-9_])?/,
  )),

  git_graph_malformed_text: (_) => token(prec(-100, /[^\r\n]+/)),
};

const gitGraphConflicts = ($) => [
  [$.git_graph_header, $._git_graph_colon_header],
];

module.exports = { gitGraphConflicts, gitGraphRules };
