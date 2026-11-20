//! Complete C4 records over the shared canonical and presentation model envelope.
//!
//! Reading keys follow each source-ordered collection, including the synthetic global boundary.
//! Endpoint aliases resolve directly, with shapes preceding boundaries. Text records retain the
//! string/wrapped-string distinction; every other JSON value stays complete in borrowed details.

use super::record::{RecordWriter, details, write_plain_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::families::c4::C4DiagramModel;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

type Fields<'a> = BTreeMap<String, &'a RawValue>;

#[derive(Default)]
struct AliasKeys {
    shapes: Vec<usize>,
    boundaries: Vec<usize>,
}

struct Identities<'a> {
    by_alias: HashMap<&'a str, AliasKeys>,
    resources: ResourceContext,
}

pub(super) fn write_if_c4(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::C4(model) = model else {
        return Ok(false);
    };
    lines.admit_node(0)?;
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    let mut root: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let shapes = take_array(&mut root, "shapes", model.shapes.len(), lines)?;
    let boundaries = take_array(&mut root, "boundaries", model.boundaries.len(), lines)?;
    let relations = take_array(&mut root, "rels", model.rels.len(), lines)?;
    let identities = Identities::new(model, lines)?;

    if let Some(view) = crate::c4::view_name(&model.c4_type) {
        let mut record = RecordWriter::new(lines)?;
        record.text("C4 view=", false)?;
        record.text(view, true)?;
        record.finish()?;
        root.remove("c4Type");
    }
    write_plain_record(
        "Keys: S shapes; B boundaries (including global); E relationships. Each collection keeps source order. Relationship endpoints use literal aliases; shapes precede boundaries.",
        lines,
    )?;
    write_plain_record(
        "Relationships name authored source then target. rel_b has a source head; birel has both heads; other known relations have a target head. Dynamic render numbers follow final relationship order.",
        lines,
    )?;
    for (index, (shape, raw)) in model.shapes.iter().zip(shapes).enumerate() {
        let key = format!("S{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [shape] alias="), false)?;
        record.text(&shape.alias, true)?;
        fields.remove("alias");
        if fields.remove("parentBoundary").is_some() {
            write_parent(&shape.parent_boundary, &identities, &mut record)?;
        }
        record.finish()?;
        write_text_fields(
            &key,
            &["typeC4Shape", "label", "type", "techn", "descr"],
            &mut fields,
            lines,
        )?;
        details(&key, &fields, lines)?;
        identities.disclose_overlap(&shape.alias, 'S', index + 1, lines)?;
    }
    for (index, (boundary, raw)) in model.boundaries.iter().zip(boundaries).enumerate() {
        let key = format!("B{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [boundary] alias="), false)?;
        record.text(&boundary.alias, true)?;
        fields.remove("alias");
        if fields.remove("parentBoundary").is_some() {
            write_parent(&boundary.parent_boundary, &identities, &mut record)?;
        }
        record.finish()?;
        write_text_fields(
            &key,
            &["label", "type", "techn", "descr"],
            &mut fields,
            lines,
        )?;
        details(&key, &fields, lines)?;
        identities.disclose_overlap(&boundary.alias, 'B', index + 1, lines)?;
    }
    for (index, (relation, raw)) in model.rels.iter().zip(relations).enumerate() {
        let key = format!("E{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [relationship] type="), false)?;
        record.text(&relation.rel_type, true)?;
        record.text(" source=", false)?;
        identities.reference(&relation.from_alias, false, &mut record)?;
        record.text(" target=", false)?;
        identities.reference(&relation.to_alias, false, &mut record)?;
        record.text(" heads=", false)?;
        record.text(
            match relation.rel_type.as_str() {
                "rel_b" => "source",
                "birel" => "both",
                "rel" | "rel_u" | "rel_d" | "rel_l" | "rel_r" => "target",
                _ => "unspecified",
            },
            false,
        )?;
        if model.c4_type == "C4Dynamic" {
            record.text(&format!(" render-number={}", index + 1), false)?;
        }
        record.finish()?;
        for name in ["type", "from", "to"] {
            fields.remove(name);
        }
        write_text_fields(&key, &["label", "techn", "descr"], &mut fields, lines)?;
        details(&key, &fields, lines)?;
    }
    // Unknown c4Type values, model/source titles, accessibility, layout, wrap, source
    // configuration, and future metadata remain complete raw fields. Known view names above
    // have a one-to-one mapping to the discriminator; authored strings are never rewritten.
    details("c4", &root, lines)?;
    Ok(true)
}

fn write_parent(
    alias: &str,
    identities: &Identities<'_>,
    record: &mut RecordWriter<'_, '_>,
) -> Result<()> {
    record.text(" parentBoundary=", false)?;
    if alias.is_empty() {
        record.text("[] (root) alias=", false)?;
        record.text(alias, true)
    } else {
        identities.reference(alias, true, record)
    }
}

/// C4Text accepts wrapped or direct JSON of any type. Consume a field only after rendering
/// its complete ordinary-text representation; typed values and extended wrappers stay raw.
fn write_text_fields(
    key: &str,
    names: &[&str],
    fields: &mut Fields<'_>,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    for &name in names {
        let Some(raw) = fields.get(name).copied() else {
            continue;
        };
        let (text, wrapped): (String, bool) = match raw.get().trim_start().as_bytes().first() {
            Some(b'"') => (decode(raw, lines)?, false),
            Some(b'{') => {
                let wrapper: Fields<'_> = decode(raw, lines)?;
                let Some(text) = wrapper.get("text").copied().filter(|_| wrapper.len() == 1) else {
                    continue;
                };
                if !text.get().trim_start().starts_with('"') {
                    continue;
                }
                (decode(text, lines)?, true)
            }
            _ => continue,
        };
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} {name}="), false)?;
        if wrapped {
            record.text("{text:", false)?;
        }
        record.text(&text, true)?;
        if wrapped {
            record.text("}", false)?;
        }
        record.finish()?;
        fields.remove(name);
    }
    Ok(())
}

