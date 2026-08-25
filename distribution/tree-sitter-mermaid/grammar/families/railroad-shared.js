// The metadata rules below translate TITLE, ACC_TITLE, and ACC_DESCR from
// lines 3-5 of all four Mermaid 11.16.1 railroad*.langium grammars. Those
// token languages are identical at commit
// 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const statementKeyword = ($, keyword) => field(
  'keyword',
  alias(token(prec(50, keyword)), $.railroad_statement_keyword),
);

const inlineSpacing = () => optional(token.immediate(prec(100, /[ \t]+/)));

// IR RR_ID/RR_STRING (lines 7-8), EBNF EBNF_ID/EBNF_STRING (lines 7-8),
// and PEG PEG_ID/PEG_STRING (lines 7-8) use identical terminal languages.
// The named CST nodes remain dialect-local.
const railroadIdentifierToken = () => token(
  prec(20, /[A-Z_a-z][A-Za-z0-9_-]*/),
);

const railroadEscapedStringToken = () => token(prec(10, choice(
  seq('"', /(?:[^"\\]|\\.)*/, '"'),
  seq("'", /(?:[^'\\]|\\.)*/, "'"),
)));

// Editing recovery derived from the shared STRING terminal. All three
// dialects stop an unclosed quote at the same CR/LF boundary.
const railroadUnclosedEscapedStringToken = () => token(prec(-20, choice(
  seq('"', /(?:[^"\\\r\n]|\\[^\r\n])*/),
  seq("'", /(?:[^'\\\r\n]|\\[^\r\n])*/),
)));

// IR RR_BLOCK_COMMENT (line 14) and EBNF EBNF_BLOCK_COMMENT (line 19)
// define the same C-style block-comment terminal.
const railroadCStyleBlockCommentToken = () => token(prec(
  40,
  /\/\*[^*]*(?:\*+[^*/][^*]*)*\*+\//,
));

// EBNF postfix rules (lines 152-161) and PEG suffix operators (line 87)
// share this token set.
const railroadSuffixQuantifierToken = () => token(
  prec(20, choice('?', '*', '+')),
);

const railroadSharedRules = {
  railroad_title_statement: ($) => prec.right(40, seq(
    statementKeyword($, 'title'),
    optional(seq(
      token.immediate(prec(100, /[ \t]+/)),
      optional(field('text', $.railroad_line_text)),
    )),
  )),

  railroad_acc_title_statement: ($) => prec.right(40, seq(
    statementKeyword($, 'accTitle'),
    inlineSpacing(),
    field('delimiter', token.immediate(':')),
    inlineSpacing(),
    optional(field('text', $.railroad_line_text)),
  )),

  railroad_acc_descr_statement: ($) => prec.right(40, seq(
    statementKeyword($, 'accDescr'),
    choice(
      seq(
        inlineSpacing(),
        field('delimiter', token.immediate(':')),
        inlineSpacing(),
        optional(field('text', $.railroad_line_text)),
      ),
      seq(
        repeat($._line_ending),
        field('description', choice(
          $.railroad_acc_descr_block,
          $.railroad_unclosed_acc_descr_block,
        )),
      ),
    ),
  )),

  railroad_acc_descr_block: ($) => seq(
    field('open', '{'),
    optional(field('text', $.railroad_acc_descr_block_text)),
    field('close', '}'),
  ),

  railroad_unclosed_acc_descr_block: ($) => seq(
    field('open', '{'),
    optional(field('text', $.railroad_unclosed_acc_descr_block_text)),
  ),

  // These line payloads intentionally stop before Mermaid's `%%` comment.
  railroad_line_text: (_) => token(prec(60, /(?:[^%\r\n]|%[^%])+/)),

  railroad_acc_descr_block_text: (_) => token.immediate(/[^}]+/),

  railroad_unclosed_acc_descr_block_text: (_) => token.immediate(/[^}\r\n]+/),
};

const railroadMetadataStatements = ($) => choice(
  $.railroad_title_statement,
  $.railroad_acc_title_statement,
  $.railroad_acc_descr_statement,
);

module.exports = {
  railroadCStyleBlockCommentToken,
  railroadEscapedStringToken,
  railroadIdentifierToken,
  railroadMetadataStatements,
  railroadSharedRules,
  railroadSuffixQuantifierToken,
  railroadUnclosedEscapedStringToken,
};
