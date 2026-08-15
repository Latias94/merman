// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/gantt/parser/gantt.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const diagramKeyword = ($) => field(
  'keyword',
  alias(token(prec(20, /gantt/i)), $.diagram_keyword),
);

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(keyword, $.statement_keyword),
);

const taskName = ($) => field('name', choice(
  $.gantt_task_name,
  alias($._gantt_task_compatible_keyword, $.gantt_task_name),
  alias($._gantt_keyword_prefixed_task_name, $.gantt_task_name),
));

const ganttRules = {
  gantt_diagram: ($) => choice(
    seq(
      field('header', $.gantt_header),
      optional(field('body', $.gantt_body)),
    ),
    seq(
      field('header', alias($._gantt_inline_header, $.gantt_header)),
      field('body', $.gantt_body),
    ),
    field('header', alias($._gantt_header_eof, $.gantt_header)),
  ),

  gantt_header: ($) => prec(30, seq(
    diagramKeyword($),
    optional(token.immediate(/[ \t]+/)),
    optional(choice($.comment, $.directive)),
    field('terminator', $._line_ending),
  )),

  _gantt_inline_header: ($) => seq(
    diagramKeyword($),
    token.immediate(/[ \t]+/),
  ),

  _gantt_header_eof: ($) => diagramKeyword($),

  gantt_body: ($) => choice(
    repeat1($._gantt_line_item),
    seq(
      repeat($._gantt_line_item),
      $._gantt_eof_item,
    ),
  ),

  _gantt_line_item: ($) => choice(
    seq(
      $._gantt_statement,
      optional(choice($.comment, $.directive)),
      $._line_ending,
    ),
    seq(choice($.comment, $.directive), $._line_ending),
    $._blank_line,
  ),

  _gantt_eof_item: ($) => choice(
    seq($._gantt_statement, optional(choice($.comment, $.directive))),
    $.comment,
    $.directive,
  ),

  _gantt_statement: ($) => choice(
    $.gantt_title_statement,
    $.gantt_accessibility_title_statement,
    $.gantt_accessibility_description_statement,
    $.gantt_setting_statement,
    $.gantt_flag_statement,
    $.gantt_weekday_statement,
    $.gantt_weekend_statement,
    $.gantt_section_statement,
    $.gantt_click_statement,
    $.gantt_task_statement,
    $.gantt_incomplete_task_statement,
    $.gantt_malformed_statement,
  ),

  gantt_title_statement: ($) => prec(30, seq(
    statementKeyword($, $._gantt_title_keyword),
    token.immediate(/[ \t]+/),
    field('text', $.gantt_line_text),
  )),

  gantt_accessibility_title_statement: ($) => prec(30, seq(
    statementKeyword($, $._gantt_acc_title_keyword),
    optional(token.immediate(/[ \t]+/)),
    field('delimiter', token.immediate(':')),
    optional(token.immediate(/[ \t]+/)),
    optional(field('text', $.gantt_line_text)),
  )),

  gantt_accessibility_description_statement: ($) => prec(30, seq(
    statementKeyword($, $._gantt_acc_descr_keyword),
    optional(token.immediate(/[ \t]+/)),
    choice(
      seq(
        field('delimiter', token.immediate(':')),
        optional(token.immediate(/[ \t]+/)),
        optional(field('text', $.gantt_line_text)),
      ),
      field('description', choice(
        $.gantt_accessibility_description_block,
        $.gantt_unclosed_accessibility_description_block,
      )),
    ),
  )),

  gantt_accessibility_description_block: ($) => seq(
    repeat($._line_ending),
    field('text', $.gantt_accessibility_block_text),
  ),

  gantt_unclosed_accessibility_description_block: ($) => seq(
    repeat($._line_ending),
    field('text', $.gantt_unclosed_accessibility_block_text),
  ),

  gantt_setting_statement: ($) => prec(30, choice(
    seq(
      statementKeyword($, $._gantt_setting_keyword),
      token.immediate(/[ \t]+/),
      field('value', $.gantt_setting_value),
    ),
    seq(
      statementKeyword($, $._gantt_today_marker_keyword),
      token.immediate(/[ \t]+/),
      field('value', $.gantt_today_marker_value),
    ),
  )),

  gantt_flag_statement: ($) => prec(
    30,
    statementKeyword($, $._gantt_flag_keyword),
  ),

  gantt_weekday_statement: ($) => prec(30, seq(
    statementKeyword($, $._gantt_weekday_keyword),
    token.immediate(/[ \t]+/),
    field('value', $.gantt_weekday),
  )),

  gantt_weekend_statement: ($) => prec(30, seq(
    statementKeyword($, $._gantt_weekend_keyword),
    token.immediate(/[ \t]+/),
    field('value', $.gantt_weekend_day),
  )),

  gantt_section_statement: ($) => prec(30, seq(
    statementKeyword($, $._gantt_section_keyword),
    token.immediate(/[ \t]+/),
    field('name', $.gantt_line_text),
  )),

  gantt_task_statement: ($) => prec(20, seq(
    taskName($),
    field('delimiter', ':'),
    field('metadata', $.gantt_task_metadata),
  )),

  gantt_task_metadata: ($) => seq(
    optional($._gantt_task_spacing),
    field('item', $.gantt_task_item),
    repeat(seq(
      ',',
      optional($._gantt_task_spacing),
      field('item', $.gantt_task_item),
    )),
  ),

  _gantt_task_spacing: (_) => token.immediate(prec(30, /[ \t]+/)),

  gantt_task_item: ($) => choice(
    field('status', $.gantt_task_status),
    field('constraint', $.gantt_dependency_clause),
    field('constraint', $.gantt_until_clause),
    field('date', $.gantt_date),
    field('duration', $.gantt_duration),
    field('value', $.gantt_task_atom),
  ),

  gantt_dependency_clause: ($) => seq(
    field(
      'keyword',
      alias(token(prec(1, /after/i)), $.gantt_constraint_keyword),
    ),
    token.immediate(/[ \t]+/),
    repeat1(field('reference', $.gantt_reference)),
  ),

  gantt_until_clause: ($) => seq(
    field(
      'keyword',
      alias(token(prec(1, /until/i)), $.gantt_constraint_keyword),
    ),
    token.immediate(/[ \t]+/),
    repeat1(field('reference', $.gantt_reference)),
  ),

  gantt_click_statement: ($) => prec(40, seq(
    statementKeyword($, $._gantt_click_keyword),
    $._gantt_required_space,
    field('task', $.gantt_reference),
    $._gantt_required_space,
    choice(
      field('action', $.gantt_href_action),
      field('action', $.gantt_call_action),
      field('action', $.gantt_malformed_href_action),
      seq(
        field('action', $.gantt_href_action),
        $._gantt_required_space,
        field('action', $.gantt_call_action),
      ),
      seq(
        field('action', $.gantt_call_action),
        $._gantt_required_space,
        field('action', $.gantt_href_action),
      ),
    ),
  )),

  gantt_href_action: ($) => seq(
    field(
      'keyword',
      alias(token(prec(30, /href/i)), $.gantt_action_keyword),
    ),
    $._gantt_required_space,
    field('url', choice($.gantt_url, $.gantt_unclosed_url)),
  ),

  // The upstream lexer requires whitespace after `href`. Preserve the action
  // boundary explicitly when an editor temporarily removes that whitespace.
  gantt_malformed_href_action: ($) => seq(
    field(
      'keyword',
      alias(token(prec(30, /href/i)), $.gantt_action_keyword),
    ),
    field('url', choice(
      alias($._gantt_immediate_url, $.gantt_url),
      alias($._gantt_immediate_unclosed_url, $.gantt_unclosed_url),
    )),
  ),

  gantt_call_action: ($) => seq(
    field(
      'keyword',
      alias(token(prec(30, /call/i)), $.gantt_action_keyword),
    ),
    $._gantt_required_space,
    field('name', $.gantt_callback_name),
    field('open', '('),
    optional(field('arguments', $.gantt_callback_arguments)),
    field('close', ')'),
  ),

  gantt_incomplete_task_statement: ($) => prec(-10, seq(
    taskName($),
    field('delimiter', ':'),
  )),

  gantt_malformed_statement: ($) => prec(-100, field(
    'text',
    alias($.gantt_task_name, $.gantt_malformed_text),
  )),

  gantt_weekday: (_) => token(prec(20, choice(
    /monday/i,
    /tuesday/i,
    /wednesday/i,
    /thursday/i,
    /friday/i,
    /saturday/i,
    /sunday/i,
  ))),

  gantt_weekend_day: (_) => token(prec(20, choice(/friday/i, /saturday/i))),

  gantt_task_status: (_) => token(prec(1, choice(
    /active/i,
    /done/i,
    /crit/i,
    /milestone/i,
    /vert/i,
  ))),

  gantt_date: (_) => token(prec(1, /[0-9]{4}-[0-9]{2}-[0-9]{2}/)),

  gantt_duration: (_) => token(prec(
    1,
    /[0-9]+(?:\.[0-9]+)?(?:ms|[Mdhmswy])/,
  )),

  gantt_reference: (_) => token(prec(10, /[^\s,#;()"\r\n]+/)),

  gantt_task_atom: ($) => repeat1($._gantt_task_atom_part),

  _gantt_task_atom_part: (_) => token(prec(1, /[^\s,#;\r\n]+/)),

  gantt_callback_name: (_) => token(prec(10, /[^\s(),\r\n]+/)),

  gantt_callback_arguments: (_) => token.immediate(/[^)\r\n]+/),

  gantt_url: (_) => token(seq('"', /[^"\r\n]*/, '"')),

  gantt_unclosed_url: (_) => token(prec(-10, seq('"', /[^"\r\n]*/))),

  gantt_line_text: (_) => token(prec(-5, /[^\r\n]+/)),

  gantt_setting_value: (_) => token(prec(-5, /[^#;\r\n]+/)),

  gantt_today_marker_value: (_) => token(prec(-5, /[^;\r\n]+/)),

  gantt_accessibility_block_text: (_) => token(seq('{', /[^}]*/, '}')),

  // Keep recovery line-local so an unfinished block cannot absorb later tasks.
  gantt_unclosed_accessibility_block_text: (_) => token(prec(
    -10,
    seq('{', /[^}\r\n]*/),
  )),

  // `%%` starts a comment even after indentation; a single leading percent
  // remains legal task text, matching the pinned lexer.
  gantt_task_name: (_) => token(prec(
    5,
    /(?:%[^%:\s\r\n]|[^%\s:\r\n])(?:%[^%:\r\n]|[^%:\r\n])*/,
  )),

  _gantt_immediate_url: (_) => token.immediate(seq('"', /[^"\r\n]*/, '"')),

  _gantt_immediate_unclosed_url: (_) => token.immediate(prec(
    -10,
    seq('"', /[^"\r\n]*/),
  )),

  _gantt_required_space: (_) => token.immediate(/[ \t]+/),

  _gantt_title_keyword: (_) => token(prec(20, /title/i)),

  _gantt_acc_title_keyword: (_) => token(prec(20, /accTitle/i)),

  _gantt_acc_descr_keyword: (_) => token(prec(20, /accDescr/i)),

  _gantt_setting_keyword: (_) => token(prec(20, choice(
    /dateFormat/i,
    /axisFormat/i,
    /tickInterval/i,
    /includes/i,
    /excludes/i,
    /accDescription/i,
  ))),

  _gantt_today_marker_keyword: (_) => token(prec(20, /todayMarker/i)),

  _gantt_flag_keyword: (_) => token(prec(20, choice(
    /inclusiveEndDates/i,
    /topAxis/i,
  ))),

  _gantt_weekday_keyword: (_) => token(prec(20, /weekday/i)),

  _gantt_weekend_keyword: (_) => token(prec(20, /weekend/i)),

  _gantt_section_keyword: (_) => token(prec(20, /section/i)),

  _gantt_click_keyword: (_) => token(prec(20, /click/i)),

  // These upstream keywords require following whitespace, so the exact word
  // remains a legal task name when it is followed by a task-data colon.
  _gantt_task_compatible_keyword: ($) => choice(
    $._gantt_title_keyword,
    $._gantt_setting_keyword,
    $._gantt_today_marker_keyword,
    $._gantt_weekday_keyword,
    $._gantt_weekend_keyword,
    $._gantt_section_keyword,
    $._gantt_click_keyword,
  ),

  _gantt_keyword_prefixed_task_name: (_) => seq(
    token(prec(30, choice(
      seq(/title/i, /[^\s:\r\n]/),
      seq(/accTitle/i, /[^\s:\r\n]/),
      seq(/accDescr/i, /[^\s:\r\n]/),
      seq(/dateFormat/i, /[^\s:\r\n]/),
      seq(/axisFormat/i, /[^\s:\r\n]/),
      seq(/tickInterval/i, /[^\s:\r\n]/),
      seq(/includes/i, /[^\s:\r\n]/),
      seq(/excludes/i, /[^\s:\r\n]/),
      seq(/todayMarker/i, /[^\s:\r\n]/),
      seq(/accDescription/i, /[^\s:\r\n]/),
      seq(/inclusiveEndDates/i, /[^\s:\r\n]/),
      seq(/topAxis/i, /[^\s:\r\n]/),
      seq(/weekday/i, /[^\s:\r\n]/),
      seq(/weekend/i, /[^\s:\r\n]/),
      seq(/section/i, /[^\s:\r\n]/),
      seq(/click/i, /[^\s:\r\n]/),
    ))),
    optional(token.immediate(/[^:\r\n]+/)),
  ),

};

module.exports = { ganttRules };
