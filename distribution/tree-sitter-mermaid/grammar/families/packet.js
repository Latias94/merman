// Source translation: Mermaid 11.16.1
// packages/parser/src/language/packet/packet.langium:4-20 and the imported
// packages/parser/src/language/common/common.langium:6-34 via shared/langium.js.
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const packetStatementEnd = ($) => seq(
  optional(field('comment', $.comment)),
  optional(field('terminator', $._langium_newline)),
);

const packetRules = {
  packet_diagram: ($) => choice(
    seq(
      field('header', alias($._packet_beta_header, $.packet_header)),
      optional(field('body', $.packet_body)),
    ),
    seq(
      field('header', $.packet_header),
      optional(seq(
        $._langium_body_boundary,
        optional(field('body', $.packet_body)),
      )),
    ),
  ),

  // Mermaid permits common statements on the header line. Leaving the line
  // boundary to the body keeps same-line and following-line statements equal.
  packet_header: ($) => field(
    'keyword',
    alias(token(prec(20, 'packet')), $.diagram_keyword),
  ),

  _packet_beta_header: ($) => field(
    'keyword',
    alias(token(prec(20, 'packet-beta')), $.diagram_keyword),
  ),

  packet_body: ($) => repeat1(choice(
    $.langium_title_statement,
    $.langium_acc_title_statement,
    $.langium_acc_descr_statement,
    $.packet_block_statement,
    $.packet_incomplete_block_statement,
    $.comment,
    $.directive,
    $._blank_line,
    $.packet_recovery_statement,
  )),

  packet_block_statement: ($) => prec.right(seq(
    field('range', choice($.packet_bit_range, $.packet_bit_count)),
    field('delimiter', alias(':', $.packet_label_delimiter)),
    optional($._langium_inline_space),
    field('label', $.langium_string),
    packetStatementEnd($),
  )),

  packet_bit_range: ($) => seq(
    field('start', $.packet_integer),
    optional(seq(
      field('operator', alias('-', $.packet_range_operator)),
      field('end', $.packet_integer),
    )),
  ),

  packet_bit_count: ($) => seq(
    field('operator', alias('+', $.packet_width_operator)),
    field('bits', $.packet_integer),
  ),

  // A missing label is an editing intermediate with useful range structure.
  packet_incomplete_block_statement: ($) => prec(-1, seq(
    field('range', choice($.packet_bit_range, $.packet_bit_count)),
    field('delimiter', alias(':', $.packet_label_delimiter)),
    optional($._langium_inline_space),
  )),

  packet_integer: (_) => token(/0|[1-9][0-9]*/),

  // This is a finite, line-local recovery leaf. It cannot consume a valid
  // sibling on the next line and never substitutes for the complete body.
  packet_recovery_statement: ($) => prec.right(seq(
    field('text', $.packet_recovery_text),
    optional(field('terminator', $._langium_newline)),
  )),

  packet_recovery_text: (_) => token(prec(-100, /[^\r\n]+/)),
};

module.exports = { packetRules };
