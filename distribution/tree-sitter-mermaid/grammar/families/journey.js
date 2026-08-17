// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/user-journey/parser/journey.jison and
// packages/mermaid/src/diagrams/user-journey/journeyDb.js
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, /journey/i)), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.statement_keyword),
);

const trailingTrivia = ($) => optional(choice(
  $.comment,
  $.directive,
  $.journey_hash_comment,
));

const taskName = ($) => field('task', choice(
  $.journey_task_name,
  alias($._journey_keyword_prefixed_task_name, $.journey_task_name),
));

const journeyRules = {
  journey_diagram: ($) => choice(
    seq(
      field('header', $.journey_header),
      optional(field('body', $.journey_body)),
    ),
    seq(
      field('header', alias($._journey_inline_header, $.journey_header)),
      field('body', $.journey_body),
    ),
    field('header', alias($._journey_header_eof, $.journey_header)),
  ),

  journey_header: ($) => seq(
    diagramKeyword($),
    field('terminator', $._line_ending),
  ),

  _journey_inline_header: ($) => seq(
    diagramKeyword($),
    token.immediate(/[ \t]+/),
  ),

  _journey_header_eof: ($) => diagramKeyword($),

  journey_body: ($) => choice(
    repeat1($._journey_line_item),
    seq(
      repeat($._journey_line_item),
      $._journey_eof_item,
    ),
  ),

  _journey_line_item: ($) => choice(
    seq($._journey_statement, trailingTrivia($), $._line_ending),
    seq(choice($.comment, $.directive, $.journey_hash_comment), $._line_ending),
    $._blank_line,
  ),

  _journey_eof_item: ($) => choice(
    seq($._journey_statement, trailingTrivia($)),
    $.comment,
    $.directive,
    $.journey_hash_comment,
  ),

  _journey_statement: ($) => choice(
    $.journey_title_statement,
    $.journey_accessibility_title_statement,
    $.journey_accessibility_description_statement,
    $.journey_section_statement,
    $.journey_task_statement,
    $.journey_incomplete_section_statement,
    $.journey_incomplete_task_statement,
    $.journey_malformed_statement,
  ),

  journey_title_statement: ($) => prec(30, seq(
    statementKeyword($, /title/i),
    token.immediate(/[ \t]+/),
    field('text', $.journey_title_text),
  )),

  journey_accessibility_title_statement: ($) => prec(30, seq(
    statementKeyword($, /accTitle/i),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.journey_accessibility_line_text)),
  )),

  journey_accessibility_description_statement: ($) => prec(30, seq(
    statementKeyword($, /accDescr/i),
    optional(token.immediate(/[ \t]+/)),
    choice(
      seq(
        field('delimiter', token.immediate(':')),
        optional(token.immediate(/[ \t]+/)),
        optional(field('text', $.journey_accessibility_line_text)),
      ),
      field('description', choice(
        $.journey_accessibility_description_block,
        $.journey_unclosed_accessibility_description_block,
      )),
    ),
  )),

  journey_section_statement: ($) => prec(30, seq(
    statementKeyword($, /section/i),
    token.immediate(/[ \t]+/),
    field('section', $.journey_section_name),
  )),

  journey_task_statement: ($) => prec(20, seq(
    taskName($),
    field('delimiter', alias(':', $.journey_task_delimiter)),
    optional(token.immediate(/[ \t]+/)),
    field('score', $.journey_score),
    optional(field('score_suffix', $.journey_score_suffix)),
    optional(seq(
      field('actors_delimiter', alias(':', $.journey_actor_delimiter)),
      optional(token.immediate(/[ \t]+/)),
      optional(field('actors', $.journey_actor_list)),
    )),
  )),

  journey_actor_list: ($) => seq(
    field('actor', $.journey_actor),
    repeat(seq(
      field('delimiter', ','),
      optional(token.immediate(/[ \t]+/)),
      field('actor', $.journey_actor),
    )),
  ),

  journey_incomplete_section_statement: ($) => prec(-10, seq(
    statementKeyword($, /section/i),
  )),

  journey_incomplete_task_statement: ($) => prec(-20, seq(
    taskName($),
    field('delimiter', alias(':', $.journey_task_delimiter)),
    optional(field('recovery', $.journey_task_recovery_text)),
  )),

  journey_malformed_statement: ($) => prec(-100, field(
    'text',
    choice(
      alias($.journey_task_name, $.journey_malformed_text),
      alias($._journey_keyword_prefixed_task_name, $.journey_malformed_text),
    ),
  )),

  journey_accessibility_description_block: (_) => token(seq(
    '{',
    /[^}]*/,
    '}',
  )),

  journey_unclosed_accessibility_description_block: (_) => token(prec(-10, seq(
    '{',
    /[^}\r\n]*/,
  ))),

  journey_score: (_) => token.immediate(prec(
    20,
    /[+-]?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)/,
  )),

  journey_score_suffix: (_) => token.immediate(/[A-Za-z_][A-Za-z0-9_.-]*/),

  journey_title_text: (_) => token(prec(
    -5,
    /(?:[^#%;\r\n]|%[^%\r\n])+/
  )),

  journey_accessibility_line_text: (_) => token(prec(
    -5,
    /(?:[^#%;\r\n]|%[^%\r\n])+/
  )),

  journey_section_name: (_) => token(prec(
    5,
    /(?:[^#:%;\r\n]|%[^%\r\n])+/
  )),

  journey_task_name: (_) => token(prec(
    5,
    /[^\s#:%;\r\n](?:[^#:%;\r\n]*[^\s#:%;\r\n])?/
  )),

  _journey_keyword_prefixed_task_name: (_) => seq(
    token(prec(30, choice(
      seq(/title/i, /[^\s#:%;\r\n]/),
      seq(/accTitle/i, /[^\s#:%;\r\n]/),
      seq(/accDescr/i, /[^\s#:%;\r\n]/),
      seq(/section/i, /[^\s#:%;\r\n]/),
    ))),
    optional(token.immediate(/[^#:%;\r\n]+/)),
  ),

  journey_actor: (_) => token.immediate(prec(
    5,
    /[^\s#,:%;\r\n](?:[^#,:%;\r\n]*[^\s#,:%;\r\n])?/,
  )),

  journey_task_recovery_text: (_) => token(prec(
    -50,
    /(?:[^#%\r\n]|%[^%\r\n])+/
  )),

  journey_hash_comment: (_) => token(prec(10, seq('#', /[^\r\n]*/))),
};

module.exports = { journeyRules };
