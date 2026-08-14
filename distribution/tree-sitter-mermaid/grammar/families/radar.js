// Source translation: Mermaid 11.16.1
// packages/parser/src/language/radar/radar.langium and the imported common
// grammar at commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const { optionallyColonTerminatedHeader } = require('../shared/header');

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(20, keyword)), $.statement_keyword),
);

const radarRules = {
  radar_diagram: ($) => choice(
    seq(
      field('header', $.radar_header),
      optional(field('body', $.radar_body)),
    ),
    seq(
      field('header', alias($._radar_inline_header, $.radar_header)),
      field('body', $.radar_body),
    ),
    field('header', alias($._radar_header_eof, $.radar_header)),
  ),

  radar_header: ($) => optionallyColonTerminatedHeader(
    $,
    token(prec(20, 'radar-beta')),
  ),

  _radar_header_eof: ($) => seq(
    field(
      'keyword',
      alias(token(prec(20, 'radar-beta')), $.diagram_keyword),
    ),
    optional(field('colon', ':')),
  ),

  _radar_inline_header: ($) => seq(
    field(
      'keyword',
      alias(token(prec(20, 'radar-beta')), $.diagram_keyword),
    ),
    optional(field('colon', ':')),
    token.immediate(/[ \t]+/),
  ),

  radar_body: ($) => repeat1(choice(
    $.comment,
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
    optional(field(
      'text',
      alias($._radar_wardley_title_text, $.radar_title_text),
    )),
    optional($._line_ending),
  )),

  radar_accessibility_title_statement: ($) => prec.right(seq(
    statementKeyword($, 'accTitle'),
    field('colon', ':'),
    optional(field(
      'text',
      alias($._radar_wardley_accessibility_text, $.radar_accessibility_text),
    )),
    optional($._line_ending),
  )),

  radar_accessibility_description_statement: ($) => prec.right(choice(
    seq(
      statementKeyword($, 'accDescr'),
      field('colon', ':'),
      optional(field(
        'text',
        alias($._radar_wardley_accessibility_text, $.radar_accessibility_text),
      )),
      optional($._line_ending),
    ),
    seq(
      statementKeyword($, 'accDescr'),
      field(
        'text',
        alias($._radar_wardley_accessibility_block, $.radar_accessibility_block),
      ),
      optional($._line_ending),
    ),
  )),

  radar_axis_statement: ($) => prec.right(10, seq(
    statementKeyword($, 'axis'),
    field('axis', $.radar_axis),
    repeat(seq(',', field('axis', $.radar_axis))),
    optional($._line_ending),
  )),

  radar_axis: ($) => seq(
    field('name', $.radar_identifier),
    optional(field('label', $.radar_label)),
  ),

  radar_curve_statement: ($) => prec.right(10, seq(
    statementKeyword($, 'curve'),
    field('curve', $.radar_curve),
    repeat(seq(',', field('curve', $.radar_curve))),
    optional($._line_ending),
  )),

  radar_curve: ($) => seq(
    field('name', $.radar_identifier),
    optional(field('label', $.radar_label)),
    field('entries', $.radar_curve_entries),
  ),

  radar_curve_entries: ($) => seq(
    '{',
    repeat($._line_ending),
    choice(
      seq(
        field('entry', $.radar_number_entry),
        repeat(seq(
          ',',
          repeat($._line_ending),
          field('entry', $.radar_number_entry),
        )),
      ),
      seq(
        field('entry', $.radar_detailed_entry),
        repeat(seq(
          ',',
          repeat($._line_ending),
          field('entry', $.radar_detailed_entry),
        )),
      ),
    ),
    repeat($._line_ending),
    '}',
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
    optional($._line_ending),
  )),

  radar_option: ($) => choice(
    seq(
      field('name', alias('showLegend', $.radar_option_name)),
      field('value', $.radar_boolean),
    ),
    seq(
      field('name', alias('ticks', $.radar_option_name)),
      field('value', $.radar_number),
    ),
    seq(
      field('name', alias('max', $.radar_option_name)),
      field('value', $.radar_number),
    ),
    seq(
      field('name', alias('min', $.radar_option_name)),
      field('value', $.radar_number),
    ),
    seq(
      field('name', alias('graticule', $.radar_option_name)),
      field('value', $.radar_graticule),
    ),
  ),

  radar_label: ($) => seq(
    '[',
    field('text', $.quoted_string),
    ']',
  ),

  radar_incomplete_axis_statement: ($) => prec.right(-10, seq(
    statementKeyword($, 'axis'),
    $._line_ending,
  )),

  radar_incomplete_curve_statement: ($) => prec.right(-10, seq(
    statementKeyword($, 'curve'),
    optional(field('name', $.radar_identifier)),
    optional(field('label', $.radar_label)),
    $._line_ending,
  )),

  radar_malformed_statement: ($) => prec.right(-100, choice(
    seq(
      field(
        'keyword',
        alias($._radar_wardley_recovery_identifier, $.radar_unknown_keyword),
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
    /(?:[A-Za-z_\u00c0-\uffff]|[0-9]+[A-Za-z_\u00c0-\uffff])(?:[A-Za-z0-9_\-\u00c0-\uffff]*[A-Za-z0-9_\u00c0-\uffff])?/,
  ),

  radar_number: (_) => token(/(?:[0-9]+\.[0-9]+|0|[1-9][0-9]*)/),

  radar_boolean: (_) => choice('true', 'false'),

  radar_graticule: (_) => choice('circle', 'polygon'),

  radar_malformed_tail: (_) => token(prec(-10, /[^\r\n]+/)),

  radar_malformed_text: (_) => token(prec(-100,
    /[^A-Za-z0-9_\u00c0-\uffff\r\n][^\r\n]*/,
  )),
};

module.exports = { radarRules };
