//! Canonical entity-relationship graph and vocabulary.

use indexmap::IndexMap;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub direction: String,
    #[serde(default)]
    pub classes: IndexMap<String, ErClassDef>,
    #[serde(default)]
    pub entities: IndexMap<String, ErEntity>,
    #[serde(default)]
    pub relationships: Vec<ErRelationship>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subgraphs: Vec<ErSubgraph>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErAttribute {
    #[serde(rename = "type")]
    pub ty: String,
    pub name: String,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErRelSpec {
    #[serde(rename = "cardA")]
    pub card_a: String,
    #[serde(rename = "cardB")]
    pub card_b: String,
    #[serde(rename = "relType")]
    pub rel_type: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErRelationship {
    #[serde(rename = "entityA")]
    pub entity_a: String,
    #[serde(default, rename = "roleA")]
    pub role_a: String,
    #[serde(rename = "entityB")]
    pub entity_b: String,
    #[serde(default, rename = "relSpec")]
    pub rel_spec: ErRelSpec,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErClassDef {
    pub id: String,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErSubgraph {
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default, rename = "cssStyles")]
    pub css_styles: Vec<String>,
    #[serde(default)]
    pub dir: Option<String>,
    pub id: String,
    #[serde(default, rename = "labelType")]
    pub label_type: String,
    #[serde(default)]
    pub nodes: Vec<String>,
    pub title: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ErEntity {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub attributes: Vec<ErAttribute>,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub shape: String,
    #[serde(default, rename = "cssClasses")]
    pub css_classes: String,
    #[serde(default, rename = "cssStyles")]
    pub css_styles: Vec<String>,
}

impl ErEntity {
    /// Returns the visible entity label source selected by Mermaid's ER renderer.
    #[cfg(feature = "presentation-data")]
    pub fn label_source_for_render(&self) -> &str {
        if self.alias.is_empty() {
            self.label.as_str()
        } else {
            self.alias.as_str()
        }
    }
}
