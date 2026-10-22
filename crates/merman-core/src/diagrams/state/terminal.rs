#[cfg(feature = "presentation")]
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use serde_json::Value;

#[cfg(feature = "presentation")]
use super::complexity::{FlatDocuments, StateModelFields, StateRecords};
use super::{StateDiagramModel, StateEdge, StateNode};
#[cfg(feature = "presentation")]
use super::{StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode};
use crate::resources::ModelComplexity;

#[derive(Clone, Copy, Debug)]
enum StateTerminalOwner<'a> {
    Semantic(&'a StateDiagramModel),
    #[cfg(feature = "presentation")]
    Render(&'a StateDiagramRenderModel),
}

/// Paired borrowed State owner for terminal graph and complete fallback consumers.
///
/// Serialization retains the flat statement document and document references in state records.
/// Full owners retain their typed node and edge fields. Unlike Mermaid compatibility JSON,
/// this shape does not duplicate nested documents or synthesize config, look, or theme values.
#[derive(Clone, Copy, Debug)]
pub struct StateTerminalView<'a> {
    owner: StateTerminalOwner<'a>,
}

impl<'a> StateTerminalView<'a> {
    pub fn from_semantic(model: &'a StateDiagramModel) -> Self {
        Self {
            owner: StateTerminalOwner::Semantic(model),
        }
    }

    #[cfg(feature = "presentation")]
    pub fn from_render(model: &'a StateDiagramRenderModel) -> Self {
        Self {
            owner: StateTerminalOwner::Render(model),
        }
    }

    pub fn direction(self) -> &'a str {
        match self.owner {
            StateTerminalOwner::Semantic(model) => &model.direction,
            #[cfg(feature = "presentation")]
            StateTerminalOwner::Render(model) => &model.direction,
        }
    }

    pub fn nodes(self) -> StateTerminalNodes<'a> {
        match self.owner {
            StateTerminalOwner::Semantic(model) => StateTerminalNodes::Semantic(&model.nodes),
            #[cfg(feature = "presentation")]
            StateTerminalOwner::Render(model) => StateTerminalNodes::Render(&model.nodes),
        }
    }

    pub fn edges(self) -> StateTerminalEdges<'a> {
        match self.owner {
            StateTerminalOwner::Semantic(model) => StateTerminalEdges::Semantic(&model.edges),
            #[cfg(feature = "presentation")]
            StateTerminalOwner::Render(model) => StateTerminalEdges::Render(&model.edges),
        }
    }

    /// Counts this complete serialized shape, retaining the authored logical nesting bound.
    pub fn complexity(self) -> ModelComplexity {
        let mut complexity = ModelComplexity::from_serializable(&self);
        let document = match self.owner {
            StateTerminalOwner::Semantic(model) => &model.document,
            #[cfg(feature = "presentation")]
            StateTerminalOwner::Render(model) => &model.document,
        };
        complexity.nesting_depth = complexity
            .nesting_depth
            .max(document.semantic_nesting_depth());
        complexity
    }
}

impl Serialize for StateTerminalView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.owner {
            StateTerminalOwner::Semantic(model) => model.terminal_fields().serialize(serializer),
            #[cfg(feature = "presentation")]
            StateTerminalOwner::Render(model) => StateModelFields {
                document: FlatDocuments(&model.document),
                direction: &model.direction,
                acc_title: &model.acc_title,
                acc_descr: &model.acc_descr,
                nodes: FullStateNodes(&model.nodes),
                edges: &model.edges,
                relations: &model.relations,
                links: &model.links,
                states: StateRecords {
                    states: &model.states,
                    include_documents: true,
                },
                style_classes: &model.style_classes,
            }
            .serialize(serializer),
        }
    }
}

#[cfg(feature = "presentation")]
struct FullStateNodes<'a>(&'a [StateDiagramRenderNode]);

#[cfg(feature = "presentation")]
impl Serialize for FullStateNodes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for node in self.0 {
            sequence.serialize_element(&FullStateNode {
                node,
                explicit_dir: node.explicit_dir,
            })?;
        }
        sequence.end()
    }
}

#[cfg(feature = "presentation")]
#[derive(Serialize)]
struct FullStateNode<'a> {
    #[serde(flatten)]
    node: &'a StateDiagramRenderNode,
    // Detached full models may have authored direction provenance without a document.
    #[serde(rename = "explicitDir", skip_serializing_if = "Option::is_none")]
    explicit_dir: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
