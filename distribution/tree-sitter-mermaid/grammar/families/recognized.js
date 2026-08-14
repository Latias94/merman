const { terminatedHeader } = require('../shared/header');

const keywordChoice = (...keywords) => token(prec(
  20,
  keywords.length === 1 ? keywords[0] : choice(...keywords),
));

const headerKeyword = ($, keyword) => field(
  'keyword',
  alias(keyword, $.diagram_keyword),
);

const spacedField = ($, fieldName, value, nodeType) => seq(
  token.immediate(/[ \t]+/),
  field(fieldName, alias(token.immediate(value), nodeType)),
);

const simple = (root, header, acceptsEof, ...keywords) => ({
  root,
  header,
  rule: ($) => terminatedHeader($, keywordChoice(...keywords)),
  eofRule: acceptsEof
    ? ($) => headerKeyword($, keywordChoice(...keywords))
    : null,
});

const FAMILY_SPECS = [
  simple('block_diagram', 'block_header', false, 'block-beta', 'block'),
  simple(
    'c4_diagram',
    'c4_header',
    false,
    'C4Deployment',
    'C4Component',
    'C4Container',
    'C4Dynamic',
    'C4Context',
  ),
  simple('class_diagram', 'class_header', false, 'classDiagram-v2', 'classDiagram'),
  simple('entity_relationship_diagram', 'entity_relationship_header', true, 'erDiagram'),
  simple('gantt_diagram', 'gantt_header', true, 'gantt'),
  {
    root: 'ishikawa_diagram',
    header: 'ishikawa_header',
    rule: ($) => terminatedHeader($, token(prec(20, /ishikawa(?:-beta)?/i))),
    eofRule: null,
  },
  simple('journey_diagram', 'journey_header', true, 'journey'),
  simple('quadrant_chart_diagram', 'quadrant_chart_header', true, 'quadrantChart'),
  simple('railroad_diagram', 'railroad_header', true, 'railroad-beta'),
  simple('railroad_abnf_diagram', 'railroad_abnf_header', true, 'railroad-abnf-beta'),
  simple('railroad_ebnf_diagram', 'railroad_ebnf_header', true, 'railroad-ebnf-beta'),
  simple('railroad_peg_diagram', 'railroad_peg_header', true, 'railroad-peg-beta'),
  {
    root: 'requirement_diagram',
    header: 'requirement_header',
    rule: ($) => terminatedHeader(
      $,
      token(prec(20, /requirement[dD][iI][aA][gG][rR][aA][mM]/)),
    ),
    eofRule: ($) => headerKeyword(
      $,
      token(prec(20, /requirement[dD][iI][aA][gG][rR][aA][mM]/)),
    ),
  },
  simple('sequence_diagram', 'sequence_header', true, 'sequenceDiagram'),
  simple(
    'state_diagram',
    'state_header',
    true,
    'stateDiagram-v2',
    'stateDiagram-V2',
    'stateDiagram',
  ),
  {
    root: 'swimlane_diagram',
    header: 'swimlane_header',
    rule: ($) => terminatedHeader(
      $,
      'swimlane-beta',
      optional(spacedField(
        $,
        'direction',
        /(?:LR|RL|TB|BT|TD|BR|[<>^v])/,
        $.direction,
      )),
    ),
    eofRule: ($) => seq(
      headerKeyword($, 'swimlane-beta'),
      optional(spacedField(
        $,
        'direction',
        /(?:LR|RL|TB|BT|TD|BR|[<>^v])/,
        $.direction,
      )),
    ),
  },
  {
    root: 'timeline_diagram',
    header: 'timeline_header',
    rule: ($) => terminatedHeader(
      $,
      'timeline',
      optional(spacedField(
        $,
        'direction',
        /(?:LR|TD)/i,
        $.timeline_direction,
      )),
    ),
    eofRule: ($) => seq(
      headerKeyword($, 'timeline'),
      optional(spacedField(
        $,
        'direction',
        /(?:LR|TD)/i,
        $.timeline_direction,
      )),
    ),
  },
  {
    root: 'xy_chart_diagram',
    header: 'xy_chart_header',
    rule: ($) => terminatedHeader(
      $,
      token(prec(20, /xychart(?:-[bB][eE][tT][aA])?/)),
      optional(spacedField(
        $,
        'orientation',
        /(?:vertical|horizontal)/i,
        $.orientation,
      )),
    ),
    eofRule: ($) => seq(
      headerKeyword($, token(prec(20, /xychart(?:-[bB][eE][tT][aA])?/))),
      optional(spacedField(
        $,
        'orientation',
        /(?:vertical|horizontal)/i,
        $.orientation,
      )),
    ),
  },
];

const recognizedFamilyRoots = ($) => [
  ...FAMILY_SPECS.map((spec) => $[spec.root]),
];

const recognizedFamilyRules = Object.fromEntries([
  ...FAMILY_SPECS.flatMap((spec) => [
    [
      spec.root,
      ($) => choice(
        seq(
          field('header', $[spec.header]),
          optional(field('body', $.unstructured_body)),
        ),
        ...(spec.eofRule
          ? [field(
            'header',
            alias($[`_${spec.header}_eof`], $[spec.header]),
          )]
          : []),
      ),
    ],
    [spec.header, spec.rule],
    ...(spec.eofRule ? [[`_${spec.header}_eof`, spec.eofRule]] : []),
  ]),
]);

module.exports = {
  FAMILY_SPECS,
  recognizedFamilyRoots,
  recognizedFamilyRules,
};
