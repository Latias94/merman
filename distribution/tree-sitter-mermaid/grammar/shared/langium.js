// Source translation: Mermaid 11.16.1
// packages/parser/src/language/common/common.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const langiumStatementEnd = ($) => seq(
  optional(field('comment', $.comment)),
  optional(field('terminator', $._langium_newline)),
);

const langiumInlineSpacing = () => optional(token.immediate(/[ \t]+/));

const langiumRules = {
  // Mermaid's imported NEWLINE terminal is CRLF/LF-only, and EOL accepts one
  // or more NEWLINE tokens. EOF is represented by the optional use at each
  // statement boundary.
  _langium_newline: (_) => token(/(?:\r?\n)+/),

  _langium_inline_space: (_) => token.immediate(/[ \t]+/),

  // Mermaid's Langium token builders accept a bare family keyword only at
  // whitespace, comment/directive, or EOF. Families consume this boundary at
  // the diagram seam so the first body node retains its exact byte range.
  _langium_body_boundary: ($) => choice(
    $._langium_inline_space,
    $._langium_newline,
    $.comment,
    $.directive,
  ),

  // Mermaid's common STRING terminal permits either quote style, escaped
  // non-line-terminator characters, and unescaped newlines inside the token.
  langium_string: (_) => token(choice(
    seq('"', repeat(choice(/[^"\\]/, /\\./)), '"'),
    seq("'", repeat(choice(/[^'\\]/, /\\./)), "'"),
  )),

  langium_title_statement: ($) => prec.right(seq(
    field(
      'keyword',
      alias(token(prec(20, 'title')), $.statement_keyword),
    ),
    optional(seq(
      token.immediate(/[ \t]+/),
      optional(field('text', $.langium_line_text)),
    )),
    langiumStatementEnd($),
  )),

  langium_acc_title_statement: ($) => prec.right(seq(
    field(
      'keyword',
      alias(token(prec(20, 'accTitle')), $.statement_keyword),
    ),
    langiumInlineSpacing(),
    field('delimiter', token.immediate(':')),
    langiumInlineSpacing(),
    optional(field('text', $.langium_line_text)),
    langiumStatementEnd($),
  )),

  langium_acc_descr_statement: ($) => prec.right(seq(
    field(
      'keyword',
      alias(token(prec(20, 'accDescr')), $.statement_keyword),
    ),
    choice(
      seq(
        field('delimiter', ':'),
        langiumInlineSpacing(),
        optional(field('text', $.langium_line_text)),
      ),
      seq(
        optional($._langium_newline),
        field('description', $.langium_acc_descr_block),
      ),
    ),
    langiumStatementEnd($),
  )),

  langium_acc_descr_block: ($) => seq(
    field('open', '{'),
    optional(field('text', $.langium_acc_descr_block_text)),
    field('close', '}'),
  ),

  // Keep the payload opaque, but stop before a Langium single-line comment.
  // A single '%' remains ordinary text; when '%%' is possible, the longer
  // comment token wins and becomes a sibling inside the statement.
  langium_line_text: (_) => prec(-1, repeat1(choice(
    token.immediate(/[^%\r\n]+/),
    token.immediate('%'),
  ))),

  langium_acc_descr_block_text: (_) => token.immediate(/[^}]+/),
};

module.exports = { langiumRules };
