// Source translation: Mermaid 11.16.1
// packages/parser/src/language/common/common.langium
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

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

  // Keep complete multiline strings structurally distinct from the line-local
  // recovery form. The dynamic preference preserves Mermaid's multiline
  // STRING behavior when a closing quote exists, while an unfinished edit can
  // stop before the next statement instead of consuming its opening quote.
  langium_string: ($) => prec.dynamic(10, choice(
    seq(
      '"',
      repeat(choice(
        $._langium_double_quoted_content,
        $._langium_escape_sequence,
        $._langium_newline,
      )),
      token.immediate('"'),
    ),
    seq(
      "'",
      repeat(choice(
        $._langium_single_quoted_content,
        $._langium_escape_sequence,
        $._langium_newline,
      )),
      token.immediate("'"),
    ),
  )),

  langium_unclosed_string: ($) => prec.dynamic(-10, choice(
    seq(
      '"',
      repeat(choice(
        $._langium_double_quoted_content,
        $._langium_escape_sequence,
      )),
    ),
    seq(
      "'",
      repeat(choice(
        $._langium_single_quoted_content,
        $._langium_escape_sequence,
      )),
    ),
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
    optional(choice(
      field('comment', $.comment),
      field('directive', $.directive),
    )),
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
    optional(choice(
      field('comment', $.comment),
      field('directive', $.directive),
    )),
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
        field('description', choice(
          $.langium_acc_descr_block,
          $.langium_unclosed_acc_descr_block,
        )),
      ),
    ),
    optional(choice(
      field('comment', $.comment),
      field('directive', $.directive),
    )),
  )),

  // The payload is intentionally opaque. Keeping each complete block in one
  // lexical leaf prevents Tree-sitter from manufacturing a missing `}` after
  // greedily consuming later declarations. The recovery token is line-local.
  langium_acc_descr_block: ($) => field(
    'text',
    $.langium_acc_descr_block_text,
  ),

  langium_unclosed_acc_descr_block: ($) => field(
    'text',
    $.langium_unclosed_acc_descr_block_text,
  ),

  // Keep the payload opaque, but stop before a Langium single-line comment.
  // A single '%' remains ordinary text; when '%%' is possible, the longer
  // comment token wins and becomes a sibling inside the statement.
  // Langium's value converter trims outer horizontal whitespace while
  // preserving internal runs and stops before a `%%` comment.
  langium_line_text: (_) => token(prec(
    -1,
    /[^\s%\r\n](?:(?:[^%\r\n]|%[^%\r\n])*[^\s%\r\n])?/,
  )),

  langium_acc_descr_block_text: (_) => token(prec(
    10,
    seq('{', /[^}]*/, '}'),
  )),

  langium_unclosed_acc_descr_block_text: (_) => token(prec(
    -10,
    seq('{', /[^\r\n]*/),
  )),

  _langium_double_quoted_content: (_) => token.immediate(/[^"\\\r\n]+/),

  _langium_single_quoted_content: (_) => token.immediate(/[^'\\\r\n]+/),

  _langium_escape_sequence: (_) => token.immediate(/\\[^\r\n]/),
};

const langiumConflicts = ($) => [
  [$.langium_string, $.langium_unclosed_string],
];

module.exports = { langiumConflicts, langiumRules };
