; Helix textobjects use the editor's fixed capture vocabulary. A Mermaid
; diagram is the outer structural container, and its family body is the inside
; selection. Direct body children are entry textobjects.

; architecture.
(architecture_diagram
  body: (architecture_body) @class.inside) @class.around

; block.
(block_diagram
  body: (block_body) @class.inside) @class.around

; c4.
(c4_diagram
  body: (c4_body) @class.inside) @class.around

; class.
(class_diagram
  body: (class_body) @class.inside) @class.around

; cynefin.
(cynefin_diagram
  body: (cynefin_body) @class.inside) @class.around

; er.
(entity_relationship_diagram
  body: (entity_relationship_body) @class.inside) @class.around

; eventmodeling.
(event_modeling_diagram
  body: (event_modeling_body) @class.inside) @class.around

; flowchart.
(flowchart_diagram
  body: (flow_body) @class.inside) @class.around

; gantt.
(gantt_diagram
  body: (gantt_body) @class.inside) @class.around

; gitgraph.
(git_graph_diagram
  body: (git_graph_body) @class.inside) @class.around

; info.
(info_diagram
  body: (info_body) @class.inside) @class.around

; ishikawa.
(ishikawa_diagram
  body: (ishikawa_body) @class.inside) @class.around

; journey.
(journey_diagram
  body: (journey_body) @class.inside) @class.around

; kanban.
(kanban_diagram
  body: (kanban_body) @class.inside) @class.around

; mindmap.
(mindmap_diagram
  body: (mindmap_body) @class.inside) @class.around

; packet.
(packet_diagram
  body: (packet_body) @class.inside) @class.around

; pie.
(pie_diagram
  body: (pie_body) @class.inside) @class.around

; quadrantchart.
(quadrant_chart_diagram
  body: (quadrant_chart_body) @class.inside) @class.around

; radar.
(radar_diagram
  body: (radar_body) @class.inside) @class.around

; railroad.
(railroad_diagram
  body: (railroad_body) @class.inside) @class.around

; railroadAbnf.
(railroad_abnf_diagram
  body: (railroad_abnf_body) @class.inside) @class.around

; railroadEbnf.
(railroad_ebnf_diagram
  body: (railroad_ebnf_body) @class.inside) @class.around

; railroadPeg.
(railroad_peg_diagram
  body: (railroad_peg_body) @class.inside) @class.around

; requirement.
(requirement_diagram
  body: (requirement_body) @class.inside) @class.around

; sankey.
(sankey_diagram
  body: (sankey_body) @class.inside) @class.around

; sequence.
(sequence_diagram
  body: (sequence_body) @class.inside) @class.around

; state.
(state_diagram
  body: (state_body) @class.inside) @class.around

; swimlane.
(swimlane_diagram
  body: (swimlane_body) @class.inside) @class.around

; timeline.
(timeline_diagram
  body: (timeline_body) @class.inside) @class.around

; treeView.
(tree_view_diagram
  body: (tree_view_body) @class.inside) @class.around

; treemap.
(treemap_diagram
  body: (treemap_body) @class.inside) @class.around

; venn.
(venn_diagram
  body: (venn_body) @class.inside) @class.around

; wardley.
(wardley_diagram
  body: (wardley_body) @class.inside) @class.around

; xychart.
(xy_chart_diagram
  body: (xy_chart_body) @class.inside) @class.around

; zenuml.
(zenuml_diagram
  body: (zenuml_body) @class.inside) @class.around

; Top-level family statements and records.
(architecture_body
  (_) @entry.around)
(block_body
  (_) @entry.around)
(c4_body
  (_) @entry.around)
(class_body
  (_) @entry.around)
(cynefin_body
  (_) @entry.around)
(entity_relationship_body
  (_) @entry.around)
(event_modeling_body
  (_) @entry.around)
(flow_body
  (_) @entry.around)
(gantt_body
  (_) @entry.around)
(git_graph_body
  (_) @entry.around)
(info_body
  (_) @entry.around)
(ishikawa_body
  (_) @entry.around)
(journey_body
  (_) @entry.around)
(kanban_body
  (_) @entry.around)
(mindmap_body
  (_) @entry.around)
(packet_body
  (_) @entry.around)
(pie_body
  (_) @entry.around)
(quadrant_chart_body
  (_) @entry.around)
(radar_body
  (_) @entry.around)
(railroad_body
  (_) @entry.around)
(railroad_abnf_body
  (_) @entry.around)
(railroad_ebnf_body
  (_) @entry.around)
(railroad_peg_body
  (_) @entry.around)
(requirement_body
  (_) @entry.around)
(sankey_body
  (_) @entry.around)
(sequence_body
  (_) @entry.around)
(state_body
  (_) @entry.around)
(swimlane_body
  (_) @entry.around)
(timeline_body
  (_) @entry.around)
(tree_view_body
  (_) @entry.around)
(treemap_body
  (_) @entry.around)
(venn_body
  (_) @entry.around)
(wardley_body
  (_) @entry.around)
(xy_chart_body
  (_) @entry.around)
(zenuml_body
  (_) @entry.around)

; Railroad rules are function-like definitions.
(railroad_rule
  definition: (railroad_expression) @function.inside) @function.around
(railroad_abnf_rule
  definition: (railroad_abnf_alternation) @function.inside) @function.around
(railroad_ebnf_rule
  definition: (railroad_ebnf_choice) @function.inside) @function.around
(railroad_peg_rule
  definition: (railroad_peg_ordered_choice) @function.inside) @function.around

; Argument entries are Helix parameter textobjects.
(c4_argument) @parameter.around
(zenuml_argument) @parameter.around

; Common and family-owned comments.
[
  (comment)
  (event_line_comment)
  (event_multiline_comment)
  (journey_hash_comment)
  (railroad_abnf_comment)
  (railroad_block_comment)
  (railroad_ebnf_block_comment)
  (railroad_ebnf_iso_comment)
  (railroad_peg_comment)
  (requirement_hash_comment)
  (sequence_hash_comment)
  (state_hash_comment)
  (timeline_hash_comment)
  (zenuml_comment)
] @comment.inside

[
  (comment)
  (event_line_comment)
  (event_multiline_comment)
  (journey_hash_comment)
  (railroad_abnf_comment)
  (railroad_block_comment)
  (railroad_ebnf_block_comment)
  (railroad_ebnf_iso_comment)
  (railroad_peg_comment)
  (requirement_hash_comment)
  (sequence_hash_comment)
  (state_hash_comment)
  (timeline_hash_comment)
  (zenuml_comment)
] @comment.around
