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

  unstructured_body: ($) => prec.right(repeat1(choice(
    seq($.unstructured_line, optional($._line_ending)),
    $._line_ending,
  ))),

  unstructured_line: (_) => token(prec(-100, /[^\r\n]+/)),
};

module.exports = { commonRules };
