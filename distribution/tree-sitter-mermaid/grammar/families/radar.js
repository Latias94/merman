// Source translation: Mermaid 11.16.1
// packages/parser/src/language/radar/radar.langium and the imported common
// grammar at commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.statement_keyword),
);

const radarStatementEnd = ($) => seq(
  optional(choice(
    field('comment', $.comment),
    field('directive', $.directive),
  )),
  optional(field('terminator', $._line_ending)),
);

const radarRules = {
  radar_diagram: ($) => choice(
    seq(
      field('header', alias($._radar_colon_header, $.radar_header)),
      optional(field('body', $.radar_body)),
    ),
    seq(
      field('header', $.radar_header),
      optional(seq(
        $._langium_body_boundary,
        optional(field('body', $.radar_body)),
      )),
    ),
  ),

  radar_header: ($) => field(
    'keyword',
    alias(token(prec(20, 'radar-beta')), $.diagram_keyword),
  ),

  _radar_colon_header: ($) => seq(
    field(
      'keyword',
      alias(token(prec(20, 'radar-beta')), $.diagram_keyword),
    ),
    choice(
      field('colon', token.immediate(':')),
      seq(
        $._langium_inline_space,
        field('colon', token.immediate(':')),
      ),
    ),
  ),

  radar_body: ($) => repeat1(choice(
    $.comment,
    $.directive,
    $._blank_line,
    $.radar_title_statement,
    $.radar_accessibility_title_statement,
    $.radar_accessibility_description_statement,
    $.radar_axis_statement,
    $.radar_curve_statement,
    $.radar_option_statement,
    $.radar_incomplete_axis_statement,
    $.radar_incomplete_curve_statement,
    $.radar_malformed_statement,
  )),

  radar_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'title'),
    optional(seq(
      $._langium_inline_space,
      optional(field(
        'text',
        alias($._radar_wardley_title_text, $.radar_title_text),
      )),
    )),
    radarStatementEnd($),
  )),

  radar_accessibility_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'accTitle'),
    field('colon', ':'),
    optional(field(
      'text',
      alias($._radar_wardley_accessibility_text, $.radar_accessibility_text),
    )),
    radarStatementEnd($),
  )),

  radar_accessibility_description_statement: ($) => prec.right(choice(
    seq(
      statementKeyword($, 'accDescr'),
      field('colon', ':'),
      optional(field(
        'text',
        alias($._radar_wardley_accessibility_text, $.radar_accessibility_text),
      )),
      radarStatementEnd($),
    ),
    seq(
      statementKeyword($, 'accDescr'),
      repeat($._line_ending),
      field(
        'text',
        alias($._radar_wardley_accessibility_block, $.radar_accessibility_block),
      ),
      radarStatementEnd($),
    ),
  )),

  radar_axis_statement: ($) => prec.right(10, seq(
    statementKeyword($, 'axis'),
    $._langium_inline_space,
    field('axis', $.radar_axis),
    repeat(seq(',', field('axis', $.radar_axis))),
    radarStatementEnd($),
  )),

  radar_axis: ($) => seq(
    field('name', $.radar_identifier),
    optional(field('label', $.radar_label)),
  ),

  radar_curve_statement: ($) => prec.right(10, seq(
    statementKeyword($, 'curve'),
    $._langium_inline_space,
    field('curve', $.radar_curve),
    repeat(seq(',', field('curve', $.radar_curve))),
    radarStatementEnd($),
  )),

  radar_curve: ($) => seq(
    field('name', $.radar_identifier),
    optional(field('label', $.radar_label)),
    field('entries', $.radar_curve_entries),
  ),

  radar_curve_entries: ($) => seq(
    '{',
    repeat($._radar_entry_trivia),
    choice(
      seq(
        field('entry', $.radar_number_entry),
        repeat(seq(
          ',',
          repeat($._radar_entry_trivia),
          field('entry', $.radar_number_entry),
        )),
      ),
      seq(
        field('entry', $.radar_detailed_entry),
        repeat(seq(
          ',',
          repeat($._radar_entry_trivia),
          field('entry', $.radar_detailed_entry),
        )),
      ),
    ),
    repeat($._radar_entry_trivia),
    '}',
  ),

  _radar_entry_trivia: ($) => choice(
    $._line_ending,
    $.comment,
    $.directive,
  ),

  radar_number_entry: ($) => field('value', $.radar_number),

  radar_detailed_entry: ($) => seq(
    field('axis', $.radar_identifier),
    optional(field('colon', ':')),
    field('value', $.radar_number),
  ),

  radar_option_statement: ($) => prec.right(seq(
    field('option', $.radar_option),
    repeat(seq(',', field('option', $.radar_option))),
    radarStatementEnd($),
  )),

  radar_option: ($) => choice(
    seq(
      field('name', alias('showLegend', $.radar_option_name)),
      $._langium_inline_space,
      field('value', $.radar_boolean),
    ),
    seq(
      field('name', alias('ticks', $.radar_option_name)),
      $._langium_inline_space,
      field('value', $.radar_number),
    ),
    seq(
      field('name', alias('max', $.radar_option_name)),
      $._langium_inline_space,
      field('value', $.radar_number),
    ),
    seq(
      field('name', alias('min', $.radar_option_name)),
      $._langium_inline_space,
      field('value', $.radar_number),
    ),
    seq(
      field('name', alias('graticule', $.radar_option_name)),
      $._langium_inline_space,
      field('value', $.radar_graticule),
    ),
  ),

  radar_label: ($) => choice(
    seq(
      '[',
      field(
        'text',
        alias($._radar_wardley_quoted_string, $.quoted_string),
      ),
      ']',
    ),
    seq(
      '[',
      field(
        'recovery',
        alias(
          $._radar_wardley_unclosed_quoted_string,
          $.radar_unclosed_quoted_string,
        ),
      ),
    ),
  ),

  radar_incomplete_axis_statement: ($) => prec.right(-10, seq(
    statementKeyword($, 'axis'),
    $._line_ending,
  )),

  radar_incomplete_curve_statement: ($) => prec.right(-10, choice(
    seq(statementKeyword($, 'curve'), $._line_ending),
    seq(
      statementKeyword($, 'curve'),
      $._langium_inline_space,
      field('name', $.radar_identifier),
      optional(field('label', $.radar_label)),
      $._line_ending,
    ),
  )),

  radar_malformed_statement: ($) => prec.right(-100, choice(
    seq(
      field(
        'keyword',
        alias($.identifier, $.radar_unknown_keyword),
      ),
      optional(field('text', $.radar_malformed_tail)),
      optional($._line_ending),
    ),
    seq(
      field('text', $.radar_malformed_text),
      optional($._line_ending),
    ),
  )),

  radar_identifier: (_) => token(
    /[A-Za-z0-9_](?:[A-Za-z0-9_-]*[A-Za-z0-9_])?/,
  ),

  radar_number: (_) => token(prec(
    10,
    /(?:[0-9]+\.[0-9]+|0|[1-9][0-9]*)/,
  )),

  radar_boolean: (_) => choice('true', 'false'),

  radar_graticule: (_) => choice('circle', 'polygon'),

  radar_malformed_tail: (_) => token(prec(-10, /[^\r\n]+/)),

  radar_malformed_text: (_) => token(prec(-100,
    /[^A-Za-z0-9_\u00c0-\uffff\r\n][^\r\n]*/,
  )),
};

const radarConflicts = ($) => [
  [$.radar_header, $._radar_colon_header],
];

module.exports = { radarConflicts, radarRules };
