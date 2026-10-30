//! Canonical Cynefin domains, items, and transitions.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinItemModel {
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinDomainModel {
    pub name: String,
    #[serde(default)]
    pub items: Vec<CynefinItemModel>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinTransitionModel {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub domains: Vec<CynefinDomainModel>,
    #[serde(default)]
    pub transitions: Vec<CynefinTransitionModel>,
}
