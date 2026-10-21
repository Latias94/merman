use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Authored Class diagram meaning, before DOM ids and namespace render configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassDiagramModel {
    #[serde(rename = "type")]
    pub diagram_type: String,
    pub direction: String,
    #[serde(rename = "accTitle")]
    #[serde(default)]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    #[serde(default)]
    pub acc_descr: Option<String>,
    pub classes: IndexMap<String, ClassNode>,
    #[serde(default)]
    pub relations: Vec<ClassRelation>,
    #[serde(default)]
    pub notes: Vec<ClassNote>,
    #[serde(default)]
    pub interfaces: Vec<ClassInterface>,
    #[serde(default)]
    pub namespaces: IndexMap<String, Namespace>,
    /// Parser-owned provenance for qualified relation endpoints synthesized as facade classes.
    ///
    /// Typed serde preserves this provenance so a model can round-trip without changing relation
    /// routing. The Mermaid-compatible JSON projection removes it explicitly because it is
    /// renderer metadata rather than an authored Mermaid field.
    #[serde(rename = "namespaceFacadeAliases", default)]
    pub namespace_facade_aliases: BTreeMap<String, String>,
    #[serde(rename = "styleClasses")]
    #[serde(default)]
    pub style_classes: IndexMap<String, StyleClass>,
    pub constants: ClassConstants,
    #[serde(rename = "stylePrecedenceFacts", default)]
    pub style_precedence_facts: ClassStylePrecedenceFacts,
}

impl ClassDiagramModel {
    /// Borrows the canonical graph with its requested namespace and routing topology.
    ///
    /// The caller supplies the already-admitted configuration and operation control. Authored
    /// classes, relations, notes, interfaces, and style witnesses remain in this model.
    pub fn topology(
        &self,
        config: &crate::MermaidConfig,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<ClassTopologyView<'_>> {
        Ok(ClassTopologyView {
            model: self,
            topology: Cow::Owned(ClassTopology::new_controlled(self, config, control)?),
        })
    }

    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

/// Requested namespace and routing projection of a canonical Class diagram.
///
/// Namespace ids historically named `namespace_dom_ids` are also the stable route identities
/// used by terminal output. Class DOM ids are owned separately by `ClassPresentation`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassTopology {
    pub namespace_dom_ids: IndexMap<String, String>,
    pub namespaces: IndexMap<String, Namespace>,
    pub class_parents: IndexMap<String, Option<String>>,
    pub note_ids: Vec<String>,
    pub note_parents: Vec<Option<String>>,
    pub interface_ids: Vec<String>,
    interface_id_remaps: BTreeMap<String, String>,
}

