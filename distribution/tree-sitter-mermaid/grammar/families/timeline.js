// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/timeline/parser/timeline.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, 'timeline')), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.timeline_statement_keyword),
);

const inlineComment = ($) => choice(
  field('comment', $.comment),
  field('directive', $.directive),
  field('comment', $.timeline_hash_comment),
);

const periodValue = ($) => field('period', choice(
  $.timeline_period,
  alias($._timeline_keyword_prefixed_period, $.timeline_period),
));

const timelineRules = {
  timeline_diagram: ($) => choice(
    seq(
      field('header', $.timeline_header),
      optional(field('body', $.timeline_body)),
    ),
    field('header', alias($._timeline_header_eof, $.timeline_header)),
  ),

  timeline_header: ($) => seq(
    diagramKeyword($),
    optional(seq(
      token.immediate(/[ \t]+/),
      field('direction', $.timeline_direction),
    )),
    field('terminator', $._line_ending),
  ),

  _timeline_header_eof: ($) => seq(
    diagramKeyword($),
    optional(seq(
      token.immediate(/[ \t]+/),
      field('direction', $.timeline_direction),
    )),
  ),

  timeline_direction: (_) => token.immediate(prec(20, choice(/LR/i, /TD/i))),

  timeline_body: ($) => choice(
    repeat1($._timeline_line_item),
    seq(
      repeat($._timeline_line_item),
      $._timeline_eof_item,
    ),
  ),

  _timeline_line_item: ($) => choice(
    seq(
      optional(token.immediate(/[ \t]+/)),
      $._timeline_statement,
      optional(inlineComment($)),
      $._line_ending,
    ),
    seq(
      token.immediate(/[ \t]+/),
      $._line_ending,
    ),
    seq(
      choice($.comment, $.directive, $.timeline_hash_comment),
      $._line_ending,
    ),
    $._blank_line,
  ),

  _timeline_eof_item: ($) => choice(
    seq($._timeline_statement, optional(inlineComment($))),
    $.comment,
    $.directive,
    $.timeline_hash_comment,
  ),

  _timeline_statement: ($) => choice(
    $.timeline_title_statement,
    $.timeline_accessibility_title_statement,
    $.timeline_accessibility_description_statement,
    $.timeline_section_statement,
    $.timeline_incomplete_title_statement,
    $.timeline_incomplete_section_statement,
    $.timeline_event_statement,
    $.timeline_malformed_event_statement,
    $.timeline_period_statement,
  ),

  timeline_title_statement: ($) => prec(30, seq(
    statementKeyword($, /title/i),
    token.immediate(/[ \t]+/),
    field('text', $.timeline_line_text),
  )),

  timeline_accessibility_title_statement: ($) => prec(30, seq(
    statementKeyword($, /accTitle/i),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.timeline_line_text)),
  )),

  timeline_accessibility_description_statement: ($) => prec(30, seq(
    statementKeyword($, /accDescr/i),
    optional(token.immediate(/[ \t]+/)),
    choice(
      seq(
        field('delimiter', token.immediate(':')),
        optional(token.immediate(/[ \t]+/)),
        optional(field('text', $.timeline_line_text)),
      ),
      field('description', $.timeline_accessibility_description_block),
      field('description', $.timeline_unclosed_accessibility_block),
    ),
  )),

  timeline_accessibility_description_block: ($) => seq(
    repeat($._line_ending),
    field('text', $.timeline_accessibility_block_text),
  ),

  timeline_accessibility_block_text: (_) => token(seq('{', /[^}]*/, '}')),

  timeline_unclosed_accessibility_block: (_) => token(prec(
    -10,
    seq('{', /[^}\r\n]*/),
  )),

  timeline_section_statement: ($) => prec(30, seq(
    statementKeyword($, /section/i),
    token.immediate(/[ \t]+/),
    field('name', $.timeline_section_name),
  )),

  timeline_incomplete_title_statement: ($) => prec(
    -20,
    statementKeyword($, /title/i),
  ),

  timeline_incomplete_section_statement: ($) => prec(
    -20,
    statementKeyword($, /section/i),
  ),

  timeline_period_statement: ($) => prec.right(seq(
    periodValue($),
    repeat(field('event', $.timeline_event)),
    optional(field('event', $.timeline_incomplete_event)),
  )),

  timeline_event_statement: ($) => prec.right(seq(
    repeat1(field('event', $.timeline_event)),
    optional(field('event', $.timeline_incomplete_event)),
  )),

  timeline_event: ($) => seq(
    field(
      'delimiter',
      alias(token.immediate(':'), $.timeline_event_delimiter),
    ),
    token.immediate(/[ \t]+/),
    field('text', $.timeline_event_text),
  ),

  timeline_incomplete_event: ($) => prec(-10, seq(
    field(
      'delimiter',
      alias(token.immediate(':'), $.timeline_event_delimiter),
    ),
    optional(token.immediate(/[ \t]+/)),
  )),

  timeline_malformed_event_statement: ($) => prec(-20, field(
    'text',
    $.timeline_malformed_event_text,
  )),

  timeline_event_text: ($) => repeat1(choice(
    $._timeline_event_text_fragment,
    $._timeline_event_embedded_colon,
    $._timeline_single_percent,
  )),

  _timeline_event_text_fragment: (_) => token.immediate(/[^:%\r\n]+/),

  _timeline_event_embedded_colon: (_) => token.immediate(
    /:[^ \t\r\n:%][^:%\r\n]*/,
  ),

  _timeline_single_percent: (_) => token.immediate(/%[^%\r\n]/),

  timeline_period: (_) => token(prec(
    5,
    /[^:#%\s\r\n](?:[^:#%\r\n]|%[^%\r\n])*/
  )),

  _timeline_keyword_prefixed_period: (_) => seq(
    token(prec(30, choice(
      seq(/title/i, /[^\s:\r\n]/),
      seq(/accTitle/i, /[^\s:\r\n]/),
      seq(/accDescr/i, /[^\s:\r\n]/),
      seq(/section/i, /[^\s:\r\n]/),
    ))),
    optional(token.immediate(/[^:\r\n]+/)),
  ),

  timeline_section_name: (_) => token(prec(
    -5,
    /(?:[^:%\r\n]|%[^%\r\n])+/
  )),

  timeline_line_text: (_) => token(prec(
    -5,
    /(?:[^%\r\n]|%[^%\r\n])+/
  )),

  timeline_malformed_event_text: (_) => token(prec(
    20,
    /:[^ \t\r\n][^\r\n]*/,
  )),

  timeline_hash_comment: (_) => token(seq('#', /[^\r\n]*/)),
};

module.exports = { timelineRules };
