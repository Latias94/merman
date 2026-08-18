// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/xychart/parser/xychart.jison and
// packages/mermaid/src/diagrams/xychart/parser/xychart.jison.spec.ts at
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const caseVariants = (value) => [...value].reduce(
  (variants, character) => {
    const lower = character.toLowerCase();
    const upper = character.toUpperCase();
    if (lower === upper) return variants.map((prefix) => prefix + character);
    return variants.flatMap((prefix) => [prefix + lower, prefix + upper]);
  },
  [''],
);

const caseInsensitiveChoice = (...values) => choice(
  ...values.flatMap(caseVariants),
);

const diagramKeyword = ($) => field(
  'keyword',
  alias(caseInsensitiveChoice('xychart'), $.diagram_keyword),
);

const statementKeyword = ($, ...keywords) => field(
  'keyword',
  alias(caseInsensitiveChoice(...keywords), $.statement_keyword),
);

const seriesStatement = ($, keyword) => prec.right(30, seq(
  statementKeyword($, keyword),
  $._xy_chart_inline_space,
  optional(field('title', $.xy_chart_text)),
  field('data', $.xy_chart_series_array),
));

const xyChartRules = {
  xy_chart_diagram: ($) => choice(
    seq(
      field('header', $.xy_chart_header),
      optional(field('comment', choice($.comment, $.directive))),
      field('terminator', $._line_ending),
      optional(field('body', $.xy_chart_body)),
    ),
    seq(
      field('header', $.xy_chart_header),
      field(
        'terminator',
        alias(';', $.xy_chart_statement_delimiter),
      ),
      optional(field('body', $.xy_chart_body)),
    ),
    prec(30, seq(
      field('header', $.xy_chart_header),
      $._xy_chart_trailing_space,
      optional(field('comment', choice($.comment, $.directive))),
    )),
    seq(
      field('header', $.xy_chart_header),
      optional(field('comment', choice($.comment, $.directive))),
    ),
  ),

  xy_chart_header: ($) => seq(
    diagramKeyword($),
    optional(field(
      'beta',
      alias(token.immediate(/-beta/i), $.xy_chart_beta_marker),
    )),
    optional(seq(
      $._xy_chart_inline_space,
      field('orientation', $.xy_chart_orientation),
    )),
  ),

  xy_chart_orientation: (_) => token.immediate(/(?:vertical|horizontal)/i),

  xy_chart_body: ($) => choice(
    repeat1($._xy_chart_terminated_body_item),
    seq(
      repeat($._xy_chart_terminated_body_item),
      $._xy_chart_eof_body_item,
    ),
  ),

  _xy_chart_terminated_body_item: ($) => choice(
    $._blank_line,
    seq(choice($.comment, $.directive), $._line_ending),
    seq(
      $._xy_chart_statement,
      optional(choice($.comment, $.directive)),
      $._line_ending,
    ),
    seq(
      $._xy_chart_statement,
      alias(';', $.xy_chart_statement_delimiter),
    ),
  ),

  _xy_chart_eof_body_item: ($) => choice(
    seq(
      $._xy_chart_statement,
      optional(choice($.comment, $.directive)),
    ),
    $.comment,
    $.directive,
  ),

  _xy_chart_statement: ($) => choice(
    $.xy_chart_malformed_statement_sequence,
    $._xy_chart_complete_statement,
    $.xy_chart_incomplete_x_axis_statement,
    $.xy_chart_incomplete_y_axis_statement,
    $.xy_chart_malformed_y_axis_categories_statement,
    $.xy_chart_malformed_title_statement,
    $.xy_chart_malformed_axis_statement,
    $.xy_chart_malformed_series_statement,
    $.xy_chart_malformed_accessibility_statement,
    $.xy_chart_malformed_statement,
  ),

  _xy_chart_complete_statement: ($) => choice(
    $.xy_chart_title_statement,
    $.xy_chart_x_axis_statement,
    $.xy_chart_y_axis_statement,
    $.xy_chart_line_statement,
    $.xy_chart_bar_statement,
    $.xy_chart_accessibility_title_statement,
    $.xy_chart_accessibility_description_statement,
  ),

  xy_chart_malformed_statement_sequence: ($) => prec.right(-40, seq(
    field('statement', choice(
      $.xy_chart_line_statement,
      $.xy_chart_bar_statement,
    )),
    field('trailing', $.xy_chart_trailing_text),
  )),

  xy_chart_title_statement: ($) => prec.right(30, seq(
    statementKeyword($, 'title'),
    $._xy_chart_inline_space,
    field('title', $.xy_chart_text),
  )),

  xy_chart_x_axis_statement: ($) => prec.right(30, seq(
    statementKeyword($, 'x-axis'),
    $._xy_chart_inline_space,
    choice(
      seq(
        optional(field('title', $.xy_chart_text)),
        field('categories', $.xy_chart_category_array),
      ),
      seq(
        optional(field('title', $.xy_chart_text)),
        field('range', $.xy_chart_axis_range),
      ),
      field('title', $.xy_chart_text),
    ),
  )),

  xy_chart_y_axis_statement: ($) => prec.right(30, seq(
    statementKeyword($, 'y-axis'),
    $._xy_chart_inline_space,
    choice(
      seq(
        optional(field('title', $.xy_chart_text)),
        field('range', $.xy_chart_axis_range),
      ),
      field('title', $.xy_chart_text),
    ),
  )),

  xy_chart_line_statement: ($) => seriesStatement(
    $,
    'line',
  ),

  xy_chart_bar_statement: ($) => seriesStatement(
    $,
    'bar',
  ),

  xy_chart_axis_range: ($) => seq(
    field('minimum', $.xy_chart_number),
    field(
      'delimiter',
      alias('-->', $.xy_chart_range_delimiter),
    ),
    field('maximum', $.xy_chart_number),
  ),

  xy_chart_category_array: ($) => seq(
    field('open', alias('[', $.xy_chart_array_open)),
    field('category', $.xy_chart_text),
    repeat(seq(
      field('delimiter', alias(',', $.xy_chart_array_delimiter)),
      field('category', $.xy_chart_text),
    )),
    field('close', alias(']', $.xy_chart_array_close)),
  ),

  xy_chart_series_array: ($) => seq(
    field('open', alias('[', $.xy_chart_array_open)),
    field('point', $.xy_chart_data_point),
    repeat(seq(
      field('delimiter', alias(',', $.xy_chart_array_delimiter)),
      field('point', $.xy_chart_data_point),
    )),
    field('close', alias(']', $.xy_chart_array_close)),
  ),

  xy_chart_data_point: ($) => seq(
    field('value', $.xy_chart_number),
    optional(field('label', $.xy_chart_quoted_text)),
  ),

  xy_chart_unclosed_category_array: ($) => prec.dynamic(-10, seq(
    field('open', alias('[', $.xy_chart_array_open)),
    field('category', $.xy_chart_text),
    repeat(seq(
      field('delimiter', alias(',', $.xy_chart_array_delimiter)),
      field('category', $.xy_chart_text),
    )),
  )),

  xy_chart_unclosed_series_array: ($) => prec.dynamic(-10, seq(
    field('open', alias('[', $.xy_chart_array_open)),
    field('point', $.xy_chart_data_point),
    repeat(seq(
      field('delimiter', alias(',', $.xy_chart_array_delimiter)),
      field('point', $.xy_chart_data_point),
    )),
  )),

  xy_chart_empty_series_array: ($) => seq(
    field('open', alias('[', $.xy_chart_array_open)),
    field('close', alias(']', $.xy_chart_array_close)),
  ),

  xy_chart_incomplete_axis_range: ($) => seq(
    field('minimum', $.xy_chart_number),
    field(
      'delimiter',
      alias('-->', $.xy_chart_range_delimiter),
    ),
  ),

  xy_chart_incomplete_x_axis_statement: ($) => prec.right(-20, seq(
    statementKeyword($, 'x-axis'),
    $._xy_chart_inline_space,
    choice(
      seq(
        optional(field('title', $.xy_chart_text)),
        field('categories', $.xy_chart_unclosed_category_array),
      ),
      seq(
        optional(field('title', $.xy_chart_text)),
        field('range', $.xy_chart_incomplete_axis_range),
      ),
    ),
  )),

  xy_chart_incomplete_y_axis_statement: ($) => prec.right(-20, seq(
    statementKeyword($, 'y-axis'),
    $._xy_chart_inline_space,
    optional(field('title', $.xy_chart_text)),
    field('range', $.xy_chart_incomplete_axis_range),
  )),

  xy_chart_malformed_y_axis_categories_statement: ($) => prec.right(-20, seq(
    statementKeyword($, 'y-axis'),
    $._xy_chart_inline_space,
    optional(field('title', $.xy_chart_text)),
    field('categories', $.xy_chart_category_array),
  )),

  xy_chart_accessibility_title_statement: ($) => prec.right(30, seq(
    statementKeyword($, 'accTitle'),
    optional($._xy_chart_inline_space),
    field(
      'delimiter',
      alias(token.immediate(':'), $.xy_chart_accessibility_delimiter),
    ),
    optional($._xy_chart_inline_space),
    optional(field('text', $.xy_chart_accessibility_text)),
  )),

  xy_chart_accessibility_description_statement: ($) => prec.right(30, seq(
    statementKeyword($, 'accDescr'),
    optional($._xy_chart_inline_space),
    choice(
      seq(
        field(
          'delimiter',
          alias(token.immediate(':'), $.xy_chart_accessibility_delimiter),
        ),
        optional($._xy_chart_inline_space),
        optional(field('text', $.xy_chart_accessibility_text)),
      ),
      field(
        'description',
        choice(
          $.xy_chart_accessibility_description_block,
          $.xy_chart_unclosed_accessibility_description_block,
        ),
      ),
    ),
  )),

  xy_chart_accessibility_description_block: ($) => field(
    'text',
    $.xy_chart_accessibility_block_text,
  ),

  xy_chart_unclosed_accessibility_description_block: ($) => field(
    'text',
    $.xy_chart_unclosed_accessibility_block_text,
  ),

  xy_chart_text: ($) => choice(
    $.xy_chart_markdown_text,
    $.xy_chart_quoted_text,
    $.xy_chart_unclosed_markdown_text,
    $.xy_chart_unclosed_quoted_text,
    $.xy_chart_bare_text,
  ),

  xy_chart_quoted_text: ($) => prec.dynamic(10, seq(
    field('open', alias('"', $.xy_chart_quote_delimiter)),
    optional(field('content', $.xy_chart_quoted_content)),
    field(
      'close',
      alias(token.immediate('"'), $.xy_chart_quote_delimiter),
    ),
  )),

  xy_chart_unclosed_quoted_text: ($) => prec.dynamic(-10, seq(
    field('open', alias('"', $.xy_chart_quote_delimiter)),
    optional(field('content', $.xy_chart_quoted_content)),
  )),

  xy_chart_markdown_text: ($) => prec.dynamic(10, seq(
    field('open', alias('"`', $.xy_chart_markdown_delimiter)),
    repeat(choice(
      field('content', $.xy_chart_markdown_content),
      field('content', $.xy_chart_markdown_backtick_content),
    )),
    field(
      'close',
      alias(token.immediate('`"'), $.xy_chart_markdown_delimiter),
    ),
  )),

  xy_chart_unclosed_markdown_text: ($) => prec.dynamic(-10, seq(
    field('open', alias('"`', $.xy_chart_markdown_delimiter)),
    repeat(choice(
      field('content', $.xy_chart_markdown_content),
      field('content', $.xy_chart_markdown_backtick_content),
    )),
  )),

  xy_chart_bare_text: ($) => repeat1(field('part', choice(
    $.xy_chart_text_atom,
    $.xy_chart_number,
  ))),

  xy_chart_number: (_) => token(prec(
    10,
    /[+-]?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)/,
  )),

  xy_chart_text_atom: (_) => token(prec(-1, /[A-Za-z&=*.#_+\-]+/)),

  xy_chart_quoted_content: (_) => token.immediate(/[^"\r\n]+/),

  xy_chart_markdown_content: (_) => token.immediate(/[^`\r\n]+/),

  xy_chart_markdown_backtick_content: (_) => token.immediate(/`[^"\r\n]/),

  xy_chart_accessibility_text: (_) => token.immediate(prec(
    5,
    /[^\s%\r\n](?:(?:[^%\r\n]|%[^%\r\n])*[^\s%\r\n])?/,
  )),

  xy_chart_accessibility_block_text: (_) => token(prec(
    10,
    seq('{', /[^}]*/, '}'),
  )),

  xy_chart_unclosed_accessibility_block_text: (_) => token(prec(
    -10,
    seq('{', /[^}\r\n]*/),
  )),

  xy_chart_malformed_title_statement: ($) => prec.right(-50, seq(
    statementKeyword($, 'title'),
  )),

  xy_chart_malformed_axis_statement: ($) => prec.right(-50, seq(
    statementKeyword($, 'x-axis', 'y-axis'),
  )),

  xy_chart_malformed_series_statement: ($) => prec.right(-50, seq(
    statementKeyword($, 'line', 'bar'),
    optional(seq(
      $._xy_chart_inline_space,
      choice(
        field('title', $.xy_chart_text),
        seq(
          optional(field('title', $.xy_chart_text)),
          field('data', $.xy_chart_unclosed_series_array),
        ),
        seq(
          optional(field('title', $.xy_chart_text)),
          field('data', $.xy_chart_empty_series_array),
        ),
      ),
    )),
  )),

  xy_chart_malformed_accessibility_statement: ($) => prec.right(-50, seq(
    statementKeyword($, 'accTitle', 'accDescr'),
    optional(seq(
      $._xy_chart_inline_space,
      optional(field('text', $.xy_chart_text)),
    )),
  )),

  xy_chart_malformed_statement: ($) => prec.right(-100, choice(
    seq(
      field('keyword', alias($.identifier, $.xy_chart_unknown_keyword)),
      optional(field('text', $.xy_chart_unknown_tail)),
    ),
    field('text', $.xy_chart_malformed_text),
  )),

  xy_chart_unknown_tail: (_) => token(prec(-100, /[^;\r\n]+/)),

  xy_chart_trailing_text: (_) => token.immediate(prec(
    -100,
    /[ \t]+[^ \t%;\r\n][^;\r\n]*/,
  )),

  xy_chart_malformed_text: (_) => token(prec(
    -100,
    /[^%;\r\n][^;\r\n]*/,
  )),

  _xy_chart_inline_space: (_) => token.immediate(/[ \t]+/),

  _xy_chart_trailing_space: (_) => token.immediate(/[ \t]+/),
};

const xyChartConflicts = ($) => [
  [$.xy_chart_header],
  [$.xy_chart_bare_text],
];

module.exports = { xyChartConflicts, xyChartRules };