impl ClassTopology {
    fn new_controlled(
        model: &ClassDiagramModel,
        config: &crate::MermaidConfig,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Self> {
        use std::collections::BTreeSet;

        control.checkpoint()?;
        let prefix = crate::diagrams::class::MERMAID_DOM_ID_PREFIX;
        let mut namespace_dom_ids = IndexMap::with_capacity(model.namespaces.len());
        for (ordinal, id) in model.namespaces.keys().enumerate() {
            control.checkpoint()?;
            namespace_dom_ids.insert(id.clone(), format!("{prefix}{id}-{ordinal}"));
        }
        let hierarchical = config
            .get_bool("class.hierarchicalNamespaces")
            .unwrap_or(true);
        let parent = |id: Option<&str>| -> crate::OperationControlResult<Option<String>> {
            if hierarchical {
                return Ok(id.map(str::to_string));
            }
            Self::explicit_ancestor(&model.namespaces, id, control)
        };
        let mut class_parents = IndexMap::with_capacity(model.classes.len());
        for (id, class) in &model.classes {
            control.checkpoint()?;
            class_parents.insert(id.clone(), parent(class.parent.as_deref())?);
        }
        let mut note_parents = Vec::with_capacity(model.notes.len());
        for note in &model.notes {
            control.checkpoint()?;
            note_parents.push(parent(note.parent.as_deref())?);
        }
        let mut namespaces = if hierarchical {
            IndexMap::with_capacity(model.namespaces.len())
        } else {
            IndexMap::new()
        };
        for (key, namespace) in &model.namespaces {
            control.checkpoint()?;
            if hierarchical || namespace.explicit {
                namespaces.insert(
                    key.clone(),
                    Namespace {
                        id: namespace.id.clone(),
                        label: if hierarchical {
                            namespace.label.clone()
                        } else {
                            namespace.id.clone()
                        },
                        class_ids: Vec::new(),
                        note_ids: Vec::new(),
                        parent: if hierarchical {
                            namespace.parent.clone()
                        } else {
                            None
                        },
                        explicit: namespace.explicit,
                    },
                );
            }
        }

        let mut occupied: BTreeSet<String> = BTreeSet::new();
        if !model.notes.is_empty() || !model.interfaces.is_empty() {
            for id in model
                .classes
                .keys()
                .chain(namespaces.keys().filter_map(|id| namespace_dom_ids.get(id)))
            {
                control.checkpoint()?;
                occupied.insert(id.clone());
            }
        }
        let mut note_ids = Vec::with_capacity(model.notes.len());
        for ordinal in 0..model.notes.len() {
            control.checkpoint()?;
            note_ids.push(Self::unique_id("note", ordinal, &mut occupied, control)?);
        }
        let mut interface_id_remaps = BTreeMap::new();
        let mut interface_ids = Vec::with_capacity(model.interfaces.len());
        for (ordinal, interface) in model.interfaces.iter().enumerate() {
            control.checkpoint()?;
            let id = Self::unique_id("interface", ordinal, &mut occupied, control)?;
            if id != interface.id {
                interface_id_remaps.insert(interface.id.clone(), id.clone());
            }
            interface_ids.push(id);
        }

        let mut projection = Self {
            namespace_dom_ids,
            namespaces,
            class_parents,
            note_ids,
            note_parents,
            interface_ids,
            interface_id_remaps,
        };
        for (id, parent) in &projection.class_parents {
            control.checkpoint()?;
            if let Some(namespace) = parent
                .as_deref()
                .and_then(|id| projection.namespaces.get_mut(id))
            {
                namespace.class_ids.push(id.clone());
            }
        }
        for (id, parent) in projection.note_ids.iter().zip(&projection.note_parents) {
            control.checkpoint()?;
            if let Some(namespace) = parent
                .as_deref()
                .and_then(|id| projection.namespaces.get_mut(id))
            {
                namespace.note_ids.push(id.clone());
            }
        }
        control.checkpoint()?;
        Ok(projection)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        let mut bytes = 0usize;
        for (id, dom_id) in &self.namespace_dom_ids {
            bytes = bytes.saturating_add(id.len()).saturating_add(dom_id.len());
        }
        for (id, namespace) in &self.namespaces {
            bytes = bytes
                .saturating_add(id.len())
                .saturating_add(namespace.id.len())
                .saturating_add(namespace.label.len())
                .saturating_add(namespace.parent.as_deref().map_or(0, str::len));
            for member in namespace.class_ids.iter().chain(&namespace.note_ids) {
                bytes = bytes.saturating_add(member.len());
            }
        }
        for (id, parent) in &self.class_parents {
            bytes = bytes
                .saturating_add(id.len())
                .saturating_add(parent.as_deref().map_or(0, str::len));
        }
        for id in self.note_ids.iter().chain(&self.interface_ids) {
            bytes = bytes.saturating_add(id.len());
        }
        for parent in self.note_parents.iter().flatten() {
            bytes = bytes.saturating_add(parent.len());
        }
        for (source, rendered) in &self.interface_id_remaps {
            bytes = bytes
                .saturating_add(source.len())
                .saturating_add(rendered.len());
        }
        bytes
    }

    pub fn namespace_dom_id(&self, id: &str) -> Option<&str> {
        self.namespace_dom_ids.get(id).map(String::as_str)
    }

    pub fn class_parent(&self, id: &str) -> Option<&str> {
        self.class_parents.get(id).and_then(Option::as_deref)
    }

    pub fn relation_endpoints<'a>(&'a self, relation: &'a ClassRelation) -> (&'a str, &'a str) {
        let endpoint = |id: &'a str, relation_type| {
            if relation_type == crate::diagrams::class::REL_LOLLIPOP {
                self.interface_id_remaps
                    .get(id)
                    .map(String::as_str)
                    .unwrap_or(id)
            } else {
                id
            }
        };
        (
            endpoint(&relation.id1, relation.relation.type1),
            endpoint(&relation.id2, relation.relation.type2),
        )
    }

    fn explicit_ancestor(
        namespaces: &IndexMap<String, Namespace>,
        id: Option<&str>,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Option<String>> {
        let mut current = id;
        for _ in 0..namespaces.len() {
            control.checkpoint()?;
            let Some(id) = current else { return Ok(None) };
            let Some(namespace) = namespaces.get(id) else {
                return Ok(None);
            };
            if namespace.explicit {
                return Ok(Some(id.to_string()));
            }
            current = namespace.parent.as_deref();
        }
        Ok(None)
    }

    fn unique_id(
        prefix: &str,
        ordinal: usize,
        occupied: &mut std::collections::BTreeSet<String>,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<String> {
        let base = format!("{prefix}{ordinal}");
        if occupied.insert(base.clone()) {
            return Ok(base);
        }
        let mut disambiguator = 1usize;
        loop {
            control.checkpoint()?;
            let candidate = format!("{base}${disambiguator}");
            if occupied.insert(candidate.clone()) {
                return Ok(candidate);
            }
            disambiguator = disambiguator.saturating_add(1);
        }
    }
}

#[cfg(test)]
#[test]
fn flat_class_topology_does_not_reserve_slots_for_implicit_namespaces() {
    let namespace_id = (0..128)
        .map(|index| format!("N{index}"))
        .collect::<Vec<_>>()
        .join(".");
    let parsed = crate::SemanticParser::new()
        .parse_diagram_sync(&format!(
            "classDiagram\nnamespace {namespace_id} {{\nclass Service\n}}\n"
        ))
        .unwrap();
    let crate::SemanticDiagramModel::Class(mut model) = parsed.model else {
        panic!("expected a canonical Class model");
    };
    assert_eq!(model.namespaces.len(), 128);
    let mut config = crate::MermaidConfig::empty_object();
    config.set_value("class.hierarchicalNamespaces", false.into());
    {
        let topology = model
            .topology(&config, &crate::OperationControl::new())
            .unwrap();
        assert_eq!(topology.namespaces().len(), 1);
        assert!(topology.topology.namespaces.capacity() < model.namespaces.len());
        assert_eq!(
            topology.namespace(&namespace_id).unwrap().route_id,
            format!("classId-{namespace_id}-127")
        );
    }
    for namespace in model.namespaces.values_mut() {
        namespace.explicit = false;
    }
    let topology = model
        .topology(&config, &crate::OperationControl::new())
        .unwrap();
    assert_eq!(topology.namespaces().len(), 0);
    assert_eq!(topology.topology.namespaces.capacity(), 0);
}

/// A canonical Class graph borrowed together with its privately paired topology.
#[derive(Debug)]
pub struct ClassTopologyView<'a> {
    model: &'a ClassDiagramModel,
    topology: Cow<'a, ClassTopology>,
}

