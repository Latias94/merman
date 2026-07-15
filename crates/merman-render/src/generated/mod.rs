// This module contains generated and fixture-derived Mermaid parity data.
//
// Note: several generated filenames still carry historical `11_12_2` or `11_15_0`
// suffixes. Those suffixes are storage-era provenance labels, not the active upstream
// contract. The repository's active Mermaid baseline is 11.16.0; existing modules
// stay in place until each family is regenerated and renamed in a controlled migration.

pub mod block_text_overrides_11_12_2;
mod c4_root_overrides_11_12_2;
pub mod c4_text_overrides_11_12_2;
pub mod class_text_overrides_11_12_2;
mod er_root_overrides_11_12_2;
pub mod er_text_overrides_11_12_2;
mod eventmodeling_root_overrides_11_15_0;
mod flowchart_root_overrides_11_12_2;
pub mod flowchart_text_overrides_11_12_2;
pub mod font_metrics_flowchart_11_12_2;
#[cfg(feature = "cytoscape-layout")]
mod mindmap_root_overrides_11_12_2;
mod pie_root_overrides_11_12_2;
mod root_viewports;
mod sankey_root_overrides_11_12_2;
pub mod sequence_calculate_text_font_metrics_11_16_0;
mod sequence_root_overrides_11_16_0;
mod state_root_overrides_11_12_2;
pub mod state_text_overrides_11_12_2;
pub mod svg_overrides_sequence_11_16_0;
mod timeline_root_overrides_11_12_2;
pub mod timeline_text_overrides_11_12_2;

pub(crate) use root_viewports::lookup_root_viewport_override;
