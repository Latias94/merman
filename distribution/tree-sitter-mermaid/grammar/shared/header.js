const terminatedHeader = ($, keyword, ...suffix) => seq(
  field('keyword', alias(keyword, $.diagram_keyword)),
  ...suffix,
  field('terminator', $._line_ending),
);

const optionallyColonTerminatedHeader = ($, keyword) => terminatedHeader(
  $,
  keyword,
  optional(field('colon', ':')),
);

module.exports = {
  optionallyColonTerminatedHeader,
  terminatedHeader,
};