impl ClassTopologyView<'_> {
    pub fn model(&self) -> &ClassDiagramModel {
        self.model
    }

    /// Bytes retained by the requested topology, excluding the borrowed canonical graph.
    pub fn retained_bytes(&self) -> usize {
        self.topology.retained_bytes()
    }

    pub fn classes(&self) -> impl ExactSizeIterator<Item = (&str, ClassNodeTopologyView<'_>)> {
        self.model.classes.iter().map(|(id, node)| {
            (
                id.as_str(),
                ClassNodeTopologyView {
                    node,
                    parent: self.topology.class_parent(id),
                },
            )
        })
    }

    pub fn class(&self, id: &str) -> Option<ClassNodeTopologyView<'_>> {
        Some(ClassNodeTopologyView {
            node: self.model.classes.get(id)?,
            parent: self.topology.class_parent(id),
        })
    }

    pub fn namespaces(&self) -> impl ExactSizeIterator<Item = (&str, NamespaceTopologyView<'_>)> {
        self.topology.namespaces.iter().map(|(id, namespace)| {
            (
                id.as_str(),
                NamespaceTopologyView {
                    namespace,
                    route_id: &self.topology.namespace_dom_ids[id],
                },
            )
        })
    }

    pub fn namespace(&self, id: &str) -> Option<NamespaceTopologyView<'_>> {
        Some(NamespaceTopologyView {
            namespace: self.topology.namespaces.get(id)?,
            route_id: self.topology.namespace_dom_id(id)?,
        })
    }

    pub fn relations(&self) -> impl ExactSizeIterator<Item = ClassRelationRenderView<'_>> {
        self.model.relations.iter().map(|source| {
            let (id1, id2) = self.topology.relation_endpoints(source);
            ClassRelationRenderView { source, id1, id2 }
        })
    }

    pub fn notes(&self) -> impl ExactSizeIterator<Item = ClassNoteRenderView<'_>> {
        self.model
            .notes
            .iter()
            .enumerate()
            .map(|(index, note)| ClassNoteRenderView {
                note,
                id: &self.topology.note_ids[index],
                parent: self.topology.note_parents[index].as_deref(),
            })
    }

    pub fn interfaces(&self) -> impl ExactSizeIterator<Item = ClassInterfaceRenderView<'_>> {
        self.model
            .interfaces
            .iter()
            .enumerate()
            .map(|(index, interface)| ClassInterfaceRenderView {
                interface,
                id: &self.topology.interface_ids[index],
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClassNodeTopologyView<'a> {
    pub node: &'a ClassNode,
    pub parent: Option<&'a str>,
}

impl std::ops::Deref for ClassNodeTopologyView<'_> {
    type Target = ClassNode;
    fn deref(&self) -> &Self::Target {
        self.node
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NamespaceTopologyView<'a> {
    pub namespace: &'a Namespace,
    pub route_id: &'a str,
}

impl std::ops::Deref for NamespaceTopologyView<'_> {
    type Target = Namespace;
    fn deref(&self) -> &Self::Target {
        self.namespace
    }
}

/// Full Class presentation with one shared topology and a requested class DOM map.
#[cfg(feature = "presentation")]
#[derive(Debug, Clone, PartialEq)]
pub struct ClassPresentation {
    pub class_dom_ids: IndexMap<String, String>,
    topology: ClassTopology,
}

#[cfg(feature = "presentation")]
impl std::ops::Deref for ClassPresentation {
    type Target = ClassTopology;
    fn deref(&self) -> &Self::Target {
        &self.topology
    }
}

#[cfg(feature = "presentation")]
impl std::ops::DerefMut for ClassPresentation {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.topology
    }
}

#[cfg(feature = "presentation")]
impl ClassPresentation {
    pub fn new(model: &ClassDiagramModel, config: &crate::MermaidConfig) -> Self {
        Self::new_controlled(model, config, &crate::OperationControl::new())
            .expect("a private operation control cannot be cancelled")
    }

    fn new_controlled(
        model: &ClassDiagramModel,
        config: &crate::MermaidConfig,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Self> {
        let topology = ClassTopology::new_controlled(model, config, control)?;
        let prefix = crate::diagrams::class::MERMAID_DOM_ID_PREFIX;
        let mut class_dom_ids = IndexMap::with_capacity(model.classes.len());
        for (ordinal, id) in model.classes.keys().enumerate() {
            control.checkpoint()?;
            class_dom_ids.insert(id.clone(), format!("{prefix}{id}-{ordinal}"));
        }
        control.checkpoint()?;
        Ok(Self {
            class_dom_ids,
            topology,
        })
    }

    pub fn class_dom_id(&self, id: &str) -> Option<&str> {
        self.class_dom_ids.get(id).map(String::as_str)
    }

    pub fn namespace_dom_id(&self, id: &str) -> Option<&str> {
        self.topology.namespace_dom_id(id)
    }

    pub fn class_parent(&self, id: &str) -> Option<&str> {
        self.topology.class_parent(id)
    }

    pub fn relation_endpoints<'a>(&'a self, relation: &'a ClassRelation) -> (&'a str, &'a str) {
        self.topology.relation_endpoints(relation)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.class_dom_ids
            .iter()
            .fold(self.topology.retained_bytes(), |bytes, (id, dom_id)| {
                bytes.saturating_add(id.len()).saturating_add(dom_id.len())
            })
    }
}

/// Full typed Class result with one canonical owner and a requested visual sidecar.
#[cfg(feature = "presentation")]
#[derive(Debug, Clone)]
pub struct ClassDiagram {
    // Keep full parse carriers small on caller-owned stacks.
    inner: Box<ClassDiagramInner>,
}

#[cfg(feature = "presentation")]
#[derive(Debug, Clone)]
struct ClassDiagramInner {
    model: ClassDiagramModel,
    presentation: ClassPresentation,
}

#[cfg(feature = "presentation")]
impl ClassDiagram {
    pub fn model(&self) -> &ClassDiagramModel {
        &self.inner.model
    }

