use std::fmt;

macro_rules! define_render_families {
    ($($variant:ident => $id:literal),+ $(,)?) => {
        /// Stable identity for a built-in typed render family.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum RenderFamilyKind {
            $($variant),+
        }

        impl RenderFamilyKind {
            /// Returns the complete typed render-family catalog.
            ///
            /// The slice is intentionally returned instead of exposing a fixed-size array so
            /// callers can iterate the catalog without coupling their public API to its length.
            pub const fn all() -> &'static [Self] {
                &[$(Self::$variant),+]
            }

            #[cfg(test)]
            pub(crate) const ALL: &'static [Self] = Self::all();

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $id),+
                }
            }

            /// Parses one stable render-family identifier from the public wire form.
            pub fn from_id(value: &str) -> Option<Self> {
                match value {
                    $($id => Some(Self::$variant),)+
                    _ => None,
                }
            }

            pub(crate) fn from_str(value: &str) -> Option<Self> {
                Self::from_id(value)
            }
        }
    };
}

define_render_families! {
    Error => "error",
    Mindmap => "mindmap",
    State => "state",
    Sequence => "sequence",
    Zenuml => "zenuml",
    Flowchart => "flowchart",
    Swimlane => "swimlane",
    Architecture => "architecture",
    Class => "class",
    C4 => "c4",
    Cynefin => "cynefin",
    Wardley => "wardley",
    Railroad => "railroad",
    Kanban => "kanban",
    Gantt => "gantt",
    Pie => "pie",
    Packet => "packet",
    Timeline => "timeline",
    Journey => "journey",
    Requirement => "requirement",
    Sankey => "sankey",
    Radar => "radar",
    Info => "info",
    Treemap => "treemap",
    Block => "block",
    Er => "er",
    QuadrantChart => "quadrantChart",
    XyChart => "xychart",
    GitGraph => "gitGraph",
    TreeView => "treeView",
    Ishikawa => "ishikawa",
    EventModeling => "eventmodeling",
    Venn => "venn",
}

impl fmt::Display for RenderFamilyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::RenderFamilyKind;

    #[test]
    fn family_catalog_round_trips_every_id() {
        for family in RenderFamilyKind::ALL {
            assert_eq!(RenderFamilyKind::from_str(family.as_str()), Some(*family));
        }
    }
}