pub enum StateTerminalNodes<'a> {
    Semantic(&'a [StateNode]),
    #[cfg(feature = "presentation")]
    Render(&'a [StateDiagramRenderNode]),
}

impl<'a> StateTerminalNodes<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Semantic(nodes) => nodes.len(),
            #[cfg(feature = "presentation")]
            Self::Render(nodes) => nodes.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn get(self, index: usize) -> Option<StateTerminalNode<'a>> {
        match self {
            Self::Semantic(nodes) => nodes.get(index).map(StateTerminalNode::from_semantic),
            #[cfg(feature = "presentation")]
            Self::Render(nodes) => nodes.get(index).map(StateTerminalNode::from_render),
        }
    }

    pub fn iter(self) -> impl Iterator<Item = StateTerminalNode<'a>> + Clone {
        let semantic = match self {
            Self::Semantic(nodes) => nodes,
            #[cfg(feature = "presentation")]
            Self::Render(_) => &[],
        };
        #[cfg(feature = "presentation")]
        {
            let render = match self {
                Self::Render(nodes) => nodes,
                Self::Semantic(_) => &[],
            };
            semantic
                .iter()
                .map(StateTerminalNode::from_semantic)
                .chain(render.iter().map(StateTerminalNode::from_render))
        }
        #[cfg(not(feature = "presentation"))]
        {
            semantic.iter().map(StateTerminalNode::from_semantic)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StateTerminalEdges<'a> {
    Semantic(&'a [StateEdge]),
    #[cfg(feature = "presentation")]
    Render(&'a [StateDiagramRenderEdge]),
}

impl<'a> StateTerminalEdges<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Semantic(edges) => edges.len(),
            #[cfg(feature = "presentation")]
            Self::Render(edges) => edges.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn iter(self) -> impl Iterator<Item = StateTerminalEdge<'a>> + Clone {
        let semantic = match self {
            Self::Semantic(edges) => edges,
            #[cfg(feature = "presentation")]
            Self::Render(_) => &[],
        };
        #[cfg(feature = "presentation")]
        {
            let render = match self {
                Self::Render(edges) => edges,
                Self::Semantic(_) => &[],
            };
            semantic
                .iter()
                .map(StateTerminalEdge::from_semantic)
                .chain(render.iter().map(StateTerminalEdge::from_render))
        }
        #[cfg(not(feature = "presentation"))]
        {
            semantic.iter().map(StateTerminalEdge::from_semantic)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StateTerminalNode<'a> {
    pub id: &'a str,
    pub label: Option<&'a Value>,
    pub description: Option<&'a [String]>,
    pub is_group: bool,
    pub parent_id: Option<&'a str>,
    pub css_compiled_styles: &'a [String],
    pub css_styles: &'a [String],
    pub dir: Option<&'a str>,
    pub explicit_dir: Option<bool>,
    pub shape: &'a str,
    pub position: Option<&'a str>,
    /// Detached render models may carry an additional authored terminal style.
    pub label_style: &'a str,
}

impl<'a> StateTerminalNode<'a> {
    fn from_semantic(node: &'a StateNode) -> Self {
        Self {
            id: &node.id,
            label: node.label.as_ref(),
            description: node.description.as_deref(),
            is_group: node.is_group,
            parent_id: node.parent_id.as_deref(),
            css_compiled_styles: &node.css_compiled_styles,
            css_styles: &node.css_styles,
            dir: node.dir.as_deref(),
            explicit_dir: node.explicit_dir,
            shape: &node.shape,
            position: node.position.as_deref(),
            label_style: "",
        }
    }

    #[cfg(feature = "presentation")]
    fn from_render(node: &'a StateDiagramRenderNode) -> Self {
        Self {
            id: &node.id,
            label: node.label.as_ref(),
            description: node.description.as_deref(),
            is_group: node.is_group,
            parent_id: node.parent_id.as_deref(),
            css_compiled_styles: &node.css_compiled_styles,
            css_styles: &node.css_styles,
            dir: node.dir.as_deref(),
            explicit_dir: node.explicit_dir,
            shape: &node.shape,
            position: node.position.as_deref(),
            label_style: &node.label_style,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StateTerminalEdge<'a> {
    pub start: &'a str,
    pub end: &'a str,
    pub classes: &'a str,
    pub label: &'a str,
    /// Validated for detached render models; canonical edges have no visual arrow selector.
    pub arrow_type_end: &'a str,
}

impl<'a> StateTerminalEdge<'a> {
    fn from_semantic(edge: &'a StateEdge) -> Self {
        Self {
            start: &edge.start,
            end: &edge.end,
            classes: &edge.classes,
            label: &edge.label,
            arrow_type_end: "",
        }
    }

    #[cfg(feature = "presentation")]
    fn from_render(edge: &'a StateDiagramRenderEdge) -> Self {
        Self {
            start: &edge.start,
            end: &edge.end,
            classes: &edge.classes,
            label: &edge.label,
            arrow_type_end: &edge.arrow_type_end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_terminal_view_borrows_nodes_and_edges() {
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync("stateDiagram-v2\nA --> B: ready\n")
            .expect("canonical state source should parse");
        let crate::SemanticDiagramModel::State(model) = &parsed.model else {
            panic!("expected State model");
        };
        let view = StateTerminalView::from_semantic(model);
        assert_eq!(view.nodes().len(), model.nodes.len());
        assert_eq!(view.edges().len(), model.edges.len());
        assert!(!view.nodes().is_empty());
        assert!(!view.edges().is_empty());
        for (view, node) in view.nodes().iter().zip(&model.nodes) {
            assert!(std::ptr::eq(view.id.as_ptr(), node.id.as_ptr()));
            assert!(std::ptr::eq(view.shape.as_ptr(), node.shape.as_ptr()));
            assert!(view.label_style.is_empty());
        }
        for (view, edge) in view.edges().iter().zip(&model.edges) {
            assert!(std::ptr::eq(view.start.as_ptr(), edge.start.as_ptr()));
            assert!(std::ptr::eq(view.label.as_ptr(), edge.label.as_ptr()));
            assert!(view.arrow_type_end.is_empty());
        }
        assert!(view.nodes().get(model.nodes.len()).is_none());
    }

    fn fallback_source() -> &'static str {
        concat!(
            "stateDiagram-v2\ndirection LR\naccTitle: State title\naccDescr: State description\n",
            "state Outer {\nstate \"Human label\" as A\nA --> B: transition\n",
            "note right of A : remember\n}\n",
            "classDef active fill:red,color:blue\nclass A active\nstyle A color:green\n",
            "click A href \"https://example.com\"\n",
        )
    }

    #[test]
    fn canonical_terminal_serialization_preserves_complete_authored_records() {
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(fallback_source())
            .unwrap();
        let crate::SemanticDiagramModel::State(model) = &parsed.model else {
            panic!("expected State model");
        };
        let view = StateTerminalView::from_semantic(model);
        let value = serde_json::to_value(view).unwrap();
        assert_eq!(value["direction"], model.direction);
        assert_eq!(
            value["accTitle"],
            serde_json::to_value(&model.acc_title).unwrap()
        );
        assert_eq!(
            value["accDescr"],
            serde_json::to_value(&model.acc_descr).unwrap()
        );
        assert_eq!(value["nodes"], serde_json::to_value(&model.nodes).unwrap());
        assert_eq!(value["edges"], serde_json::to_value(&model.edges).unwrap());
        assert_eq!(
            value["relations"],
            serde_json::to_value(&model.relations).unwrap()
        );
        assert_eq!(value["links"], serde_json::to_value(&model.links).unwrap());
        assert_eq!(
            value["styleClasses"],
            serde_json::to_value(&model.style_classes).unwrap()
        );
        assert!(!model.links.is_empty());
        assert!(!model.style_classes.is_empty());
        assert_eq!(value["document"]["root"], model.document.root().unwrap().0);
        assert_eq!(
            value["document"]["documents"].as_array().unwrap().len(),
            model.document.len()
        );
        assert!(model.document.len() > 1);
        for (id, record) in &model.states {
            assert_eq!(value["states"][id]["id"], record.id);
            assert_eq!(value["states"][id]["type"], record.state_type);
            assert_eq!(
                value["states"][id]["descriptions"],
                serde_json::to_value(&record.descriptions).unwrap()
            );
            assert_eq!(
                value["states"][id]["classes"],
                serde_json::to_value(&record.classes).unwrap()
            );
            assert_eq!(
                value["states"][id]["styles"],
                serde_json::to_value(&record.styles).unwrap()
            );
            assert_eq!(
                value["states"][id]["textStyles"],
                serde_json::to_value(&record.text_styles).unwrap()
            );
            assert_eq!(
                value["states"][id]["note"],
                serde_json::to_value(&record.note).unwrap()
            );
            assert_eq!(
                value["states"][id]["doc"],
                serde_json::to_value(record.doc.map(|id| id.0)).unwrap()
            );
            assert_eq!(
                value["states"][id]["start"],
                serde_json::to_value(record.start).unwrap()
            );
        }
        let statements = serde_json::to_string(&value["document"]).unwrap();
        for authored in [
            "Human label",
            "transition",
            "remember",
            "classDef",
            "color:green",
            "https://example.com",
        ] {
            assert!(
                statements.contains(authored),
                "missing {authored}: {statements}"
            );
        }
        assert!(value.get("config").is_none());
        assert!(value.get("look").is_none());
        assert_eq!(view.complexity(), model.model_complexity());
        let complexity = view.complexity();
        let json_complexity = ModelComplexity::from_json(&value);
        // Typed complexity excludes fixed schema keys that arbitrary JSON counts as data.
        assert!(complexity.text_bytes <= json_complexity.text_bytes);
        assert_eq!(complexity.nesting_depth, json_complexity.nesting_depth);
        assert!(complexity.items <= json_complexity.items);
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn full_terminal_serialization_keeps_typed_fields_and_flat_documents() {
        let parsed = crate::Engine::new()
            .parse_diagram_for_render_model_sync(fallback_source(), crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::State(model) = parsed.model() else {
            panic!("expected full State model");
        };
        let view = StateTerminalView::from_render(model);
        let value = serde_json::to_value(view).unwrap();
        for (actual, node) in value["nodes"].as_array().unwrap().iter().zip(&model.nodes) {
            let typed = serde_json::to_value(node).unwrap();
            for (field, expected) in typed.as_object().unwrap() {
                assert_eq!(&actual[field], expected, "field: {field}");
            }
            assert_eq!(
                actual.get("explicitDir"),
                node.explicit_dir.map(serde_json::Value::Bool).as_ref()
            );
        }
        assert_eq!(value["edges"], serde_json::to_value(&model.edges).unwrap());
        assert_eq!(
            value["relations"],
            serde_json::to_value(&model.relations).unwrap()
        );
        assert_eq!(value["links"], serde_json::to_value(&model.links).unwrap());
        assert_eq!(
            value["styleClasses"],
            serde_json::to_value(&model.style_classes).unwrap()
        );
        assert_eq!(
            value["document"]["documents"].as_array().unwrap().len(),
            model.document.len()
        );
        assert!(value["states"]["Outer"]["doc"].is_number());
        assert!(
            value["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node.get("domId").is_some())
        );
        assert!(
            value["edges"]
                .as_array()
                .unwrap()
                .iter()
                .any(|edge| edge.get("arrowTypeEnd").is_some())
        );
        let complexity = view.complexity();
        let json_complexity = ModelComplexity::from_json(&value);
        // Typed complexity excludes fixed schema keys that arbitrary JSON counts as data.
        assert!(complexity.text_bytes <= json_complexity.text_bytes);
        assert_eq!(complexity.nesting_depth, json_complexity.nesting_depth);
        assert!(complexity.items <= json_complexity.items);

        let mut detached = StateDiagramRenderModel {
            nodes: vec![model.nodes[0].clone()],
            ..StateDiagramRenderModel::default()
        };
        detached.nodes[0].explicit_dir = Some(true);
        let value = serde_json::to_value(StateTerminalView::from_render(&detached)).unwrap();
        assert_eq!(value["nodes"][0]["explicitDir"], true);
        assert!(
            value["document"]["documents"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn terminal_complexity_keeps_the_authored_document_nesting_bound() {
        const DEPTH: usize = 40;
        let mut source = String::from("stateDiagram-v2\n");
        for level in 0..DEPTH {
            source.push_str(&format!("state S{level} {{\n"));
        }
        source.push_str("Leaf\n");
        source.push_str(&"}\n".repeat(DEPTH));
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(&source)
            .unwrap();
        let crate::SemanticDiagramModel::State(model) = &parsed.model else {
            panic!("expected State model");
        };
        let view = StateTerminalView::from_semantic(model);
        assert!(view.complexity().nesting_depth >= DEPTH);
        assert_eq!(view.complexity(), model.model_complexity());
    }
}
