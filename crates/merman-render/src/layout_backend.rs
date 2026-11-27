//! Family-aware Mermaid 12 loader selection, before session capability admission.

use crate::config::ConfigLookup;
use crate::{Error, RenderCapability, Result};
use merman_model::family::DiagramFamilyId;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphLayoutBackend {
    Native,
    Dagre,
    Elk(ElkRootAlgorithm),
    Grid,
    Layered,
    CoseBilkent,
    Shoal,
    Swimlane,
    TidyTree,
}

impl GraphLayoutBackend {
    #[cfg(feature = "benchmark-stages")]
    pub(crate) const fn benchmark_id(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Dagre => "dagre",
            Self::Elk(algorithm) => match algorithm {
                ElkRootAlgorithm::Layered => "elk",
                ElkRootAlgorithm::Stress => "elk.stress",
                ElkRootAlgorithm::Force => "elk.force",
                ElkRootAlgorithm::MrTree => "elk.mrtree",
                ElkRootAlgorithm::SporeOverlap => "elk.sporeOverlap",
                ElkRootAlgorithm::Box => "elk.box",
                ElkRootAlgorithm::Rectpacking => "elk.rectpacking",
            },
            Self::CoseBilkent => "cytoscape",
            Self::Grid => "grid",
            Self::Layered => "layered",
            Self::Shoal => "shoal",
            Self::Swimlane => "swimlane",
            Self::TidyTree => "tidy-tree",
        }
    }

    pub(crate) const fn required_capability(self) -> Option<RenderCapability> {
        match self {
            Self::Native | Self::Swimlane | Self::TidyTree => None,
            Self::Dagre => Some(RenderCapability::LayoutDagre),
            Self::Elk(_) => Some(RenderCapability::LayoutElk),
            Self::Grid => Some(RenderCapability::LayoutGrid),
            Self::Layered => Some(RenderCapability::LayoutLayered),
            Self::CoseBilkent => Some(RenderCapability::LayoutCytoscape),
            Self::Shoal => Some(RenderCapability::LayoutShoal),
        }
    }

    /// The selected CoSE loader requires Mindmap's root node.
    pub(crate) fn validate_rootless_graph(self) -> Result<()> {
        if self == Self::CoseBilkent {
            return Err(Error::InvalidModel {
                message: "Root node is required for the cose-bilkent layout".to_owned(),
            });
        }
        Ok(())
    }

    pub(crate) fn unavailable_for(self, diagram_type: &str) -> Error {
        if let Some(capability) = self.required_capability() {
            Error::MissingCapability {
                capability,
                diagram_type: diagram_type.to_owned(),
            }
        } else {
            Error::InvalidModel {
                message: format!("no {self:?} layout adapter for {diagram_type}"),
            }
        }
    }
}

/// The ELK root loaders registered by Mermaid 12. Containers have their own allowlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElkRootAlgorithm {
    Layered,
    Stress,
    Force,
    MrTree,
    SporeOverlap,
    Box,
    Rectpacking,
}

impl ElkRootAlgorithm {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "elk" => Self::Layered,
            "elk.stress" => Self::Stress,
            "elk.force" => Self::Force,
            "elk.mrtree" => Self::MrTree,
            "elk.sporeOverlap" => Self::SporeOverlap,
            "elk.box" => Self::Box,
            "elk.rectpacking" => Self::Rectpacking,
            _ => return None,
        })
    }

    #[cfg(all(test, feature = "layout-elk"))]
    pub(crate) fn for_diagnostic(config: &(impl ConfigLookup + ?Sized)) -> Self {
        Self::from_name(requested_layout(config)).unwrap_or(Self::Layered)
    }

    #[cfg(feature = "layout-elk")]
    pub(crate) fn algorithm(self) -> merman_layout_elk::Algorithm {
        use merman_layout_elk::Algorithm;
        match self {
            Self::Layered => Algorithm::Layered,
            Self::Stress => Algorithm::Stress,
            Self::Force => Algorithm::Force,
            Self::MrTree => Algorithm::MrTree,
            Self::SporeOverlap => Algorithm::SporeOverlap,
            Self::Box => Algorithm::Box,
            Self::Rectpacking => Algorithm::Rectpacking,
        }
    }
}

fn requested_layout(config: &(impl ConfigLookup + ?Sized)) -> &str {
    config.get("layout").and_then(Value::as_str).unwrap_or("")
}

fn unsupported_mermaid_layout(family: DiagramFamilyId, requested: &str) -> Option<&'static str> {
    let backend = match requested {
        "grid" => "grid",
        "layered" => "layered",
        _ => return None,
    };
    matches!(
        family,
        DiagramFamilyId::AGENTFLOW
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::ER
            | DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::MINDMAP
            | DiagramFamilyId::REQUIREMENT
            | DiagramFamilyId::STATE
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::USECASE
    )
    .then_some(backend)
}