    pub fn presentation(&self) -> &ClassPresentation {
        &self.inner.presentation
    }

    /// Borrows the existing full topology, including detached wire identities, without rebuilding.
    pub fn topology_view(&self) -> ClassTopologyView<'_> {
        ClassTopologyView {
            model: &self.inner.model,
            topology: Cow::Borrowed(&self.inner.presentation.topology),
        }
    }

    /// Returns the canonical graph for editing before rebuilding its presentation.
    pub fn into_model(self) -> ClassDiagramModel {
        self.inner.model
    }

    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        self.inner.model.sanitize_common_db_fields(config);
    }

    pub fn new(model: ClassDiagramModel, config: &crate::MermaidConfig) -> Self {
        let presentation = ClassPresentation::new(&model, config);
        Self {
            inner: Box::new(ClassDiagramInner {
                model,
                presentation,
            }),
        }
    }

    pub(crate) fn new_controlled(
        model: ClassDiagramModel,
        config: &crate::MermaidConfig,
        control: &crate::OperationControl,
    ) -> crate::OperationControlResult<Self> {
        let presentation = ClassPresentation::new_controlled(&model, config, control)?;
        Ok(Self {
            inner: Box::new(ClassDiagramInner {
                model,
                presentation,
            }),
        })
    }

    pub fn classes(&self) -> impl ExactSizeIterator<Item = (&str, ClassNodeRenderView<'_>)> {
        self.inner.model.classes.iter().map(|(id, node)| {
            (
                id.as_str(),
                ClassNodeRenderView {
                    node,
                    dom_id: &self.inner.presentation.class_dom_ids[id],
                    parent: self.inner.presentation.class_parent(id),
                },
            )
        })
    }

    pub fn class(&self, id: &str) -> Option<ClassNodeRenderView<'_>> {
        let node = self.inner.model.classes.get(id)?;
        Some(ClassNodeRenderView {
            node,
            dom_id: self.inner.presentation.class_dom_id(id)?,
            parent: self.inner.presentation.class_parent(id),
        })
    }

    pub fn namespaces(&self) -> impl ExactSizeIterator<Item = (&str, NamespaceRenderView<'_>)> {
        self.inner
            .presentation
            .namespaces
            .iter()
            .map(|(id, namespace)| {
                (
                    id.as_str(),
                    NamespaceRenderView {
                        namespace,
                        dom_id: &self.inner.presentation.namespace_dom_ids[id],
                    },
                )
            })
    }

    pub fn namespace(&self, id: &str) -> Option<NamespaceRenderView<'_>> {
        Some(NamespaceRenderView {
            namespace: self.inner.presentation.namespaces.get(id)?,
            dom_id: self.inner.presentation.namespace_dom_id(id)?,
        })
    }

    pub fn relations(&self) -> impl ExactSizeIterator<Item = ClassRelationRenderView<'_>> {
        self.inner.model.relations.iter().map(|relation| {
            let (id1, id2) = self.inner.presentation.relation_endpoints(relation);
            ClassRelationRenderView {
                source: relation,
                id1,
                id2,
            }
        })
    }

    pub fn notes(&self) -> impl ExactSizeIterator<Item = ClassNoteRenderView<'_>> {
        self.inner
            .model
            .notes
            .iter()
            .enumerate()
            .map(|(index, note)| ClassNoteRenderView {
                note,
                id: &self.inner.presentation.note_ids[index],
                parent: self.inner.presentation.note_parents[index].as_deref(),
            })
    }

    pub fn interfaces(&self) -> impl ExactSizeIterator<Item = ClassInterfaceRenderView<'_>> {
        self.inner
            .model
            .interfaces
            .iter()
            .enumerate()
            .map(|(index, interface)| ClassInterfaceRenderView {
                interface,
                id: &self.inner.presentation.interface_ids[index],
            })
    }
}

/// Borrowed full node fields, including visual parent and DOM id.
#[cfg(feature = "presentation")]
#[derive(Debug, Clone, Copy)]
pub struct ClassNodeRenderView<'a> {
    pub node: &'a ClassNode,
    pub dom_id: &'a str,
    pub parent: Option<&'a str>,
}

#[cfg(feature = "presentation")]
impl std::ops::Deref for ClassNodeRenderView<'_> {
    type Target = ClassNode;
    fn deref(&self) -> &Self::Target {
        self.node
    }
}

#[cfg(feature = "presentation")]
impl PartialEq for ClassNodeRenderView<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.dom_id == other.dom_id
            && self.parent == other.parent
            && self.node.id == other.node.id
            && self.node.type_param == other.node.type_param
            && self.node.label == other.node.label
            && self.node.text == other.node.text
            && self.node.css_classes == other.node.css_classes
            && self.node.methods == other.node.methods
            && self.node.members == other.node.members
            && self.node.annotations == other.node.annotations
            && self.node.styles == other.node.styles
            && self.node.link == other.node.link
            && self.node.link_target == other.node.link_target
            && self.node.tooltip == other.node.tooltip
            && self.node.have_callback == other.node.have_callback
            && self.node.callback == other.node.callback
            && self.node.callback_effective == other.node.callback_effective
    }
}

#[cfg(feature = "presentation")]
impl Serialize for ClassNodeRenderView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut node = serializer.serialize_struct("ClassNode", 17)?;
        node.serialize_field("id", &self.node.id)?;
        node.serialize_field("type", &self.node.type_param)?;
        node.serialize_field("label", &self.node.label)?;
        node.serialize_field("text", &self.node.text)?;
        node.serialize_field("cssClasses", &self.node.css_classes)?;
        node.serialize_field("methods", &self.node.methods)?;
        node.serialize_field("members", &self.node.members)?;
        node.serialize_field("annotations", &self.node.annotations)?;
        node.serialize_field("styles", &self.node.styles)?;
        node.serialize_field("domId", &self.dom_id)?;
        node.serialize_field("parent", &self.parent)?;
        node.serialize_field("link", &self.node.link)?;
        node.serialize_field("linkTarget", &self.node.link_target)?;
        node.serialize_field("tooltip", &self.node.tooltip)?;
        node.serialize_field("haveCallback", &self.node.have_callback)?;
        node.serialize_field("callback", &self.node.callback)?;
        node.serialize_field("callbackEffective", &self.node.callback_effective)?;
        node.end()
    }
}

