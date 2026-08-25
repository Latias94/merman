const commonRules = {
  _blank_line: ($) => $._line_ending,

  _line_ending: (_) => token(choice('\r\n', '\n', '\r')),

  _statement_terminator: ($) => choice($._line_ending, ';'),

  direction: (_) => choice('LR', 'RL', 'TB', 'BT', 'TD', 'BR', '<', '>', '^', 'v'),

  identifier: (_) => token(/[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\-\u00c0-\uffff]*/),

  quoted_string: (_) => token(choice(
    seq('"', /(?:[^"\\]|\\.)*/, '"'),
    seq("'", /(?:[^'\\]|\\.)*/, "'"),
  )),

  _radar_wardley_quoted_string: ($) => prec.dynamic(10, choice(
    seq(
      '"',
      repeat(choice(
        $._radar_wardley_double_quoted_content,
        $._radar_wardley_escape_sequence,
        $._line_ending,
      )),
      token.immediate('"'),
    ),
    seq(
      "'",
      repeat(choice(
        $._radar_wardley_single_quoted_content,
        $._radar_wardley_escape_sequence,
        $._line_ending,
      )),
      token.immediate("'"),
    ),
  )),

  _radar_wardley_unclosed_quoted_string: ($) => prec.dynamic(-10, choice(
    seq(
      '"',
      repeat(choice(
        $._radar_wardley_double_quoted_content,
        $._radar_wardley_escape_sequence,
      )),
    ),
    seq(
      "'",
      repeat(choice(
        $._radar_wardley_single_quoted_content,
        $._radar_wardley_escape_sequence,
      )),
    ),
  )),

  _radar_wardley_double_quoted_content: (_) => token.immediate(/[^"\\\r\n]+/),

  _radar_wardley_single_quoted_content: (_) => token.immediate(/[^'\\\r\n]+/),

  _radar_wardley_escape_sequence: (_) => token.immediate(/\\[^\r\n]/),

  _radar_wardley_title_text: (_) => token(prec(
    5,
    /[^\s%\r\n](?:[^%\r\n]|%[^%\r\n])*/,
  )),

  _radar_wardley_accessibility_text: (_) => token(prec(
    5,
    /(?:[^%\r\n]|%[^%\r\n])+/
  )),

  _radar_wardley_accessibility_block: (_) => token(seq('{', /[^}]*/, '}')),
};

const commonConflicts = ($) => [
  [$._radar_wardley_quoted_string, $._radar_wardley_unclosed_quoted_string],
];

module.exports = { commonConflicts, commonRules };
