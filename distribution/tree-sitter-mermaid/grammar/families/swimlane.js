// Source translation: Mermaid 11.16.1
// packages/mermaid/src/diagrams/flowchart/parser/flow.jison
// packages/mermaid/src/diagrams/swimlanes/{detector.ts,swimlanesDiagram.ts}
// commit 7ecca0cd7f1658ef74f4e7e91f925724ef403bbf.
//
// Mermaid deliberately routes swimlane-beta through the Flowchart parser and
// database. Reuse the accepted token language while keeping every public CST
// node, including the root and header, owned by the Swimlane family.

const {
  createFlowFamilyConflicts,
  createFlowFamilyRules,
} = require('./flowchart');

const swimlaneRules = createFlowFamilyRules({
  prefix: 'swimlane',
  diagram: 'swimlane_diagram',
  header: 'swimlane_header',
  headerEof: '_swimlane_header_eof',
  keywords: ['swimlane-beta'],
});

const swimlaneConflicts = ($) => createFlowFamilyConflicts($, 'swimlane');

module.exports = { swimlaneConflicts, swimlaneRules };