#[cfg(feature = "presentation")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NamespaceRenderView<'a> {
    pub namespace: &'a Namespace,
    pub dom_id: &'a str,
}

#[cfg(feature = "presentation")]
impl std::ops::Deref for NamespaceRenderView<'_> {
    type Target = Namespace;
    fn deref(&self) -> &Self::Target {
        self.namespace
    }
}

#[cfg(feature = "presentation")]
impl Serialize for NamespaceRenderView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut namespace = serializer.serialize_struct("Namespace", 7)?;
        namespace.serialize_field("id", &self.namespace.id)?;
        namespace.serialize_field("label", &self.namespace.label)?;
        namespace.serialize_field("domId", &self.dom_id)?;
        namespace.serialize_field("classIds", &self.namespace.class_ids)?;
        namespace.serialize_field("noteIds", &self.namespace.note_ids)?;
        namespace.serialize_field("parent", &self.namespace.parent)?;
        namespace.serialize_field("explicit", &self.namespace.explicit)?;
        namespace.end()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClassRelationRenderView<'a> {
    pub source: &'a ClassRelation,
    pub id1: &'a str,
    pub id2: &'a str,
}

impl std::ops::Deref for ClassRelationRenderView<'_> {
    type Target = ClassRelation;
    fn deref(&self) -> &Self::Target {
        self.source
    }
}

impl PartialEq for ClassRelationRenderView<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id1 == other.id1
            && self.id2 == other.id2
            && self.source.id == other.source.id
            && self.source.relation_title_1 == other.source.relation_title_1
            && self.source.relation_title_2 == other.source.relation_title_2
            && self.source.title == other.source.title
            && self.source.relation == other.source.relation
    }
}

#[cfg(feature = "presentation")]
impl Serialize for ClassRelationRenderView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut relation = serializer.serialize_struct("ClassRelation", 7)?;
        relation.serialize_field("id", &self.source.id)?;
        relation.serialize_field("id1", &self.id1)?;
        relation.serialize_field("id2", &self.id2)?;
        relation.serialize_field("relationTitle1", &self.source.relation_title_1)?;
        relation.serialize_field("relationTitle2", &self.source.relation_title_2)?;
        relation.serialize_field("title", &self.source.title)?;
        relation.serialize_field("relation", &self.source.relation)?;
        relation.end()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClassNoteRenderView<'a> {
    pub note: &'a ClassNote,
    pub id: &'a str,
    pub parent: Option<&'a str>,
}

impl std::ops::Deref for ClassNoteRenderView<'_> {
    type Target = ClassNote;
    fn deref(&self) -> &Self::Target {
        self.note
    }
}

impl PartialEq for ClassNoteRenderView<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.parent == other.parent
            && self.note.class_id == other.note.class_id
            && self.note.text == other.note.text
    }
}

#[cfg(feature = "presentation")]
impl Serialize for ClassNoteRenderView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut note =
            serializer.serialize_struct("ClassNote", 3 + usize::from(self.parent.is_some()))?;
        note.serialize_field("id", &self.id)?;
        note.serialize_field("class", &self.note.class_id)?;
        note.serialize_field("text", &self.note.text)?;
        if let Some(parent) = self.parent {
            note.serialize_field("parent", parent)?;
        }
        note.end()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClassInterfaceRenderView<'a> {
    pub interface: &'a ClassInterface,
    pub id: &'a str,
}

impl std::ops::Deref for ClassInterfaceRenderView<'_> {
    type Target = ClassInterface;
    fn deref(&self) -> &Self::Target {
        self.interface
    }
}

impl PartialEq for ClassInterfaceRenderView<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.interface.label == other.interface.label
            && self.interface.class_id == other.interface.class_id
    }
}

#[cfg(feature = "presentation")]
impl Serialize for ClassInterfaceRenderView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut interface = serializer.serialize_struct("ClassInterface", 3)?;
        interface.serialize_field("id", &self.id)?;
        interface.serialize_field("label", &self.interface.label)?;
        interface.serialize_field("classId", &self.interface.class_id)?;
        interface.end()
    }
}

// Full equality follows the published visual fields, as it did before canonical extraction.
// A legacy full wire document cannot preserve parser-only authored structure or style witnesses.
#[cfg(feature = "presentation")]
impl PartialEq for ClassDiagram {
    fn eq(&self, other: &Self) -> bool {
        self.inner.model.diagram_type == other.inner.model.diagram_type
            && self.inner.model.direction == other.inner.model.direction
            && self.inner.model.acc_title == other.inner.model.acc_title
            && self.inner.model.acc_descr == other.inner.model.acc_descr
            && self.inner.model.namespace_facade_aliases
                == other.inner.model.namespace_facade_aliases
            && self.inner.model.style_classes == other.inner.model.style_classes
            && self.inner.model.constants == other.inner.model.constants
            && self.inner.model.classes.len() == other.inner.model.classes.len()
            && self
                .classes()
                .all(|(id, node)| other.class(id).is_some_and(|other| node == other))
            && self.inner.presentation.namespaces.len() == other.inner.presentation.namespaces.len()
            && self
                .namespaces()
                .all(|(id, namespace)| other.namespace(id).is_some_and(|other| namespace == other))
            && self.relations().eq(other.relations())
            && self.notes().eq(other.notes())
            && self.interfaces().eq(other.interfaces())
    }
}

