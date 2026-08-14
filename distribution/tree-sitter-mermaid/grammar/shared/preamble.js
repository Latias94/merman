const preambleRules = {
  bom: (_) => '\ufeff',

  frontmatter: ($) => seq(
    $.frontmatter_delimiter,
    $._line_ending,
    repeat($.frontmatter_line),
    $.frontmatter_delimiter,
  ),

  frontmatter_delimiter: (_) => '---',

  frontmatter_line: ($) => seq(optional($.frontmatter_content), $._line_ending),

  frontmatter_content: (_) => token(prec(-1, /[^\r\n]+/)),

  directive: (_) => token(seq('%%{', /[^%]*(?:%+[^}%][^%]*)*/, '}%%')),

  comment: (_) => token(seq('%%', /[^\r\n]*/)),
};

module.exports = { preambleRules };
