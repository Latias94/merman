//! Ordered C4 database mutations and typed model projection.

use super::{C4Diagnostic, C4Result};
use merman_markup::sanitize::sanitize_text;
use merman_model::MermaidConfig;
use merman_model::families::c4::{
    C4Boundary, C4DiagramModel, C4LayoutConfig, C4Relation, C4RelationOffset, C4RowPreferences,
    C4Shape, C4Text, parse_integer_prefix,
};
use serde_json::{Map, Value, json};

#[derive(Debug, Default)]
pub(super) struct C4Db {
    c4_type: String,
    title: String,
    acc_descr: String,
    wrap_enabled: bool,

    boundaries: Vec<C4Boundary>,

    shapes: Vec<C4Shape>,

    rels: Vec<C4RelationRecord>,

    c4_shape_in_row: f64,
    c4_boundary_in_row: f64,
    pub(super) row_preferences: C4RowPreferences,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum C4Arg {
    Text(String),
    Named { key: String, value: String },
}

impl C4Arg {
    pub(super) fn value(&self) -> &str {
        match self {
            Self::Text(value) | Self::Named { value, .. } => value,
        }
    }

    fn key_or<'a>(&'a self, positional_key: &'a str) -> &'a str {
        match self {
            Self::Text(_) => positional_key,
            Self::Named { key, .. } => key,
        }
    }

    fn to_value(&self) -> Value {
        match self {
            Self::Text(value) => json!(value),
            Self::Named { key, value } => {
                let mut map = Map::new();
                map.insert(key.clone(), json!(value));
                Value::Object(map)
            }
        }
    }
}

impl C4Db {
    pub(super) fn new(config: &MermaidConfig) -> Self {
        // Mermaid captures the root `wrap` directive while parsing and stores it on
        // each C4 source object. The renderer's diagram-local `c4.wrap` setting is
        // applied later as the second half of the `object.wrap && conf.wrap` check.
        let wrap_enabled = config.get_bool("wrap").unwrap_or(false);
        let mut db = Self {
            wrap_enabled,
            c4_shape_in_row: C4LayoutConfig::DEFAULT_CARD_COLUMNS,
            c4_boundary_in_row: C4LayoutConfig::DEFAULT_BOUNDARY_COLUMNS,
            ..Default::default()
        };
        db.add_global_boundary();
        db
    }

    fn add_global_boundary(&mut self) {
        self.boundaries.push(C4Boundary {
            alias: "global".to_string(),
            label: C4Text::Wrapped {
                text: Value::from("global"),
            },
            ty: Some(C4Text::Wrapped {
                text: Value::from("global"),
            }),
            ..Default::default()
        });
    }

    fn ensure_shape(&mut self, alias: &str) -> &mut C4Shape {
        if let Some(index) = self.shapes.iter().position(|shape| shape.alias == alias) {
            return &mut self.shapes[index];
        }
        self.shapes.push(C4Shape {
            alias: alias.to_string(),
            ..Default::default()
        });
        self.shapes.last_mut().expect("shape was just inserted")
    }

    fn ensure_boundary(&mut self, alias: &str) -> &mut C4Boundary {
        if let Some(index) = self
            .boundaries
            .iter()
            .position(|boundary| boundary.alias == alias)
        {
            return &mut self.boundaries[index];
        }
        self.boundaries.push(C4Boundary {
            alias: alias.to_string(),
            ..Default::default()
        });
        self.boundaries
            .last_mut()
            .expect("boundary was just inserted")
    }

    fn ensure_relation(&mut self, from: &str, to: &str) -> (&mut C4RelationRecord, bool) {
        if let Some(index) = self
            .rels
            .iter()
            .position(|rel| rel.matches_endpoints(from, to))
        {
            return (&mut self.rels[index], true);
        }
        self.rels.push(C4RelationRecord::default());
        (
            self.rels.last_mut().expect("relation was just inserted"),
            false,
        )
    }