#[cfg(feature = "presentation")]
struct RenderClasses<'a>(&'a ClassDiagram);
#[cfg(feature = "presentation")]
impl Serialize for RenderClasses<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.classes())
    }
}

#[cfg(feature = "presentation")]
struct RenderNamespaces<'a>(&'a ClassDiagram);
#[cfg(feature = "presentation")]
impl Serialize for RenderNamespaces<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.namespaces())
    }
}

#[cfg(feature = "presentation")]
struct RenderRelations<'a>(&'a ClassDiagram);
#[cfg(feature = "presentation")]
impl Serialize for RenderRelations<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.relations())
    }
}

#[cfg(feature = "presentation")]
struct RenderNotes<'a>(&'a ClassDiagram);
#[cfg(feature = "presentation")]
impl Serialize for RenderNotes<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.notes())
    }
}

#[cfg(feature = "presentation")]
struct RenderInterfaces<'a>(&'a ClassDiagram);
#[cfg(feature = "presentation")]
impl Serialize for RenderInterfaces<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.interfaces())
    }
}

#[cfg(feature = "presentation")]
impl Serialize for ClassDiagram {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut diagram = serializer.serialize_struct("ClassDiagram", 12)?;
        diagram.serialize_field("type", &self.inner.model.diagram_type)?;
        diagram.serialize_field("direction", &self.inner.model.direction)?;
        diagram.serialize_field("accTitle", &self.inner.model.acc_title)?;
        diagram.serialize_field("accDescr", &self.inner.model.acc_descr)?;
        diagram.serialize_field("classes", &RenderClasses(self))?;
        diagram.serialize_field("relations", &RenderRelations(self))?;
        diagram.serialize_field("notes", &RenderNotes(self))?;
        diagram.serialize_field("interfaces", &RenderInterfaces(self))?;
        diagram.serialize_field("namespaces", &RenderNamespaces(self))?;
        diagram.serialize_field(
            "namespaceFacadeAliases",
            &self.inner.model.namespace_facade_aliases,
        )?;
        diagram.serialize_field("styleClasses", &self.inner.model.style_classes)?;
        diagram.serialize_field("constants", &self.inner.model.constants)?;
        diagram.end()
    }
}

#[cfg(feature = "presentation")]
#[derive(Deserialize)]
struct FullClassNodeWire {
    #[serde(flatten)]
    node: ClassNode,
    #[serde(rename = "domId")]
    dom_id: String,
}

#[cfg(feature = "presentation")]
#[derive(Deserialize)]
struct FullNamespaceWire {
    #[serde(flatten)]
    namespace: Namespace,
    #[serde(rename = "domId")]
    dom_id: String,
}

#[cfg(feature = "presentation")]
impl<'de> Deserialize<'de> for ClassDiagram {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FullClassVisitor;
        impl<'de> serde::de::Visitor<'de> for FullClassVisitor {
            type Value = ClassDiagram;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a full Class diagram")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut diagram_type = None;
                let mut direction = None;
                let mut acc_title = None;
                let mut acc_descr = None;
                let mut classes = None;
                let mut class_dom_ids = IndexMap::new();
                let mut relations = Vec::new();
                let mut notes = Vec::new();
                let mut interfaces = Vec::new();
                let mut namespaces = IndexMap::new();
                let mut namespace_dom_ids = IndexMap::new();
                let mut namespace_facade_aliases = BTreeMap::new();
                let mut style_classes = IndexMap::new();
                let mut constants = None;
                let mut seen = std::collections::BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    let known = match key.as_str() {
                        "type" => Some("type"),
                        "direction" => Some("direction"),
                        "accTitle" => Some("accTitle"),
                        "accDescr" => Some("accDescr"),
                        "classes" => Some("classes"),
                        "relations" => Some("relations"),
                        "notes" => Some("notes"),
                        "interfaces" => Some("interfaces"),
                        "namespaces" => Some("namespaces"),
                        "namespaceFacadeAliases" => Some("namespaceFacadeAliases"),
                        "styleClasses" => Some("styleClasses"),
                        "constants" => Some("constants"),
                        _ => None,
                    };
                    if let Some(field) = known
                        && !seen.insert(field)
                    {
                        return Err(serde::de::Error::duplicate_field(field));
                    }
                    match key.as_str() {
                        "type" => diagram_type = Some(map.next_value()?),
                        "direction" => direction = Some(map.next_value()?),
                        "accTitle" => acc_title = map.next_value()?,
                        "accDescr" => acc_descr = map.next_value()?,
                        "classes" => {
                            let wire: IndexMap<String, FullClassNodeWire> = map.next_value()?;
                            let mut canonical = IndexMap::with_capacity(wire.len());
                            for (id, wire) in wire {
                                class_dom_ids.insert(id.clone(), wire.dom_id);
                                canonical.insert(id, wire.node);
                            }
                            classes = Some(canonical);
                        }
                        "relations" => relations = map.next_value()?,
                        "notes" => notes = map.next_value()?,
                        "interfaces" => interfaces = map.next_value()?,
                        "namespaces" => {
                            let wire: IndexMap<String, FullNamespaceWire> = map.next_value()?;
                            for (id, wire) in wire {
                                namespace_dom_ids.insert(id.clone(), wire.dom_id);
                                namespaces.insert(id, wire.namespace);
                            }
                        }
                        "namespaceFacadeAliases" => namespace_facade_aliases = map.next_value()?,
                        "styleClasses" => style_classes = map.next_value()?,
                        "constants" => constants = Some(map.next_value()?),
                        _ => {
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                let model = ClassDiagramModel {
                    diagram_type: diagram_type
                        .ok_or_else(|| serde::de::Error::missing_field("type"))?,
                    direction: direction
                        .ok_or_else(|| serde::de::Error::missing_field("direction"))?,
                    acc_title,
                    acc_descr,
                    classes: classes.ok_or_else(|| serde::de::Error::missing_field("classes"))?,
                    relations,
                    notes,
                    interfaces,
                    namespaces,
                    namespace_facade_aliases,
                    style_classes,
                    constants: constants
                        .ok_or_else(|| serde::de::Error::missing_field("constants"))?,
                    style_precedence_facts: ClassStylePrecedenceFacts::default(),
                };
                // An old full wire document already contains a visual projection. Retain that
                // projection verbatim; it cannot recover authored namespaces or original ids.
                let presentation = ClassPresentation {
                    class_dom_ids,
                    topology: ClassTopology {
                        namespace_dom_ids,
                        namespaces: model.namespaces.clone(),
                        class_parents: model
                            .classes
                            .iter()
                            .map(|(id, node)| (id.clone(), node.parent.clone()))
                            .collect(),
                        note_ids: model.notes.iter().map(|note| note.id.clone()).collect(),
                        note_parents: model.notes.iter().map(|note| note.parent.clone()).collect(),
                        interface_ids: model
                            .interfaces
                            .iter()
                            .map(|interface| interface.id.clone())
                            .collect(),
                        interface_id_remaps: BTreeMap::new(),
                    },
                };
                Ok(ClassDiagram {
                    inner: Box::new(ClassDiagramInner {
                        model,
                        presentation,
                    }),
                })
            }
        }
        deserializer.deserialize_map(FullClassVisitor)
    }
}