/// A fixed backend is the family route when the benchmark site option has no effect.
/// Unavailable or rejected selectable backends must go through normal admission.
#[cfg(feature = "benchmark-stages")]
pub(crate) fn benchmark_fixed_layout(
    family: DiagramFamilyId,
    requested: &str,
) -> Option<GraphLayoutBackend> {
    use GraphLayoutBackend as Backend;

    if unsupported_mermaid_layout(family, requested).is_some() {
        return None;
    }
    match family {
        DiagramFamilyId::AGENTFLOW
        | DiagramFamilyId::CLASS
        | DiagramFamilyId::ER
        | DiagramFamilyId::FLOWCHART
        | DiagramFamilyId::MINDMAP
        | DiagramFamilyId::REQUIREMENT
        | DiagramFamilyId::STATE
        | DiagramFamilyId::USECASE => None,
        DiagramFamilyId::ARCHITECTURE
        | DiagramFamilyId::BLOCK
        | DiagramFamilyId::C4
        | DiagramFamilyId::SWIMLANE
            if requested == "shoal" =>
        {
            None
        }
        DiagramFamilyId::ARCHITECTURE => Some(Backend::CoseBilkent),
        DiagramFamilyId::SWIMLANE => Some(Backend::Swimlane),
        _ => Some(Backend::Native),
    }
}

