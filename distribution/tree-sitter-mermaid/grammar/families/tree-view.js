const { terminatedHeader } = require('../shared/header');
const { indentationTransition } = require('../shared/indentation');

const treeViewRules = {
  tree_view_diagram: ($) => choice(
    seq(
      field('header', $.tree_view_header),
      optional(field('body', $.tree_view_body)),
    ),
    field('header', alias($._tree_view_header_eof, $.tree_view_header)),
  ),

  tree_view_header: ($) => terminatedHeader($, token(prec(20, 'treeView-beta'))),

  _tree_view_header_eof: ($) => field(
    'keyword',
    alias(token(prec(20, 'treeView-beta')), $.diagram_keyword),
  ),

  tree_view_body: ($) => repeat1(choice(
    $.comment,
    $._blank_line,
    $.tree_view_box_statement,
    $.tree_view_node_statement,
    $.tree_view_unstructured_statement,
  )),

  tree_view_node_statement: ($) => prec.right(seq(
    optional(indentationTransition($, 'tree_view')),
    field('node', $.tree_view_node),
    $._line_ending,
  )),

  tree_view_box_statement: ($) => prec.right(seq(
    field('prefix', $.tree_view_box_prefix),
    field('node', $.tree_view_node),
    $._line_ending,
  )),

  tree_view_node: ($) => seq(
    field('name', choice($.quoted_string, $.tree_view_bare_name)),
    repeat(field('annotation', choice(
      $.tree_view_class_annotation,
      $.tree_view_icon_annotation,
      $.tree_view_description_annotation,
    ))),
  ),

  tree_view_class_annotation: ($) => seq(':::', field('name', $.identifier)),

  tree_view_icon_annotation: ($) => seq(
    'icon',
    '(',
    optional(field('name', $.tree_view_icon_name)),
    ')',
  ),

  tree_view_description_annotation: ($) => seq(
    '##',
    field('text', $.tree_view_description_text),
  ),

  tree_view_box_prefix: (_) => token(prec(
    30,
    /(?:[│┃|][ \t]*)*(?:├──|└──|┣━━|┗━━)[ \t]*/,
  )),

  tree_view_bare_name: (_) => token(prec(-10, /[^ \t\r\n"'#:]+/)),

  tree_view_icon_name: (_) => token(/[A-Za-z0-9_-]*(?::[A-Za-z0-9_-]+)?/),

  tree_view_description_text: (_) => token(prec(5, /[^\r\n]+/)),

  tree_view_unstructured_statement: ($) => prec.right(seq(
    alias($.unstructured_line, $.unstructured_body),
    $._line_ending,
  )),
};

module.exports = { treeViewRules };