/// Parser-owned witnesses for ClassDiagram encounter-order styling behavior.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassStylePrecedenceFacts {
    pub(crate) assignment_before_definition_copy: Option<ClassStylePrecedenceWitness>,
    pub(crate) definition_before_assignment_no_backfill: Option<ClassStylePrecedenceWitness>,
    pub(crate) inline_paint: Option<ClassStyleDeclarationWitness>,
    pub(crate) classdef_typography: Option<ClassStyleDeclarationWitness>,
}

impl ClassStylePrecedenceFacts {
    /// Whether a non-empty class definition copied paint into an already assigned class.
    pub const fn assignment_before_definition_copy(&self) -> bool {
        self.assignment_before_definition_copy.is_some()
    }

    /// Whether a later assignment retained the class name without replaying prior paint.
    pub const fn definition_before_assignment_no_backfill(&self) -> bool {
        self.definition_before_assignment_no_backfill.is_some()
    }

    /// Returns the concrete target and declaration witness for copy semantics.
    pub fn assignment_before_definition_copy_witness(
        &self,
    ) -> Option<&ClassStylePrecedenceWitness> {
        self.assignment_before_definition_copy.as_ref()
    }

    /// Returns the concrete target and declaration witness for no-backfill semantics.
    pub fn definition_before_assignment_no_backfill_witness(
        &self,
    ) -> Option<&ClassStylePrecedenceWitness> {
        self.definition_before_assignment_no_backfill.as_ref()
    }

    /// Returns one explicit `style` paint declaration observed by the parser.
    pub fn inline_paint_witness(&self) -> Option<&ClassStyleDeclarationWitness> {
        self.inline_paint.as_ref()
    }

    /// Returns one layout-affecting classDef typography declaration observed by the parser.
    pub fn classdef_typography_witness(&self) -> Option<&ClassStyleDeclarationWitness> {
        self.classdef_typography.as_ref()
    }

    #[cfg(feature = "presentation")]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.assignment_before_definition_copy
            .as_ref()
            .map_or(0, ClassStylePrecedenceWitness::retained_bytes)
            .saturating_add(
                self.definition_before_assignment_no_backfill
                    .as_ref()
                    .map_or(0, ClassStylePrecedenceWitness::retained_bytes),
            )
            .saturating_add(
                self.inline_paint
                    .as_ref()
                    .map_or(0, ClassStyleDeclarationWitness::retained_bytes),
            )
            .saturating_add(
                self.classdef_typography
                    .as_ref()
                    .map_or(0, ClassStyleDeclarationWitness::retained_bytes),
            )
    }
}

/// One parser-observed style declaration with its concrete target and encounter ordinal.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassStyleDeclarationWitness {
    pub(crate) target: String,
    pub(crate) styles: Vec<String>,
    pub(crate) style_event_ordinal: usize,
}

impl ClassStyleDeclarationWitness {
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn styles(&self) -> &[String] {
        &self.styles
    }

    pub const fn style_event_ordinal(&self) -> usize {
        self.style_event_ordinal
    }

    #[cfg(feature = "presentation")]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.target
            .len()
            .saturating_add(self.styles.iter().map(String::len).sum::<usize>())
    }
}

/// One parser-observed class style application witness.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassStylePrecedenceWitness {
    pub(crate) class_name: String,
    pub(crate) target: String,
    pub(crate) styles: Vec<String>,
    pub(crate) earlier_style_event_ordinal: usize,
    pub(crate) later_style_event_ordinal: usize,
}

impl ClassStylePrecedenceWitness {
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn styles(&self) -> &[String] {
        &self.styles
    }

    pub const fn earlier_style_event_ordinal(&self) -> usize {
        self.earlier_style_event_ordinal
    }

    pub const fn later_style_event_ordinal(&self) -> usize {
        self.later_style_event_ordinal
    }