pub(crate) fn resolve(
    family: DiagramFamilyId,
    diagram_type: &str,
    config: &(impl ConfigLookup + ?Sized),
) -> Result<GraphLayoutBackend> {
    use GraphLayoutBackend as Backend;

    let requested = requested_layout(config);
    if requested == "layered"
        && crate::layout_layered_available()
        && matches!(
            family,
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::AGENTFLOW | DiagramFamilyId::SWIMLANE
        )
    {
        return Ok(Backend::Layered);
    }
    if requested == "grid"
        && crate::layout_grid_available()
        && matches!(
            family,
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::AGENTFLOW | DiagramFamilyId::SWIMLANE
        )
    {
        return Ok(Backend::Grid);
    }
    if let Some(backend) = unsupported_mermaid_layout(family, requested) {
        return Err(Error::UnsupportedLayoutBackend {
            backend,
            diagram_type: diagram_type.to_owned(),
        });
    }
    if requested == "shoal" {
        match family {
            DiagramFamilyId::ARCHITECTURE
            | DiagramFamilyId::BLOCK
            | DiagramFamilyId::C4
            | DiagramFamilyId::AGENTFLOW
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::ER
            | DiagramFamilyId::REQUIREMENT
            | DiagramFamilyId::STATE
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::USECASE => return Ok(Backend::Shoal),
            DiagramFamilyId::FLOWCHART if matches!(diagram_type, "flowchart" | "flowchart-v2") => {
                return Ok(Backend::Shoal);
            }
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::MINDMAP => {
                return Err(Error::UnsupportedLayoutBackend {
                    backend: "shoal",
                    diagram_type: diagram_type.to_owned(),
                });
            }
            _ => {}
        }
    }

    let elk = ElkRootAlgorithm::from_name(requested);
    Ok(match family {
        DiagramFamilyId::ARCHITECTURE => Backend::CoseBilkent,
        DiagramFamilyId::MINDMAP => match requested {
            "dagre" => Backend::Dagre,
            "swimlane" => Backend::Swimlane,
            "tidy-tree" => Backend::TidyTree,
            _ => match elk {
                Some(algorithm) if crate::layout_elk_available() => Backend::Elk(algorithm),
                _ if crate::layout_cytoscape_available() => Backend::CoseBilkent,
                Some(algorithm) if !crate::layout_dagre_available() => Backend::Elk(algorithm),
                _ => Backend::Dagre,
            },
        },
        DiagramFamilyId::SWIMLANE => Backend::Swimlane,
        DiagramFamilyId::ER if requested == "swimlane" => Backend::Swimlane,
        DiagramFamilyId::AGENTFLOW
        | DiagramFamilyId::CLASS
        | DiagramFamilyId::ER
        | DiagramFamilyId::FLOWCHART
        | DiagramFamilyId::REQUIREMENT
        | DiagramFamilyId::STATE
        | DiagramFamilyId::USECASE => match elk {
            Some(algorithm)
                if crate::layout_elk_available() || !crate::layout_dagre_available() =>
            {
                Backend::Elk(algorithm)
            }
            _ if requested == "cose-bilkent"
                && (crate::layout_cytoscape_available() || !crate::layout_dagre_available()) =>
            {
                Backend::CoseBilkent
            }
            _ => Backend::Dagre,
        },
        _ => Backend::Native,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use GraphLayoutBackend as Backend;
    use serde_json::json;

    fn flowchart(requested: &str) -> Backend {
        resolve(
            DiagramFamilyId::FLOWCHART,
            "flowchart-v2",
            &json!({"layout": requested}),
        )
        .unwrap()
    }

    #[test]
    fn preserves_the_selected_registered_elk_algorithm() {
        for (name, algorithm) in [
            ("elk", ElkRootAlgorithm::Layered),
            ("elk.stress", ElkRootAlgorithm::Stress),
            ("elk.force", ElkRootAlgorithm::Force),
            ("elk.mrtree", ElkRootAlgorithm::MrTree),
            ("elk.sporeOverlap", ElkRootAlgorithm::SporeOverlap),
            ("elk.box", ElkRootAlgorithm::Box),
            ("elk.rectpacking", ElkRootAlgorithm::Rectpacking),
        ] {
            let expected = if crate::layout_elk_available() || !crate::layout_dagre_available() {
                Backend::Elk(algorithm)
            } else {
                Backend::Dagre
            };
            assert_eq!(flowchart(name), expected, "{name}");
        }
        for name in [
            "dagre",
            "elk.layered",
            "elk.radial",
            "elk.sporeoverlap",
            "ELK",
            "unknown",
            "",
        ] {
            assert_eq!(flowchart(name), Backend::Dagre, "{name}");
        }
    }

    #[test]
    fn native_and_rootless_families_apply_their_own_registration() {
        let swimlane = json!({"layout": "swimlane"});
        for (family, diagram_type, expected) in [
            (DiagramFamilyId::ER, "er", Backend::Swimlane),
            (DiagramFamilyId::SWIMLANE, "flowchart-v2", Backend::Swimlane),
            (DiagramFamilyId::FLOWCHART, "flowchart-v2", Backend::Dagre),
            (DiagramFamilyId::BLOCK, "block", Backend::Native),
            (
                DiagramFamilyId::ARCHITECTURE,
                "architecture",
                Backend::CoseBilkent,
            ),
        ] {
            assert_eq!(resolve(family, diagram_type, &swimlane).unwrap(), expected);
        }
        let cose = flowchart("cose-bilkent");
        if crate::layout_cytoscape_available() {
            assert_eq!(cose, Backend::CoseBilkent);
            assert!(
                matches!(cose.validate_rootless_graph(), Err(Error::InvalidModel { message }) if message.contains("Root node is required"))
            );
        } else if crate::layout_dagre_available() {
            assert_eq!(cose, Backend::Dagre);
            cose.validate_rootless_graph().unwrap();
        } else {
            assert_eq!(
                cose.required_capability(),
                Some(RenderCapability::LayoutCytoscape)
            );
        }
    }

    #[test]
    fn pending_mermaid_layouts_apply_only_to_families_with_selectable_layouts() {
        for requested in ["grid", "layered"] {
            let config = json!({"layout": requested});
            for (family, diagram_type) in [
                (DiagramFamilyId::AGENTFLOW, "agentflow"),
                (DiagramFamilyId::CLASS, "classDiagram"),
                (DiagramFamilyId::ER, "er"),
                (DiagramFamilyId::FLOWCHART, "flowchart-v2"),
                (DiagramFamilyId::MINDMAP, "mindmap"),
                (DiagramFamilyId::REQUIREMENT, "requirement"),
                (DiagramFamilyId::STATE, "stateDiagram"),
                (DiagramFamilyId::SWIMLANE, "swimlane"),
                (DiagramFamilyId::USECASE, "usecase"),
            ] {
                let available = match requested {
                    "grid" if crate::layout_grid_available() => Some(Backend::Grid),
                    "layered" if crate::layout_layered_available() => Some(Backend::Layered),
                    _ => None,
                };
                if let Some(backend) = available
                    && matches!(
                        family,
                        DiagramFamilyId::FLOWCHART
                            | DiagramFamilyId::AGENTFLOW
                            | DiagramFamilyId::SWIMLANE
                    )
                {
                    assert_eq!(resolve(family, diagram_type, &config).unwrap(), backend);
                    continue;
                }
                assert!(matches!(
                    resolve(family, diagram_type, &config),
                    Err(Error::UnsupportedLayoutBackend { backend, diagram_type: actual })
                        if backend == requested && actual == diagram_type
                ));
                #[cfg(feature = "benchmark-stages")]
                assert_eq!(benchmark_fixed_layout(family, requested), None);
            }
            assert_eq!(
                resolve(DiagramFamilyId::ARCHITECTURE, "architecture", &config).unwrap(),
                Backend::CoseBilkent
            );
            assert_eq!(
                resolve(DiagramFamilyId::BLOCK, "block", &config).unwrap(),
                Backend::Native
            );
        }
    }
}