impl<'a> Identities<'a> {
    fn new(model: &'a C4DiagramModel, lines: &SemanticFallbackWriter<'_>) -> Result<Self> {
        let resources = lines.semantic_resources.clone();
        let count = resources.checked_work_add(model.shapes.len(), model.boundaries.len())?;
        resources.charge_layout_work(count)?;
        resources.check(AsciiResourceLimitId::MaxDocumentCells, count)?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, count)?;
        let mut by_alias: HashMap<&str, AliasKeys> = HashMap::new();
        by_alias
            .try_reserve(count)
            .map_err(|_| allocation_failed())?;
        for (prefix, index, alias) in model
            .shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| ('S', index, shape.alias.as_str()))
            .chain(
                model
                    .boundaries
                    .iter()
                    .enumerate()
                    .map(|(index, boundary)| ('B', index, boundary.alias.as_str())),
            )
        {
            resources.charge_layout_work(alias.len().max(1))?;
            let keys = by_alias.entry(alias).or_default();
            let keys = if prefix == 'S' {
                &mut keys.shapes
            } else {
                &mut keys.boundaries
            };
            keys.try_reserve(1).map_err(|_| allocation_failed())?;
            keys.push(index + 1);
        }
        Ok(Self {
            by_alias,
            resources,
        })
    }

    fn reference(
        &self,
        alias: &str,
        only_boundaries: bool,
        record: &mut RecordWriter<'_, '_>,
    ) -> Result<()> {
        self.resources.charge_layout_work(alias.len().max(1))?;
        let identity = self.by_alias.get(alias);
        let shapes = identity.map_or(&[][..], |keys| keys.shapes.as_slice());
        let boundaries = identity.map_or(&[][..], |keys| keys.boundaries.as_slice());
        let use_shapes = !only_boundaries && !shapes.is_empty();
        let (prefix, keys) = if use_shapes {
            ('S', shapes)
        } else {
            ('B', boundaries)
        };
        self.write_keys(prefix, keys, record)?;
        if keys.is_empty() {
            record.text(" (unresolved)", false)?;
        } else if keys.len() > 1 {
            record.text(" (ambiguous)", false)?;
        }
        record.text(" alias=", false)?;
        record.text(alias, true)?;
        if use_shapes && !boundaries.is_empty() {
            record.text(" shadowed-boundaries=", false)?;
            self.write_keys('B', boundaries, record)?;
        }
        Ok(())
    }

    fn write_keys(
        &self,
        prefix: char,
        keys: &[usize],
        record: &mut RecordWriter<'_, '_>,
    ) -> Result<()> {
        self.resources.charge_layout_work(keys.len())?;
        record.text("[", false)?;
        for (position, number) in keys.iter().enumerate() {
            if position > 0 {
                record.text(",", false)?;
            }
            record.text(&format!("{prefix}{number}"), false)?;
        }
        record.text("]", false)
    }

    fn disclose_overlap(
        &self,
        alias: &str,
        prefix: char,
        number: usize,
        lines: &mut SemanticFallbackWriter<'_>,
    ) -> Result<()> {
        self.resources.charge_layout_work(alias.len().max(1))?;
        let keys = self.by_alias.get(alias).ok_or_else(invalid_projection)?;
        let first = keys
            .shapes
            .first()
            .map(|&number| ('S', number))
            .or_else(|| keys.boundaries.first().map(|&number| ('B', number)));
        let overlap = !keys.shapes.is_empty() && !keys.boundaries.is_empty();
        let duplicate = keys.shapes.len() > 1 || keys.boundaries.len() > 1;
        // Emit each alias diagnostic once, even if many declarations share that alias.
        if first != Some((prefix, number)) || !(overlap || duplicate) {
            return Ok(());
        }
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{prefix}{number} identity diagnostic: "), false)?;
        if duplicate {
            record.text("duplicate alias within a collection; ", false)?;
        }
        if overlap {
            record.text("shape lookup precedes boundary lookup; ", false)?;
        }
        record.text("shapes=", false)?;
        self.write_keys('S', &keys.shapes, &mut record)?;
        record.text(" boundaries=", false)?;
        self.write_keys('B', &keys.boundaries, &mut record)?;
        record.text(" alias=", false)?;
        record.text(alias, true)?;
        record.finish()
    }
}

fn decode<'a, T: Deserialize<'a>>(
    raw: &'a RawValue,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<T> {
    lines.admit_node(1)?;
    lines
        .semantic_resources
        .charge_layout_work(raw.get().len())?;
    serde_json::from_str(raw.get()).map_err(|_| invalid_projection())
}

fn take_array<'a>(
    fields: &mut Fields<'a>,
    name: &str,
    expected: usize,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Vec<&'a RawValue>> {
    let values: Vec<_> = decode(fields.remove(name).ok_or_else(invalid_projection)?, lines)?;
    if values.len() != expected {
        return Err(invalid_projection());
    }
    Ok(values)
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "c4",
        feature: "invalid serialized semantic projection",
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_semantic_fallback")
}
