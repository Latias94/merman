// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/flowchart/parser/flow.jison
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.

const familyRule = (prefix, suffix) => `${prefix}_${suffix}`;
const privateFamilyRule = (prefix, suffix) => `_${familyRule(prefix, suffix)}`;

const ref = ($, prefix, suffix) => $[familyRule(prefix, suffix)];
const privateRef = ($, prefix, suffix) => $[privateFamilyRule(prefix, suffix)];

const diagramKeyword = ($, keywords) => field(
  'keyword',
  alias(
    token(prec(40, keywords.length === 1 ? keywords[0] : choice(...keywords))),
    $.diagram_keyword,
  ),
);

const statementKeyword = ($, prefix, keyword) => field(
  'keyword',
  alias(
    token(prec(keyword === 'classDef' ? 50 : 30, keyword)),
    ref($, prefix, 'statement_keyword'),
  ),
);

const optionalInlineGap = () => optional(token.immediate(/[ \t]+/));

const keywordPrefixedIdentifier = () => choice(
  token(prec(
    60,
    /classDef(?:[A-Za-z0-9_.!?$%#\u00c0-\uffff]|-[A-Za-z0-9_\u00c0-\uffff])+/
  )),
  token(prec(
    40,
    /(?:subgraph|direction|class|style|linkStyle|click|accTitle|accDescr|end)(?:[A-Za-z0-9_.!?$%#\u00c0-\uffff]|-[A-Za-z0-9_\u00c0-\uffff])+/
  )),
);

const headerDirection = ($) => seq(
  token.immediate(/[ \t]+/),
  field(
    'direction',
    alias(
      token.immediate(/(?:LR|RL|TB|BT|TD|BR|[<>^v])/),
      $.direction,
    ),
  ),
);

const shapeDelimiter = ($, prefix, delimiter) => alias(
  delimiter,
  ref($, prefix, 'shape_delimiter'),
);

const shape = ($, prefix, open, close, textRule) => seq(
  field('open', shapeDelimiter($, prefix, open)),
  optional(field('label', choice(
    ref($, prefix, 'markdown_label'),
    ref($, prefix, 'quoted_label'),
    ref($, prefix, textRule),
  ))),
  field('close', shapeDelimiter($, prefix, close)),
);

const quotedText = () => token(prec(30, choice(
  seq('"', /(?:[^"\\]|\\.)*/, '"'),
  seq("'", /(?:[^'\\]|\\.)*/, "'"),
)));

const immediateQuotedText = () => token.immediate(prec(30, choice(
  seq('"', /(?:[^"\\]|\\.)*/, '"'),
  seq("'", /(?:[^'\\]|\\.)*/, "'"),
)));

const clickSuffix = ($, prefix) => seq(
  token.immediate(/[ \t]+/),
  choice(
    seq(
      field('tooltip', alias(
        immediateQuotedText(),
        ref($, prefix, 'quoted_label'),
      )),
      optional(seq(
        token.immediate(/[ \t]+/),
        field('link_target', alias(
          token.immediate(choice('_self', '_blank', '_parent', '_top')),
          ref($, prefix, 'link_target'),
        )),
      )),
    ),
    field('link_target', alias(
      token.immediate(choice('_self', '_blank', '_parent', '_top')),
      ref($, prefix, 'link_target'),
    )),
  ),
);

const clickTooltip = ($, prefix) => seq(
  token.immediate(/[ \t]+/),
  field('tooltip', alias(
    immediateQuotedText(),
    ref($, prefix, 'quoted_label'),
  )),
);

const markdownText = () => token(prec(40, seq('"`', /[^`]*/, '`"')));

const createFlowFamilyRules = ({
  prefix,
  diagram,
  header,
  headerEof,
  keywords,
}) => ({
  [diagram]: ($) => choice(
    seq(
      field('header', $[header]),
      optional(field('body', ref($, prefix, 'body'))),
    ),
    field('header', alias($[headerEof], $[header])),
  ),

  [header]: ($) => prec(50, seq(
    diagramKeyword($, keywords),
    choice(
      seq(
        headerDirection($),
        optional(choice(
          field('comment', $.comment),
          field('directive', $.directive),
        )),
        field('terminator', $._statement_terminator),
      ),
      seq(
        optional(choice(
          field('comment', $.comment),
          field('directive', $.directive),
        )),
        field('terminator', $._line_ending),
      ),
    ),
  )),

  [headerEof]: ($) => seq(
    diagramKeyword($, keywords),
    optional(headerDirection($)),
    $._end_of_input,
  ),

  [familyRule(prefix, 'body')]: ($) => choice(
    repeat1(ref($, prefix, 'line_item')),
    seq(
      repeat(ref($, prefix, 'line_item')),
      ref($, prefix, 'eof_item'),
    ),
  ),

  [familyRule(prefix, 'line_item')]: ($) => choice(
    seq(
      field('statement', choice(
        ref($, prefix, 'statement'),
        ref($, prefix, 'incomplete_edge_statement'),
      )),
      field('terminator', $._statement_terminator),
    ),
    seq(choice($.comment, $.directive), $._line_ending),
    $._blank_line,
  ),

  [familyRule(prefix, 'eof_item')]: ($) => choice(
    field('statement', choice(
      ref($, prefix, 'statement'),
      ref($, prefix, 'incomplete_edge_statement'),
    )),
    $.comment,
    $.directive,
  ),

  [familyRule(prefix, 'statement')]: ($) => choice(
    ref($, prefix, 'subgraph'),
    ref($, prefix, 'edge_statement'),
    ref($, prefix, 'node_statement'),
    ref($, prefix, 'direction_statement'),
    ref($, prefix, 'class_definition_statement'),
    ref($, prefix, 'class_assignment_statement'),
    ref($, prefix, 'style_statement'),
    ref($, prefix, 'link_style_statement'),
    ref($, prefix, 'click_statement'),
    ref($, prefix, 'accessibility_title_statement'),
    ref($, prefix, 'accessibility_description_statement'),
  ),

  [familyRule(prefix, 'subgraph')]: ($) => prec.right(50, seq(
    statementKeyword($, prefix, 'subgraph'),
    optional(field('identity', choice(
      seq(
        field('id', alias(ref($, prefix, 'identifier'), ref($, prefix, 'node_id'))),
        field('label', ref($, prefix, 'square_label')),
      ),
      field('name', ref($, prefix, 'subgraph_name')),
    ))),
    field('terminator', $._statement_terminator),
    repeat(ref($, prefix, 'line_item')),
    field('end', ref($, prefix, 'subgraph_end')),
  )),

  [familyRule(prefix, 'subgraph_name')]: ($) => choice(
    ref($, prefix, 'identifier'),
    ref($, prefix, 'markdown_label'),
    ref($, prefix, 'quoted_label'),
    ref($, prefix, 'subgraph_title'),
  ),

  [familyRule(prefix, 'subgraph_title')]: (_) => token(prec(
    10,
    /[^;%\[\]\s\r\n](?:[^;%\[\]\r\n]*[ \t][^;%\[\]\s\r\n][^;%\[\]\r\n]*)/,
  )),

  [familyRule(prefix, 'subgraph_end')]: (_) => token(prec(30, 'end')),

  [familyRule(prefix, 'direction_statement')]: ($) => seq(
    statementKeyword($, prefix, 'direction'),
    token.immediate(/[ \t]+/),
    field('direction', alias(
      token.immediate(/(?:LR|RL|TB|BT|TD)/),
      ref($, prefix, 'direction'),
    )),
  ),

  [familyRule(prefix, 'edge_statement')]: ($) => prec.right(60, seq(
    field('source', ref($, prefix, 'node')),
    field('edge', ref($, prefix, 'edge')),
    optionalInlineGap(),
    field('target', ref($, prefix, 'node')),
    repeat(seq(
      field('edge', ref($, prefix, 'edge')),
      optionalInlineGap(),
      field('target', ref($, prefix, 'node')),
    )),
  )),

  [familyRule(prefix, 'incomplete_edge_statement')]: ($) => prec(-30, seq(
    field('source', ref($, prefix, 'node')),
    field('edge', ref($, prefix, 'edge')),
  )),

  [familyRule(prefix, 'node_statement')]: ($) => field(
    'node',
    ref($, prefix, 'node'),
  ),

  [familyRule(prefix, 'node')]: ($) => seq(
    field('vertex', ref($, prefix, 'vertex')),
    repeat(seq(
      field('separator', '&'),
      field('vertex', ref($, prefix, 'vertex')),
    )),
  ),

  [familyRule(prefix, 'vertex')]: ($) => seq(
    field('id', alias(ref($, prefix, 'identifier'), ref($, prefix, 'node_id'))),
    optional(field('shape', ref($, prefix, 'shape'))),
    optional(field('data', ref($, prefix, 'shape_data'))),
    optional(field('class', ref($, prefix, 'class_annotation'))),
  ),

  [familyRule(prefix, 'shape')]: ($) => choice(
    ref($, prefix, 'square_label'),
    ref($, prefix, 'round_label'),
    ref($, prefix, 'circle_label'),
    ref($, prefix, 'ellipse_label'),
    ref($, prefix, 'stadium_label'),
    ref($, prefix, 'subroutine_label'),
    ref($, prefix, 'property_label'),
    ref($, prefix, 'cylinder_label'),
    ref($, prefix, 'double_circle_label'),
    ref($, prefix, 'diamond_label'),
    ref($, prefix, 'hexagon_label'),
    ref($, prefix, 'odd_label'),
    ref($, prefix, 'slash_label'),
    ref($, prefix, 'backslash_label'),
  ),

  [familyRule(prefix, 'square_label')]: ($) => shape($, prefix, '[', ']', 'square_label_text'),
  [familyRule(prefix, 'round_label')]: ($) => shape($, prefix, '(', ')', 'round_label_text'),
  [familyRule(prefix, 'circle_label')]: ($) => shape($, prefix, '((', '))', 'round_label_text'),
  [familyRule(prefix, 'ellipse_label')]: ($) => shape($, prefix, '(-', '-)', 'ellipse_label_text'),
  [familyRule(prefix, 'stadium_label')]: ($) => shape($, prefix, '([', '])', 'square_label_text'),
  [familyRule(prefix, 'subroutine_label')]: ($) => shape($, prefix, '[[', ']]', 'square_label_text'),
  [familyRule(prefix, 'cylinder_label')]: ($) => shape($, prefix, '[(', ')]', 'round_label_text'),
  [familyRule(prefix, 'double_circle_label')]: ($) => shape($, prefix, '(((', ')))', 'round_label_text'),
  [familyRule(prefix, 'diamond_label')]: ($) => shape($, prefix, '{', '}', 'curly_label_text'),
  [familyRule(prefix, 'hexagon_label')]: ($) => shape($, prefix, '{{', '}}', 'curly_label_text'),
  [familyRule(prefix, 'odd_label')]: ($) => seq(
    optional(token.immediate('-')),
    shape($, prefix, '>', ']', 'square_label_text'),
  ),
  [familyRule(prefix, 'slash_label')]: ($) => shape(
    $,
    prefix,
    '[/',
    choice('\\]', '/]'),
    'trap_label_text',
  ),
  [familyRule(prefix, 'backslash_label')]: ($) => shape(
    $,
    prefix,
    '[\\',
    choice('/]', '\\]'),
    'trap_label_text',
  ),

  [familyRule(prefix, 'square_label_text')]: (_) => token(prec(-10, /[^\]\r\n]+/)),
  [familyRule(prefix, 'round_label_text')]: (_) => token(prec(-10, /[^)\r\n]+/)),
  [familyRule(prefix, 'curly_label_text')]: (_) => token(prec(-10, /[^}\r\n]+/)),
  [familyRule(prefix, 'ellipse_label_text')]: ($) => repeat1(choice(
    ref($, prefix, 'ellipse_label_fragment'),
    token.immediate('-'),
  )),
  [familyRule(prefix, 'ellipse_label_fragment')]: (_) => token.immediate(/[^-)\r\n]+/),
  [familyRule(prefix, 'trap_label_text')]: ($) => repeat1(choice(
    ref($, prefix, 'trap_label_fragment'),
    token.immediate('/'),
    token.immediate('\\'),
  )),
  [familyRule(prefix, 'trap_label_fragment')]: (_) => token.immediate(/[^/\\\]\r\n]+/),

  [familyRule(prefix, 'property_label')]: ($) => seq(
    field('open', shapeDelimiter($, prefix, '[|')),
    field('property', ref($, prefix, 'property_pair')),
    field('separator', '|'),
    optional(field('label', ref($, prefix, 'label'))),
    field('close', shapeDelimiter($, prefix, ']')),
  ),

  [familyRule(prefix, 'property_pair')]: ($) => seq(
    field('name', alias(ref($, prefix, 'identifier'), ref($, prefix, 'property_name'))),
    field('delimiter', ':'),
    field('value', ref($, prefix, 'property_value')),
  ),

  [familyRule(prefix, 'property_value')]: (_) => token(prec(-10, /[^|\]\r\n]+/)),

  [familyRule(prefix, 'label')]: ($) => choice(
    ref($, prefix, 'markdown_label'),
    ref($, prefix, 'quoted_label'),
    ref($, prefix, 'label_text'),
  ),

  [familyRule(prefix, 'markdown_label')]: (_) => markdownText(),
  [familyRule(prefix, 'quoted_label')]: (_) => quotedText(),
  [familyRule(prefix, 'label_text')]: (_) => token(prec(
    -10,
    /[^"'`\[\](){}|\r\n]+/,
  )),

  [familyRule(prefix, 'shape_data')]: ($) => seq(
    field('open', '@{'),
    repeat(field('content', choice(
      ref($, prefix, 'shape_data_string'),
      ref($, prefix, 'shape_data_content'),
    ))),
    field('close', '}'),
  ),

  [familyRule(prefix, 'shape_data_string')]: (_) => token(prec(30, choice(
    seq('"', /(?:[^"\\]|\\.)*/, '"'),
    seq("'", /(?:[^'\\]|\\.)*/, "'"),
  ))),
  [familyRule(prefix, 'shape_data_content')]: (_) => token(prec(-20, /[^}"']+/)),

  [familyRule(prefix, 'edge')]: ($) => prec.right(choice(
    prec(50, seq(
      optional(field('id', ref($, prefix, 'edge_id'))),
      field('operator', ref($, prefix, 'arrow_start')),
      field('label', ref($, prefix, 'middle_edge_label')),
      field('operator_end', ref($, prefix, 'arrow')),
    )),
    prec(50, seq(
      field('operator', ref($, prefix, 'continued_arrow_start')),
      field('label', ref($, prefix, 'middle_edge_label')),
      field('operator_end', ref($, prefix, 'arrow')),
    )),
    privateRef($, prefix, 'labeled_edge'),
    privateRef($, prefix, 'continued_labeled_edge'),
    privateRef($, prefix, 'unlabeled_edge'),
    privateRef($, prefix, 'continued_unlabeled_edge'),
  )),

  [privateFamilyRule(prefix, 'labeled_edge')]: ($) => seq(
    optional(field('id', ref($, prefix, 'edge_id'))),
    field('operator', ref($, prefix, 'arrow')),
    choice(
      seq(
        token.immediate(/[ \t]+/),
        field('label', ref($, prefix, 'edge_label')),
      ),
      field('label', ref($, prefix, 'edge_label')),
    ),
  ),

  [privateFamilyRule(prefix, 'continued_labeled_edge')]: ($) => seq(
    field('operator', ref($, prefix, 'continued_arrow')),
    choice(
      seq(
        token.immediate(/[ \t]+/),
        field('label', ref($, prefix, 'edge_label')),
      ),
      field('label', ref($, prefix, 'edge_label')),
    ),
  ),

  [privateFamilyRule(prefix, 'unlabeled_edge')]: ($) => seq(
    optional(field('id', ref($, prefix, 'edge_id'))),
    field('operator', ref($, prefix, 'arrow')),
  ),

  [privateFamilyRule(prefix, 'continued_unlabeled_edge')]: ($) => field(
    'operator',
    ref($, prefix, 'continued_arrow'),
  ),

  [familyRule(prefix, 'edge_id')]: ($) => seq(
    field('name', alias(ref($, prefix, 'identifier'), ref($, prefix, 'edge_name'))),
    field('delimiter', '@'),
  ),

  [familyRule(prefix, 'edge_label')]: ($) => seq(
    field('open', '|'),
    optional(field('text', choice(
      ref($, prefix, 'markdown_label'),
      ref($, prefix, 'quoted_label'),
      ref($, prefix, 'edge_label_text'),
    ))),
    field('close', '|'),
  ),

  [familyRule(prefix, 'edge_label_text')]: (_) => token(prec(-10, /[^|\r\n]+/)),

  [familyRule(prefix, 'middle_edge_label')]: ($) => choice(
    ref($, prefix, 'markdown_label'),
    ref($, prefix, 'quoted_label'),
    ref($, prefix, 'middle_edge_label_text'),
  ),

  [familyRule(prefix, 'middle_edge_label_text')]: ($) => repeat1(choice(
    ref($, prefix, 'middle_edge_label_fragment'),
    token.immediate('-'),
    token.immediate('='),
    token.immediate('.'),
  )),
  [familyRule(prefix, 'middle_edge_label_fragment')]: (_) => token.immediate(/[^-=.\r\n]+/),

  [familyRule(prefix, 'arrow_start')]: (_) => token(prec(
    20,
    /[xo<]?(?:--|==|-\.)/,
  )),

  [familyRule(prefix, 'continued_arrow_start')]: (_) => token(prec(
    25,
    /(?:\r\n|\n|\r)[ \t]*[xo<]?(?:--|==|-\.)/,
  )),

  [familyRule(prefix, 'arrow')]: (_) => token(prec(
    20,
    /[xo<]?(?:--+[-xo>]|==+[=xo>]|-?\.+-[xo>]?|~~~+)/,
  )),

  [familyRule(prefix, 'continued_arrow')]: (_) => token(prec(
    25,
    /(?:\r\n|\n|\r)[ \t]*[xo<]?(?:--+[-xo>]|==+[=xo>]|-?\.+-[xo>]?|~~~+)/,
  )),

  [familyRule(prefix, 'class_annotation')]: ($) => seq(
    field('operator', ':::'),
    field('name', alias(ref($, prefix, 'identifier'), ref($, prefix, 'class_name'))),
  ),

  [familyRule(prefix, 'class_definition_statement')]: ($) => seq(
    statementKeyword($, prefix, 'classDef'),
    token.immediate(/[ \t]+/),
    field('classes', ref($, prefix, 'identifier_list')),
    optional(seq(
      token.immediate(/[ \t]+/),
      field('style', ref($, prefix, 'style_list')),
    )),
  ),

  [familyRule(prefix, 'class_assignment_statement')]: ($) => seq(
    statementKeyword($, prefix, 'class'),
    token.immediate(/[ \t]+/),
    field('targets', ref($, prefix, 'identifier_list')),
    token.immediate(/[ \t]+/),
    field('class', alias(ref($, prefix, 'identifier'), ref($, prefix, 'class_name'))),
  ),

  [familyRule(prefix, 'style_statement')]: ($) => seq(
    statementKeyword($, prefix, 'style'),
    token.immediate(/[ \t]+/),
    field('target', alias(ref($, prefix, 'identifier'), ref($, prefix, 'node_id'))),
    token.immediate(/[ \t]+/),
    field('style', ref($, prefix, 'style_list')),
  ),

  [familyRule(prefix, 'link_style_statement')]: ($) => seq(
    statementKeyword($, prefix, 'linkStyle'),
    field('targets', choice(
      alias(token(prec(20, 'default')), ref($, prefix, 'link_style_default')),
      ref($, prefix, 'number_list'),
    )),
    optional(seq(
      statementKeyword($, prefix, 'interpolate'),
      field('interpolation', alias(
        ref($, prefix, 'identifier'),
        ref($, prefix, 'interpolation'),
      )),
    )),
    optional(field('style', ref($, prefix, 'style_list'))),
  ),

  [familyRule(prefix, 'identifier_list')]: ($) => seq(
    field('item', alias(ref($, prefix, 'identifier'), ref($, prefix, 'reference'))),
    repeat(seq(
      field('delimiter', ','),
      field('item', alias(ref($, prefix, 'identifier'), ref($, prefix, 'reference'))),
    )),
  ),

  [familyRule(prefix, 'number_list')]: ($) => seq(
    field('item', alias(/[0-9]+/, ref($, prefix, 'edge_index'))),
    repeat(seq(
      field('delimiter', ','),
      field('item', alias(/[0-9]+/, ref($, prefix, 'edge_index'))),
    )),
  ),

  [familyRule(prefix, 'style_list')]: ($) => seq(
    field('item', ref($, prefix, 'style_item')),
    repeat(seq(
      optional(field('delimiter', ',')),
      field('item', ref($, prefix, 'style_item')),
    )),
    optional(field('delimiter', ',')),
  ),

  [familyRule(prefix, 'style_item')]: ($) => choice(
    prec(30, ref($, prefix, 'style_declaration')),
    field('value', alias(
      ref($, prefix, 'style_property'),
      ref($, prefix, 'style_fragment'),
    )),
    ref($, prefix, 'style_fragment'),
  ),

  [familyRule(prefix, 'style_declaration')]: ($) => seq(
    field('property', ref($, prefix, 'style_property')),
    field('delimiter', ':'),
    field('value', ref($, prefix, 'style_value')),
  ),

  [familyRule(prefix, 'style_property')]: (_) => token(prec(10, /[A-Za-z_-][A-Za-z0-9_-]*/)),
  [familyRule(prefix, 'style_value')]: (_) => token(prec(-20, /[^,;\r\n]+/)),
  [familyRule(prefix, 'style_fragment')]: (_) => token(prec(-40, /[^,;\r\n]+/)),

  [familyRule(prefix, 'click_statement')]: ($) => seq(
    statementKeyword($, prefix, 'click'),
    token.immediate(/[ \t]+/),
    field('target', alias(ref($, prefix, 'identifier'), ref($, prefix, 'node_id'))),
    optional(choice(
      seq(
        token.immediate(/[ \t]+/),
        field('action', ref($, prefix, 'href_action')),
        optional(clickSuffix($, prefix)),
      ),
      seq(
        token.immediate(/[ \t]+/),
        field('action', ref($, prefix, 'call_action')),
        optional(clickTooltip($, prefix)),
      ),
      seq(
        token.immediate(/[ \t]+/),
        field('action', ref($, prefix, 'callback_action')),
        optional(clickTooltip($, prefix)),
      ),
      seq(
        token.immediate(/[ \t]+/),
        field('action', ref($, prefix, 'quoted_label')),
        optional(clickSuffix($, prefix)),
      ),
    )),
  ),

  [familyRule(prefix, 'href_action')]: ($) => seq(
    statementKeyword($, prefix, 'href'),
    token.immediate(/[ \t]+/),
    field('url', ref($, prefix, 'quoted_label')),
  ),

  [familyRule(prefix, 'call_action')]: ($) => seq(
    optional(field('keyword', ref($, prefix, 'call_keyword'))),
    field('function', alias(
      ref($, prefix, 'identifier'),
      ref($, prefix, 'callback_name'),
    )),
    field('arguments', ref($, prefix, 'argument_list')),
  ),

  [familyRule(prefix, 'call_keyword')]: ($) => alias(
    token(prec(30, /call[ \t]+/)),
    ref($, prefix, 'statement_keyword'),
  ),

  [familyRule(prefix, 'callback_action')]: ($) => field(
    'function',
    alias(ref($, prefix, 'identifier'), ref($, prefix, 'callback_name')),
  ),

  [familyRule(prefix, 'argument_list')]: ($) => seq(
    field('open', '('),
    optional(field('value', ref($, prefix, 'arguments'))),
    field('close', ')'),
  ),

  [familyRule(prefix, 'arguments')]: (_) => token(prec(-10, /[^)\r\n]+/)),
  [familyRule(prefix, 'link_target')]: (_) => choice('_self', '_blank', '_parent', '_top'),

  [familyRule(prefix, 'accessibility_title_statement')]: ($) => seq(
    statementKeyword($, prefix, 'accTitle'),
    field('delimiter', ':'),
    optional(field('text', ref($, prefix, 'accessibility_text'))),
  ),

  [familyRule(prefix, 'accessibility_description_statement')]: ($) => seq(
    statementKeyword($, prefix, 'accDescr'),
    choice(
      seq(
        field('delimiter', ':'),
        optional(field('text', ref($, prefix, 'accessibility_text'))),
      ),
      field('description', ref($, prefix, 'accessibility_description_block')),
    ),
  ),

  [familyRule(prefix, 'accessibility_text')]: (_) => token(prec(-10, /[^;%\r\n]+/)),

  [familyRule(prefix, 'accessibility_description_block')]: ($) => seq(
    field('open', '{'),
    optional(field('text', ref($, prefix, 'accessibility_block_text'))),
    field('close', token.immediate('}')),
  ),

  [familyRule(prefix, 'accessibility_block_text')]: (_) => token.immediate(/[^}]+/),

  [familyRule(prefix, 'identifier')]: (_) => choice(
    keywordPrefixedIdentifier(),
    token(prec(
      -5,
      /[A-Za-z0-9_\u00c0-\uffff][A-Za-z0-9_.!?$%#\u00c0-\uffff]*(?:-[A-Za-z0-9_\u00c0-\uffff][A-Za-z0-9_.!?$%#\u00c0-\uffff]*)*/,
    )),
  ),
});

const createFlowFamilyConflicts = ($, prefix) => [
  [
    privateRef($, prefix, 'labeled_edge'),
    privateRef($, prefix, 'unlabeled_edge'),
  ],
  [
    privateRef($, prefix, 'continued_labeled_edge'),
    privateRef($, prefix, 'continued_unlabeled_edge'),
  ],
];

const flowchartRules = createFlowFamilyRules({
  prefix: 'flow',
  diagram: 'flowchart_diagram',
  header: 'flowchart_header',
  headerEof: '_flowchart_header_eof',
  keywords: ['flowchart-elk', 'flowchart', 'graph'],
});

const flowchartConflicts = ($) => createFlowFamilyConflicts($, 'flow');

module.exports = {
  createFlowFamilyConflicts,
  createFlowFamilyRules,
  flowchartConflicts,
  flowchartRules,
};
