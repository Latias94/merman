const commonRules = {
  _blank_line: ($) => $._line_ending,

  _line_ending: (_) => token(choice('\r\n', '\n', '\r')),

  _statement_terminator: ($) => choice($._line_ending, ';'),

  direction: (_) => choice('LR', 'RL', 'TB', 'BT', 'TD', 'BR', '<', '>', '^', 'v'),

  orientation: (_) => choice('vertical', 'horizontal'),

  identifier: (_) => token(/[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\-\u00c0-\uffff]*/),

  number: (_) => token(/-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?/),

  quoted_string: (_) => token(choice(
    seq('"', /(?:[^"\\]|\\.)*/, '"'),
    seq("'", /(?:[^'\\]|\\.)*/, "'"),
  )),

  _radar_wardley_title_text: (_) => token(prec(5, /[^\s\r\n][^\r\n]*/)),

  _radar_wardley_accessibility_text: (_) => token(prec(5, /[^\r\n]+/)),

  _radar_wardley_accessibility_block: (_) => token(seq('{', /[^}]*/, '}')),

  _radar_wardley_recovery_identifier: (_) => token(prec(
    -10,
    /[A-Za-z_\u00c0-\uffff][A-Za-z0-9_\-\u00c0-\uffff]*/,
  )),

  unstructured_body: ($) => prec.right(repeat1(choice(
    seq($.unstructured_line, optional($._line_ending)),
    $._line_ending,
  ))),

  unstructured_line: (_) => token(prec(-100, /[^\r\n]+/)),
};

module.exports = { commonRules };