    pub(super) fn set_c4_type(&mut self, raw: &str, config: &MermaidConfig) {
        self.c4_type = sanitize_text(raw, config);
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn is_dynamic(&self) -> bool {
        self.c4_type == "C4Dynamic"
    }

    pub(super) fn set_title(&mut self, raw: &str, config: &MermaidConfig) {
        self.title = sanitize_text(raw, config);
    }

    pub(super) fn set_acc_description(&mut self, raw: &str) {
        self.acc_descr = raw.to_string();
    }

    pub(super) fn add_person_or_system(
        &mut self,
        type_c4_shape: &str,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> C4Result<()> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_shape(&alias);
        obj.assign("label", wrap_text(label));
        assign_text_argument(obj, "descr", args.get(2), "");
        assign_optional_argument(obj, "sprite", args.get(3));
        assign_optional_argument(obj, "tags", args.get(4));
        assign_optional_argument(obj, "link", args.get(5));
        obj.assign("typeC4Shape", wrap_text(json!(type_c4_shape)));
        obj.assign("parentBoundary", json!(parent_boundary));
        obj.assign("wrap", json!(wrap_enabled));
        Ok(())
    }

    pub(super) fn add_container(
        &mut self,
        type_c4_shape: &str,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> C4Result<()> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_shape(&alias);
        obj.assign("label", wrap_text(label));
        assign_text_argument(obj, "techn", args.get(2), "");
        assign_text_argument(obj, "descr", args.get(3), "");
        assign_optional_argument(obj, "sprite", args.get(4));
        assign_optional_argument(obj, "tags", args.get(5));
        assign_optional_argument(obj, "link", args.get(6));
        obj.assign("wrap", json!(wrap_enabled));
        obj.assign("typeC4Shape", wrap_text(json!(type_c4_shape)));
        obj.assign("parentBoundary", json!(parent_boundary));
        Ok(())
    }

    pub(super) fn add_component(
        &mut self,
        type_c4_shape: &str,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> C4Result<()> {
        self.add_container(type_c4_shape, args, parent_boundary)
    }

    pub(super) fn add_person_or_system_boundary(
        &mut self,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> C4Result<String> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_boundary(&alias);
        obj.assign("label", wrap_text(label));
        assign_text_argument(obj, "type", args.get(2), "system");

        assign_optional_argument(obj, "tags", args.get(3));
        assign_optional_argument(obj, "link", args.get(4));

        obj.assign("parentBoundary", json!(parent_boundary));
        obj.assign("wrap", json!(wrap_enabled));

        Ok(alias)
    }

    pub(super) fn add_container_boundary(
        &mut self,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> C4Result<String> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_boundary(&alias);
        obj.assign("label", wrap_text(label));
        assign_text_argument(obj, "type", args.get(2), "container");
        assign_optional_argument(obj, "tags", args.get(3));
        assign_optional_argument(obj, "link", args.get(4));
        obj.assign("parentBoundary", json!(parent_boundary));
        obj.assign("wrap", json!(wrap_enabled));

        Ok(alias)
    }

    pub(super) fn add_deployment_node(
        &mut self,
        node_type: &str,
        args: &[C4Arg],
        parent_boundary: &str,
    ) -> C4Result<String> {
        let alias = positional_arg_to_string(args.first())?;
        let label = c4_arg_value_or_empty(args.get(1));

        let wrap_enabled = self.wrap_enabled;
        let obj = self.ensure_boundary(&alias);
        obj.assign("label", wrap_text(label));

        assign_text_argument(obj, "type", args.get(2), "node");
        assign_text_argument(obj, "descr", args.get(3), "");
        assign_optional_argument(obj, "tags", args.get(5));
        assign_optional_argument(obj, "link", args.get(6));

        obj.assign("nodeType", json!(node_type));
        obj.assign("parentBoundary", json!(parent_boundary));
        obj.assign("wrap", json!(wrap_enabled));

        Ok(alias)
    }

    /// Returns whether the declaration overwrote an existing relationship.
    pub(super) fn add_rel(&mut self, rel_type: &str, args: &[C4Arg]) -> C4Result<bool> {
        let from = positional_arg_to_string(args.first())?;
        let to = positional_arg_to_string(args.get(1))?;
        let Some(label) = args.get(2).map(C4Arg::to_value) else {
            return Ok(false);
        };

        let wrap_enabled = self.wrap_enabled;
        let (rel, overwritten) = self.ensure_relation(&from, &to);

        rel.assign("type", json!(rel_type));
        rel.assign("from", json!(from));
        rel.assign("to", json!(to));
        rel.assign("label", wrap_text(label));

        assign_text_argument(rel, "techn", args.get(3), "");
        assign_text_argument(rel, "descr", args.get(4), "");

        assign_optional_argument(rel, "sprite", args.get(5));
        assign_optional_argument(rel, "tags", args.get(6));
        assign_optional_argument(rel, "link", args.get(7));
        rel.assign("wrap", json!(wrap_enabled));
        Ok(overwritten)
    }

    pub(super) fn update_el_style(&mut self, args: &[C4Arg]) -> C4Result<()> {
        let element_name = positional_arg_to_string(args.first())?;
        if let Some(target) = self
            .shapes
            .iter_mut()
            .find(|element| element.alias == element_name)
        {
            apply_element_style_arguments(target, args);
        } else if let Some(target) = self
            .boundaries
            .iter_mut()
            .find(|element| element.alias == element_name)
        {
            apply_element_style_arguments(target, args);
        }
        Ok(())
    }

    pub(super) fn update_rel_style(&mut self, args: &[C4Arg]) -> C4Result<()> {
        let from = positional_arg_to_string(args.first())?;
        let to = positional_arg_to_string(args.get(1))?;

        let Some(target) = self
            .rels
            .iter_mut()
            .find(|rel| rel.matches_endpoints(&from, &to))
        else {
            return Ok(());
        };

        apply_relation_style_argument(target, "textColor", args.get(2));
        apply_relation_style_argument(target, "lineColor", args.get(3));
        apply_relation_style_argument(target, "offsetX", args.get(4));
        apply_relation_style_argument(target, "offsetY", args.get(5));
        Ok(())
    }

    pub(super) fn update_layout_config(&mut self, args: &[C4Arg]) -> C4Result<()> {
        for ((positional, legacy), arg) in [
            ("c4ShapeInRow", &mut self.c4_shape_in_row),
            ("c4BoundaryInRow", &mut self.c4_boundary_in_row),
        ]
        .into_iter()
        .zip(args)
        {
            let Some(value) = parse_integer_prefix(arg.value()).filter(|&value| value >= 1.0)
            else {
                continue;
            };
            *legacy = value;
            let preference = match arg.key_or(positional) {
                "c4ShapeInRow" => &mut self.row_preferences.cards,
                "c4BoundaryInRow" => &mut self.row_preferences.boundaries,
                _ => continue,
            };
            *preference = Some(value);
        }
        Ok(())
    }

    pub(super) fn into_model(self) -> C4Result<C4DiagramModel> {
        Ok(C4DiagramModel {
            c4_type: self.c4_type,
            title: (!self.title.is_empty()).then_some(self.title),
            acc_title: None,
            acc_descr: (!self.acc_descr.is_empty()).then_some(self.acc_descr),
            wrap: self.wrap_enabled,
            layout: C4LayoutConfig {
                c4_shape_in_row: self.c4_shape_in_row,
                c4_boundary_in_row: self.c4_boundary_in_row,
            },
            shapes: self.shapes,
            boundaries: self.boundaries,
            rels: self
                .rels
                .into_iter()
                .map(C4RelationRecord::into_model)
                .collect::<C4Result<_>>()?,
        })
    }
}

// C4 permits named attributes to rewrite fields outside their positional slots.
// Interpret that dynamic assignment once, at the typed database owner.
trait C4Attributes {
    fn assign(&mut self, key: &str, value: Value);
    fn has_text_attribute(&self, key: &str) -> bool;
}

impl C4Attributes for C4Shape {
    fn assign(&mut self, key: &str, value: Value) {
        match key {
            "alias" => self.alias = c4_string_argument(value),
            "parentBoundary" => self.parent_boundary = c4_string_argument(value),
            "typeC4Shape" => self.type_c4_shape = c4_text_from_value(value),
            "label" => self.label = c4_text_from_value(value),
            "wrap" => self.wrap = c4_truthy(&value),
            "type" => self.ty = Some(c4_text_from_value(value)),
            "techn" => self.techn = Some(c4_text_from_value(value)),
            "descr" => self.descr = Some(c4_text_from_value(value)),
            "sprite" => self.sprite = Some(value),
            "tags" => self.tags = Some(value),
            "link" => self.link = Some(value),
            "bgColor" => self.bg_color = Some(c4_string_argument(value)),
            "borderColor" => self.border_color = Some(c4_string_argument(value)),
            "fontColor" => self.font_color = Some(c4_string_argument(value)),
            "shadowing" => self.shadowing = Some(value),
            "shape" => self.shape = Some(value),
            "legendText" => self.legend_text = Some(value),
            "legendSprite" => self.legend_sprite = Some(value),
            _ => {}
        }
    }

    fn has_text_attribute(&self, key: &str) -> bool {
        match key {
            "type" => self.ty.is_some(),
            "techn" => self.techn.is_some(),
            "descr" => self.descr.is_some(),
            _ => false,
        }
    }
}

impl C4Attributes for C4Boundary {
    fn assign(&mut self, key: &str, value: Value) {
        match key {
            "alias" => self.alias = c4_string_argument(value),
            "parentBoundary" => self.parent_boundary = c4_string_argument(value),
            "label" => self.label = c4_text_from_value(value),
            "type" => self.ty = Some(c4_text_from_value(value)),
            "descr" => self.descr = Some(c4_text_from_value(value)),
            "wrap" => self.wrap = Some(c4_truthy(&value)),
            "sprite" => self.sprite = Some(value),
            "tags" => self.tags = Some(value),
            "link" => self.link = Some(value),
            "nodeType" => self.node_type = Some(c4_string_argument(value)),
            "bgColor" => self.bg_color = Some(c4_string_argument(value)),
            "borderColor" => self.border_color = Some(c4_string_argument(value)),
            "fontColor" => self.font_color = Some(c4_string_argument(value)),
            "shadowing" => self.shadowing = Some(value),
            "shape" => self.shape = Some(value),
            "techn" => self.techn = Some(value),
            "legendText" => self.legend_text = Some(value),
            "legendSprite" => self.legend_sprite = Some(value),
            _ => {}
        }
    }

    fn has_text_attribute(&self, key: &str) -> bool {
        match key {
            "type" => self.ty.is_some(),
            "techn" => self.techn.is_some(),
            "descr" => self.descr.is_some(),
            _ => false,
        }
    }
}

#[derive(Debug, Default)]
struct C4RelationRecord {
    model: C4Relation,
    // A named text argument can temporarily replace the required string type with {text: ...}.
    // Later redeclarations/style assignments may repair it before final model validation.
    invalid_type: Option<Value>,
    invalid_string_fields: std::collections::BTreeMap<String, Value>,
}

impl C4RelationRecord {
    fn matches_endpoints(&self, from: &str, to: &str) -> bool {
        // Upstream uses strict equality: a numeric endpoint cannot match an authored alias.
        !self.invalid_string_fields.contains_key("from")
            && !self.invalid_string_fields.contains_key("to")
            && self.model.from_alias == from
            && self.model.to_alias == to
    }

    fn into_model(self) -> C4Result<C4Relation> {
        if let Some(value) = self.invalid_type {
            return Err(C4Diagnostic::fallback(format!(
                "expected string field `type`, got {value:?}"
            )));
        }
        if let Some((key, value)) = self.invalid_string_fields.into_iter().next() {
            return Err(C4Diagnostic::fallback(format!(
                "expected string field `{key}`, got {value:?}"
            )));
        }
        Ok(self.model)
    }
}

impl C4Attributes for C4RelationRecord {
    fn assign(&mut self, key: &str, value: Value) {
        if matches!(key, "from" | "to" | "lineColor" | "textColor") {
            if !value.is_string() {
                self.invalid_string_fields.insert(key.to_string(), value);
                return;
            }
            self.invalid_string_fields.remove(key);
        }
        match key {
            "from" => self.model.from_alias = c4_string_argument(value),
            "to" => self.model.to_alias = c4_string_argument(value),
            "type" => match value {
                Value::String(value) => {
                    self.model.rel_type = value;
                    self.invalid_type = None;
                }
                value => self.invalid_type = Some(value),
            },
            "label" => self.model.label = c4_text_from_value(value),
            "techn" => self.model.techn = Some(c4_text_from_value(value)),
            "descr" => self.model.descr = Some(c4_text_from_value(value)),
            "wrap" => self.model.wrap = c4_truthy(&value),
            "sprite" => self.model.sprite = Some(value),
            "tags" => self.model.tags = Some(value),
            "link" => self.model.link = Some(value),
            "offsetX" => self.model.offset_x = c4_relation_offset(value),
            "offsetY" => self.model.offset_y = c4_relation_offset(value),
            "lineColor" => self.model.line_color = Some(c4_string_argument(value)),
            "textColor" => self.model.text_color = Some(c4_string_argument(value)),
            _ => {}
        }
    }

    fn has_text_attribute(&self, key: &str) -> bool {
        match key {
            "type" => true,
            "techn" => self.model.techn.is_some(),
            "descr" => self.model.descr.is_some(),
            _ => false,
        }
    }
}

fn c4_string_argument(value: Value) -> String {
    let Value::String(value) = value else {
        unreachable!("C4 string fields receive string macro arguments")
    };
    value
}

fn c4_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn c4_text_from_value(value: Value) -> C4Text {
    match value {
        Value::Object(mut map) if map.contains_key("text") => C4Text::Wrapped {
            text: map.remove("text").expect("text key was checked"),
        },
        Value::String(value) => C4Text::String(value),
        value => C4Text::Value(value),
    }
}

fn wrap_text(value: Value) -> Value {
    json!({"text": value})
}

fn apply_element_style_arguments(target: &mut impl C4Attributes, args: &[C4Arg]) {
    for (key, arg) in [
        "bgColor",
        "fontColor",
        "borderColor",
        "shadowing",
        "shape",
        "sprite",
        "techn",
        "legendText",
        "legendSprite",
    ]
    .into_iter()
    .zip(args.iter().skip(1))
    {
        apply_update_argument(target, key, Some(arg));
    }
}

fn positional_arg_to_string(arg: Option<&C4Arg>) -> C4Result<String> {
    match arg {
        None => Ok(String::new()),
        Some(C4Arg::Text(value)) => Ok(value.clone()),
        Some(C4Arg::Named { key, .. }) => Err(C4Diagnostic::fallback(format!(
            "expected positional string argument, got named argument `${key}`"
        ))),
    }
}

fn c4_arg_value_or_empty(arg: Option<&C4Arg>) -> Value {
    arg.map(C4Arg::to_value).unwrap_or_else(|| json!(""))
}

fn assign_text_argument(
    obj: &mut impl C4Attributes,
    positional_key: &str,
    arg: Option<&C4Arg>,
    missing_value: &str,
) {
    match arg {
        None => {
            // Boundary type defaults reset on redeclaration; text descriptions do not.
            if positional_key == "type" {
                obj.assign(positional_key, wrap_text(json!(missing_value)));
            } else {
                if !obj.has_text_attribute(positional_key) {
                    obj.assign(positional_key, wrap_text(json!(missing_value)));
                }
            }
        }
        Some(C4Arg::Text(value)) => {
            obj.assign(positional_key, wrap_text(json!(value)));
        }
        Some(C4Arg::Named { key, value }) => assign_named_argument(obj, key, value),
    }
}

fn assign_optional_argument(
    obj: &mut impl C4Attributes,
    positional_key: &str,
    arg: Option<&C4Arg>,
) {
    match arg {
        // A named argument may occupy an earlier positional slot.  A later
        // missing positional value must not clobber the named value that was
        // already applied to the object.
        None => {}
        Some(C4Arg::Text(value)) => {
            obj.assign(positional_key, json!(value));
        }
        Some(C4Arg::Named { key, value }) => assign_named_argument(obj, key, value),
    }
}

fn assign_named_argument(obj: &mut impl C4Attributes, key: &str, value: &str) {
    let value = json!(value);
    let value = if matches!(key, "label" | "descr" | "techn" | "type") {
        wrap_text(value)
    } else {
        value
    };
    obj.assign(key, value);
}

fn apply_update_argument(obj: &mut impl C4Attributes, positional_key: &str, arg: Option<&C4Arg>) {
    let Some(arg) = arg else {
        return;
    };

    obj.assign(arg.key_or(positional_key), json!(arg.value()));
}

fn apply_relation_style_argument(
    obj: &mut C4RelationRecord,
    positional_key: &str,
    arg: Option<&C4Arg>,
) {
    let Some(arg) = arg else {
        return;
    };

    let key = arg.key_or(positional_key);
    if matches!(key, "offsetX" | "offsetY") {
        let offset = if matches!(positional_key, "offsetX" | "offsetY") {
            C4RelationOffset::Number(parse_integer_prefix(arg.value()).unwrap_or(f64::NAN))
        } else {
            C4RelationOffset::String(arg.value().to_string())
        };
        match key {
            "offsetX" => obj.model.offset_x = Some(offset),
            _ => obj.model.offset_y = Some(offset),
        }
    } else {
        let value = if matches!(positional_key, "offsetX" | "offsetY") {
            // Conversion belongs to the argument slot, even when a named argument
            // targets another property such as wrap, label, or tags.
            let parsed = parse_integer_prefix(arg.value()).unwrap_or(f64::NAN);
            if key == "wrap" {
                // JSON's null projection would lose Infinity's truthiness.
                obj.model.wrap = !parsed.is_nan() && parsed != 0.0;
                return;
            }
            merman_model::json::number_value(parsed)
        } else {
            json!(arg.value())
        };
        obj.assign(key, value);
    }
}

fn c4_relation_offset(value: Value) -> Option<C4RelationOffset> {
    match value {
        Value::Number(number) => number.as_f64().map(C4RelationOffset::Number),
        Value::String(text) => Some(C4RelationOffset::String(text)),
        _ => None,
    }
}
