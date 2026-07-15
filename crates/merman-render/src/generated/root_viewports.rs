use crate::family::RenderFamilyKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedRootViewport {
    pub(crate) view_box: &'static str,
    pub(crate) max_width: &'static str,
}

pub(crate) fn lookup_root_viewport_override(
    family: RenderFamilyKind,
    baseline_version: &str,
    diagram_id: &str,
) -> Option<GeneratedRootViewport> {
    if baseline_version != merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION {
        return None;
    }

    let raw = match family {
        RenderFamilyKind::C4 => {
            super::c4_root_overrides_11_12_2::lookup_c4_root_viewport_override(diagram_id)
        }
        RenderFamilyKind::Er => {
            super::er_root_overrides_11_12_2::lookup_er_root_viewport_override(diagram_id)
        }
        RenderFamilyKind::EventModeling => {
            super::eventmodeling_root_overrides_11_15_0::lookup_eventmodeling_root_viewport_override(
                diagram_id,
            )
        }
        RenderFamilyKind::Flowchart => {
            super::flowchart_root_overrides_11_12_2::lookup_flowchart_root_viewport_override(
                diagram_id,
            )
        }
        RenderFamilyKind::Mindmap => {
            #[cfg(feature = "cytoscape-layout")]
            {
                super::mindmap_root_overrides_11_12_2::lookup_mindmap_root_viewport_override(
                    diagram_id,
                )
            }
            #[cfg(not(feature = "cytoscape-layout"))]
            {
                None
            }
        }
        RenderFamilyKind::Pie => {
            super::pie_root_overrides_11_12_2::lookup_pie_root_viewport_override(diagram_id)
        }
        RenderFamilyKind::Sankey => {
            super::sankey_root_overrides_11_12_2::lookup_sankey_root_viewport_override(diagram_id)
        }
        RenderFamilyKind::Sequence => {
            super::sequence_root_overrides_11_16_0::lookup_sequence_root_viewport_override(
                diagram_id,
            )
        }
        RenderFamilyKind::State => {
            super::state_root_overrides_11_12_2::lookup_state_root_viewport_override(diagram_id)
        }
        RenderFamilyKind::Timeline => {
            super::timeline_root_overrides_11_12_2::lookup_timeline_root_viewport_override(
                diagram_id,
            )
        }
        RenderFamilyKind::Error
        | RenderFamilyKind::Architecture
        | RenderFamilyKind::Class
        | RenderFamilyKind::Cynefin
        | RenderFamilyKind::Railroad
        | RenderFamilyKind::Kanban
        | RenderFamilyKind::Gantt
        | RenderFamilyKind::Packet
        | RenderFamilyKind::Journey
        | RenderFamilyKind::Requirement
        | RenderFamilyKind::Radar
        | RenderFamilyKind::Info
        | RenderFamilyKind::Treemap
        | RenderFamilyKind::Block
        | RenderFamilyKind::QuadrantChart
        | RenderFamilyKind::XyChart
        | RenderFamilyKind::GitGraph
        | RenderFamilyKind::TreeView
        | RenderFamilyKind::Ishikawa
        | RenderFamilyKind::Venn => None,
    };

    raw.map(|(view_box, max_width)| GeneratedRootViewport {
        view_box,
        max_width,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED: &str = merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION;
    const C4_KEY: &str = "upstream_pkgtests_c4person_spec_004";

    #[test]
    fn known_key_returns_typed_viewport_for_its_family() {
        let viewport = lookup_root_viewport_override(RenderFamilyKind::C4, PINNED, C4_KEY)
            .expect("known C4 root viewport override");

        assert!(!viewport.view_box.is_empty());
        assert!(!viewport.max_width.is_empty());
    }

    #[test]
    fn key_cannot_cross_family_or_baseline_boundaries() {
        assert_eq!(
            lookup_root_viewport_override(RenderFamilyKind::Er, PINNED, C4_KEY),
            None
        );
        assert_eq!(
            lookup_root_viewport_override(RenderFamilyKind::C4, "11.15.0", C4_KEY),
            None
        );
    }

    #[test]
    fn missing_key_and_family_without_a_table_return_none() {
        assert_eq!(
            lookup_root_viewport_override(RenderFamilyKind::C4, PINNED, "missing-fixture"),
            None
        );
        assert_eq!(
            lookup_root_viewport_override(RenderFamilyKind::Architecture, PINNED, C4_KEY),
            None
        );
    }

    #[test]
    fn sequence_residual_is_scoped_to_sequence_and_the_pinned_baseline() {
        const KEY: &str = "upstream_docs_math_sequence_002";
        let viewport = lookup_root_viewport_override(RenderFamilyKind::Sequence, PINNED, KEY)
            .expect("known Sequence root viewport residual");

        assert_eq!(viewport.view_box, "-50 -10 550 273");
        assert_eq!(viewport.max_width, "550");
        assert_eq!(
            lookup_root_viewport_override(RenderFamilyKind::Flowchart, PINNED, KEY),
            None
        );
        assert_eq!(
            lookup_root_viewport_override(RenderFamilyKind::Sequence, "11.15.0", KEY),
            None
        );
    }

    #[cfg(not(feature = "cytoscape-layout"))]
    #[test]
    fn mindmap_table_is_unavailable_without_cytoscape_layout() {
        assert_eq!(
            lookup_root_viewport_override(RenderFamilyKind::Mindmap, PINNED, "any-fixture"),
            None
        );
    }
}