    #[cfg(feature = "presentation")]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.class_name
            .len()
            .saturating_add(self.target.len())
            .saturating_add(self.styles.iter().map(String::len).sum::<usize>())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassNode {
    pub id: String,
    #[serde(rename = "type")]
    #[serde(default)]
    pub type_param: String,
    pub label: String,
    pub text: String,
    #[serde(rename = "cssClasses")]
    #[serde(default)]
    pub css_classes: String,
    #[serde(default)]
    pub methods: Vec<ClassMember>,
    #[serde(default)]
    pub members: Vec<ClassMember>,
    #[serde(default)]
    pub annotations: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(rename = "linkTarget")]
    #[serde(default)]
    pub link_target: Option<String>,
    #[serde(default)]
    pub tooltip: Option<String>,
    #[serde(rename = "haveCallback")]
    #[serde(default)]
    pub have_callback: bool,
    #[serde(default)]
    pub callback: Option<Map<String, Value>>,
    #[serde(rename = "callbackEffective")]
    #[serde(default)]
    pub callback_effective: bool,
}

#[cfg(feature = "presentation")]
impl ClassNode {
    /// Returns the visible title text used by Mermaid's SVG class renderer.
    pub fn title_text_for_render(&self) -> String {
        let decoded = crate::entities::decode_entities_minimal(self.text.trim());
        decoded.strip_prefix('\\').unwrap_or(&decoded).to_string()
    }

    /// Returns the first visible class annotation wrapped in Mermaid's guillemets.
    pub fn annotation_text_for_render(&self) -> Option<String> {
        self.annotations.first().map(|annotation| {
            format!(
                "\u{00AB}{}\u{00BB}",
                crate::entities::decode_entities_minimal(annotation.trim())
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassMember {
    #[serde(rename = "memberType")]
    pub member_type: String,
    pub visibility: String,
    pub id: String,
    pub classifier: String,
    pub parameters: String,
    #[serde(rename = "returnType")]
    pub return_type: String,
    #[serde(rename = "displayText")]
    pub display_text: String,
    #[serde(rename = "cssStyle")]
    pub css_style: String,
}

#[cfg(feature = "presentation")]
impl ClassMember {
    /// Returns the visible member or method text used by Mermaid's SVG class renderer.
    pub fn display_text_for_render(&self) -> String {
        let decoded = crate::entities::decode_entities_minimal(self.display_text.trim());
        decoded.strip_prefix('\\').unwrap_or(&decoded).to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassRelation {
    pub id: String,
    pub id1: String,
    pub id2: String,
    /// Authored label attached to the first endpoint, or `None` when no label was authored.
    ///
    /// Mermaid's compatibility JSON projects `None` as `"none"`; typed consumers must use this
    /// optional field rather than interpreting that boundary representation.
    #[serde(default, rename = "relationTitle1")]
    pub relation_title_1: Option<String>,
    /// Authored label attached to the second endpoint, or `None` when no label was authored.
    #[serde(default, rename = "relationTitle2")]
    pub relation_title_2: Option<String>,
    pub title: String,
    pub relation: RelationShape,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationShape {
    pub type1: i32,
    pub type2: i32,
    #[serde(rename = "lineType")]
    pub line_type: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassNote {
    pub id: String,
    #[serde(rename = "class")]
    pub class_id: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassInterface {
    pub id: String,
    pub label: String,
    #[serde(rename = "classId")]
    pub class_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Namespace {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(rename = "classIds")]
    #[serde(default)]
    pub class_ids: Vec<String>,
    #[serde(rename = "noteIds")]
    #[serde(default)]
    pub note_ids: Vec<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub explicit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleClass {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(rename = "textStyles")]
    #[serde(default)]
    pub text_styles: Vec<String>,
}

#[cfg(all(test, feature = "presentation"))]
mod tests {
    use super::{ClassMember, ClassNode};

    fn class_node(text: &str, annotations: &[&str]) -> ClassNode {
        ClassNode {
            id: "Service".to_string(),
            type_param: String::new(),
            label: "Service".to_string(),
            text: text.to_string(),
            css_classes: String::new(),
            methods: Vec::new(),
            members: Vec::new(),
            annotations: annotations
                .iter()
                .map(|annotation| (*annotation).to_string())
                .collect(),
            styles: Vec::new(),
            parent: None,
            link: None,
            link_target: None,
            tooltip: None,
            have_callback: false,
            callback: None,
            callback_effective: false,
        }
    }

    fn class_member(display_text: &str) -> ClassMember {
        ClassMember {
            member_type: "field".to_string(),
            visibility: "+".to_string(),
            id: "items".to_string(),
            classifier: String::new(),
            parameters: String::new(),
            return_type: String::new(),
            display_text: display_text.to_string(),
            css_style: String::new(),
        }
    }

    #[test]
    fn class_title_render_text_decodes_generics_and_removes_one_leading_escape() {
        let node = class_node(r#"  \\\Service&lt;T&gt;&amp;quot;  "#, &[]);

        assert_eq!(node.title_text_for_render(), "\\\\Service<T>\"");
    }

    #[test]
    fn class_annotation_render_text_uses_only_the_first_annotation() {
        let node = class_node("Service", &["  &amp;quot;service&amp;quot;  ", "ignored"]);

        assert_eq!(
            node.annotation_text_for_render().as_deref(),
            Some("\u{00AB}\"service\"\u{00BB}")
        );
    }

    #[test]
    fn class_member_render_text_decodes_and_removes_one_leading_escape() {
        let member = class_member(r#"  \\\+items&lt;T&gt;&amp;#39;  "#);

        assert_eq!(member.display_text_for_render(), "\\\\+items<T>'");
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassConstants {
    #[serde(rename = "lineType")]
    pub line_type: ClassLineTypeConstants,
    #[serde(rename = "relationType")]
    pub relation_type: ClassRelationTypeConstants,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassLineTypeConstants {
    pub line: i32,
    #[serde(rename = "dottedLine")]
    pub dotted_line: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassRelationTypeConstants {
    pub none: i32,
    pub aggregation: i32,
    pub extension: i32,
    pub composition: i32,
    pub dependency: i32,
    pub lollipop: i32,
}
