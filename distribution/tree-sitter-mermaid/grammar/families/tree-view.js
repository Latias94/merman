// Source translation: Mermaid 11.16.1
// packages/parser/src/language/treeView/treeView.langium:26-64 and
// packages/mermaid/src/diagrams/treeView/boxDrawingPreprocessor.ts:67-208
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const { terminatedHeader } = require('../shared/header');

const treeViewIndentation = ($) => choice(
  alias($._tree_view_start, $.tree_view_indentation_start),
  alias($._tree_view_indent, $.tree_view_indentation_indent),
  alias($._tree_view_reindent, $.tree_view_indentation_reindent),
  alias($._tree_view_dedent, $.tree_view_indentation_dedent),
  $.tree_view_indentation_overflow,
);

const trailingTrivia = ($) => optional(field(
  'trivia',
  choice($.comment, $.directive),
));

const treeViewRules = {
  tree_view_diagram: ($) => choice(
    seq(
      field('header', $.tree_view_header),
      optional(field('body', $.tree_view_body)),
    ),
    field('header', alias($._tree_view_header_eof, $.tree_view_header)),
  ),

  tree_view_header: ($) => terminatedHeader(
    $,
    token(prec(20, 'treeView-beta')),
  ),

  _tree_view_header_eof: ($) => field(
    'keyword',
    alias(token(prec(20, 'treeView-beta')), $.diagram_keyword),
  ),

  tree_view_body: ($) => choice(
    repeat1($._tree_view_line_item),
    seq(
      repeat($._tree_view_line_item),
      $._tree_view_eof_item,
    ),
  ),

  _tree_view_line_item: ($) => choice(
    seq(
      field('statement', $._tree_view_statement),
      trailingTrivia($),
      $._line_ending,
    ),
    seq(field('trivia', choice($.comment, $.directive)), $._line_ending),
    $._blank_line,
  ),

  _tree_view_eof_item: ($) => choice(
    seq(field('statement', $._tree_view_statement), trailingTrivia($)),
    $.comment,
    $.directive,
  ),

  _tree_view_statement: ($) => choice(
    $.langium_title_statement,
    $.langium_acc_title_statement,
    $.langium_acc_descr_statement,
    $.tree_view_box_statement,
    $.tree_view_incomplete_box_statement,
    $.tree_view_decoration_statement,
    $.tree_view_node_statement,
  ),

  tree_view_node_statement: ($) => prec.right(20, seq(
    optional(field('indentation', treeViewIndentation($))),
    field('node', $.tree_view_node),
  )),

  tree_view_box_statement: ($) => prec(40, seq(
    field('prefix', $.tree_view_box_prefix),
    field('node', $.tree_view_node),
  )),

  tree_view_incomplete_box_statement: ($) => prec(-30, seq(
    field('prefix', $.tree_view_box_prefix),
  )),

  tree_view_decoration_statement: ($) => field(
    'decoration',
    $.tree_view_box_decoration,
  ),

  tree_view_node: ($) => prec.right(seq(
    field('name', choice(
      $.tree_view_quoted_name,
      $.tree_view_unclosed_name,
      $.tree_view_bare_name,
    )),
    repeat(field('annotation', choice(
      $.tree_view_class_annotation,
      $.tree_view_incomplete_class_annotation,
      $.tree_view_icon_annotation,
      $.tree_view_incomplete_icon_annotation,
      $.tree_view_description_annotation,
    ))),
  )),

  tree_view_class_annotation: ($) => prec(30, seq(
    field('marker', $.tree_view_class_marker),
    field('name', $.tree_view_class_name),
  )),

  tree_view_incomplete_class_annotation: ($) => prec(-20, seq(
    field('marker', $.tree_view_class_marker),
  )),

  tree_view_icon_annotation: ($) => prec(30, seq(
    field('open', $.tree_view_icon_open),
    optional(field('name', $.tree_view_icon_name)),
    field('close', token.immediate(')')),
  )),

  tree_view_incomplete_icon_annotation: ($) => prec(-20, seq(
    field('open', $.tree_view_icon_open),
    optional(field('name', $.tree_view_icon_name)),
  )),

  tree_view_description_annotation: ($) => prec.right(seq(
    field('marker', $.tree_view_description_marker),
    optional(field('text', $.tree_view_description_text)),
  )),

  // The prefix follows Mermaid's preprocessor rather than assuming one fixed
  // dash count. Decoration-only rows remain explicit trivia instead of a
  // whole-line recovery fallback.
  tree_view_box_prefix: (_) => token(prec(
    60,
    /[│┃| \t]*[├└┣┗][─━-]*[ \t]*/,
  )),

  tree_view_box_decoration: (_) => token(prec(
    50,
    /[│┃| \t]*[│┃|][│┃| \t]*/,
  )),

  tree_view_quoted_name: (_) => token(prec(30, choice(
    seq('"', /(?:[^"\\\r\n]|\\.)*/, '"'),
    seq("'", /(?:[^'\\\r\n]|\\.)*/, "'"),
  ))),

  tree_view_unclosed_name: (_) => token(prec(-20, choice(
    seq('"', /(?:[^"\\\r\n]|\\.)*/),
    seq("'", /(?:[^'\\\r\n]|\\.)*/),
  ))),

  // Split punctuation into low-precedence atoms so a high-precedence marker
  // can end a spaced bare name without excluding ordinary ':'/'#'/'()' from
  // filenames. The parent node retains the exact inter-atom whitespace span.
  tree_view_bare_name: ($) => prec.right(-10, repeat1(
    $._tree_view_name_atom,
  )),

  _tree_view_name_atom: (_) => choice(
    token(prec(-20, /[^ \t\f\u00a0\r\n"'%:#()]+/)),
    '%',
    ':',
    '#',
    '(',
    ')',
  ),

  tree_view_class_marker: (_) => token(prec(70, ':::')),

  tree_view_class_name: (_) => token(/[A-Za-z_][A-Za-z0-9_-]*/),

  tree_view_icon_open: (_) => token(prec(70, 'icon(')),

  tree_view_icon_name: (_) => token.immediate(
    /[A-Za-z0-9_-]+(?::[A-Za-z0-9_-]+)?/,
  ),

  tree_view_description_marker: (_) => token(prec(70, '##')),

  tree_view_description_text: (_) => token(
    /[^ \t\f\u00a0\r\n](?:[^\r\n]*[^ \t\f\u00a0\r\n])?/,
  ),
};

module.exports = { treeViewRules };
