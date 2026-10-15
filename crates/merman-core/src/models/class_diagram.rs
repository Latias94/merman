use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassDiagram {
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
}

impl ClassDiagram {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

/// Parser-owned witnesses for ClassDiagram encounter-order styling behavior.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

    pub(crate) fn retained_bytes(&self) -> usize {
        self.target
            .len()
            .saturating_add(self.styles.iter().map(String::len).sum::<usize>())
    }
}

/// One parser-observed class style application witness.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
    #[serde(rename = "domId")]
    pub dom_id: String,
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
    #[serde(rename = "domId")]
    pub dom_id: String,
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

#[cfg(test)]
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
            dom_id: "classId-Service-0".to_string(),
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
