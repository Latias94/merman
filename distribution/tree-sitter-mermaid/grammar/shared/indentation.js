const INDENTATION_FAMILIES = ['mindmap', 'treemap', 'tree_view'];

const indentationExternals = ($) => INDENTATION_FAMILIES.flatMap((family) => [
  $[`_${family}_start`],
  $[`_${family}_indent`],
  $[`_${family}_reindent`],
  $[`_${family}_dedent`],
  $[`${family}_indentation_overflow`],
]);

const indentationTransition = ($, family) => choice(
  $[`_${family}_start`],
  $[`_${family}_indent`],
  $[`_${family}_reindent`],
  $[`_${family}_dedent`],
  $[`${family}_indentation_overflow`],
);

module.exports = {
  indentationExternals,
  indentationTransition,
};
