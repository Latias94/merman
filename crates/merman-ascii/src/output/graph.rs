//! A complete graph document over the existing bounded semantic projection.
//!
//! N/G keys are reader identities, E keys identify individual relationships. Remaining authored
//! properties stay with their owner in compact detail records. Raw slices borrow the one admitted
//! serialization; this adapter neither serializes the model again nor reconstructs a JSON tree.
//!
//! Flow nodes are declared with their full quoted label and type at first relationship use, then
//! referenced by key. Group declarations stay scoped; isolated nodes are declared separately.
//! Relationships always name source then target, including headless and invisible constraints.
//! Only non-obvious marker/stroke distinctions receive a legend. State's declarations and complete
//! authored-document records retain their separate ownership.

use super::{
    AsciiDiagram, AsciiError, AsciiResourceLimitId, RawValue, Result, SemanticFallbackWriter,
    display_width_with_profile,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use unicode_segmentation::UnicodeSegmentation;

type Fields<'a> = BTreeMap<String, &'a RawValue>;
type Identities = BTreeMap<String, String>;

struct Entity<'a> {
    key: String,
    fields: Fields<'a>,
}

struct NodeRecord<'a> {
    key: String,
    prefix: String,
    label: NodeLabel<'a>,
    fields: Fields<'a>,
    parent: Option<String>,
}

enum NodeLabel<'a> {
    Text(String),
    Structured(&'a RawValue),
}

enum RecordPart<'a> {
    Plain(&'a str),
    Quoted(&'a str),
}

impl NodeRecord<'_> {
    fn parts(&self) -> [RecordPart<'_>; 2] {
        [
            RecordPart::Plain(&self.prefix),
            match &self.label {
                NodeLabel::Text(label) => RecordPart::Quoted(label),
                NodeLabel::Structured(raw) => RecordPart::Plain(raw.get()),
            },
        ]
    }
}

pub(super) fn write_if_graph(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let state = match model {
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        AsciiDiagram::Flowchart(_) => false,
        #[cfg(feature = "diagram-state")]
        AsciiDiagram::State(_) => true,
        _ => return Ok(false),
    };
    let raw = serde_json::from_slice::<&RawValue>(bytes).map_err(|_| invalid_projection())?;
    let mut root: Fields<'_> = decode(raw, lines)?;
    let mut identities = Identities::new();
    let mut groups = Vec::new();
    for (index, raw) in take_array(&mut root, "subgraphs", lines)?
        .into_iter()
        .enumerate()
    {
        let fields: Fields<'_> = decode(raw, lines)?;
        let id = text(&fields, "id", lines)?.ok_or_else(invalid_projection)?;
        let key = format!("G{}", index + 1);
        // The primary adapter keeps the first group declaration as the endpoint owner.
        identities.entry(id).or_insert_with(|| key.clone());
        groups.push(Entity { key, fields });
    }
    let mut nodes = Vec::new();
    let mut node_count = 0usize;
    let mut group_count = 0usize;
    for raw in take_array(&mut root, "nodes", lines)? {
        let fields: Fields<'_> = decode(raw, lines)?;
        let id = text(&fields, "id", lines)?.ok_or_else(invalid_projection)?;
        let key = if state {
            if fields.get("isGroup").is_some_and(|raw| raw.get() == "true") {
                group_count += 1;
                format!("G{group_count}")
            } else {
                node_count += 1;
                format!("N{node_count}")
            }
        } else if let Some(group) = identities.get(&id) {
            group.clone()
        } else {
            node_count += 1;
            format!("N{node_count}")
        };
        identities.insert(id, key.clone());
        nodes.push(Entity { key, fields });
    }

    if let Some(direction) = take_text(&mut root, "direction", lines)? {
        lines.push(format!("direction: {direction}"))?;
    }
    if let Some(title) = take_text(&mut root, "sourceTitle", lines)? {
        lines.push_quoted("title", &title)?;
    }
    lines.push("Keys: N nodes; G groups; E edges, source then target.".into())?;
    if state || !groups.is_empty() {
        lines.push("Local directions inherit unless marked explicit.".into())?;
    }

    let explicit_directions = take_array(&mut root, "subgraphExplicitDirections", lines)?;
    write_groups(groups, explicit_directions, &identities, lines)?;
    let nodes = prepare_nodes(nodes, state, &identities, lines)?;
    let mut pending = BTreeMap::new();
    if state {
        write_nodes(nodes, lines)?;
    } else {
        let (group_vertices, inline): (Vec<_>, Vec<_>) = nodes
            .into_iter()
            .partition(|node| node.key.starts_with('G'));
        write_nodes(group_vertices, lines)?;
        for node in inline {
            if let Some(previous) = pending.insert(node.key.clone(), node) {
                // Detached models may repeat an ID. Retain every declaration and its details.
                write_nodes(vec![previous], lines)?;
            }
        }
    }
    write_relationships(&mut root, state, &identities, &mut pending, lines)?;
    write_nodes(pending.into_values().collect(), lines)?;
    if state {
        state_details(&mut root, &identities, lines)?;
    } else {
        flow_details(&mut root, &identities, lines)?;
    }
    root.remove("type");
    root.remove("keyword");
    omit_empty(
        &mut root,
        &[
            "accTitle",
            "accDescr",
            "sourceConfig",
            "classDefs",
            "styleClasses",
        ],
    );
    details("graph", &root, lines)?;
    Ok(true)
}

fn write_groups(
    groups: Vec<Entity<'_>>,
    explicit_directions: Vec<&RawValue>,
    identities: &Identities,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    if !groups.is_empty() {
        lines.push("Groups:".into())?;
    }
    for (index, mut group) in groups.into_iter().enumerate() {
        let id = take_text(&mut group.fields, "id", lines)?.ok_or_else(invalid_projection)?;
        let canonical = identity(identities, &id)?;
        let title = take_text(&mut group.fields, "title", lines)?.unwrap_or_default();
        let direction = take_text(&mut group.fields, "dir", lines)?;
        let explicit = explicit_directions
            .get(index)
            .is_some_and(|raw| raw.get() == "true");
        let mut kind = if canonical == group.key {
            "group".to_owned()
        } else {
            format!("group declaration of {canonical}")
        };
        append_direction(&mut kind, direction.as_deref(), explicit);
        lines.push_quoted(&format!("{} [{kind}]", group.key), &title)?;
        let members = take_array(&mut group.fields, "nodes", lines)?;
        let mut member_keys = Vec::new();
        for member in members {
            let id: String = decode(member, lines)?;
            member_keys.push(identity(identities, &id)?.to_owned());
        }
        lines.push(format!("members {canonical}: {}", member_keys.join(", ")))?;
        omit_text_default(&mut group.fields, "labelType", &["text"], lines)?;
        omit_empty(&mut group.fields, &["classes", "styles"]);
        details(&group.key, &group.fields, lines)?;
    }
    Ok(())
}

fn prepare_nodes<'a>(
    nodes: Vec<Entity<'a>>,
    state: bool,
    identities: &Identities,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<Vec<NodeRecord<'a>>> {
    let mut records = Vec::new();
    for mut node in nodes {
        if take_text(&mut node.fields, "provenance", lines)?.as_deref() == Some("subgraph_anchor") {
            continue;
        }
        let id = take_text(&mut node.fields, "id", lines)?.ok_or_else(invalid_projection)?;
        // layoutShape is the terminal adapter's authoritative type. Keep a non-equivalent
        // source shape with its owner; core normalizes square/round to the aliases below.
        let source_shape = text(&node.fields, "shape", lines)?;
        let shape = take_text(&mut node.fields, "layoutShape", lines)?
            .or_else(|| source_shape.clone())
            .unwrap_or_else(|| if state { "state" } else { "node" }.into());
        if source_shape.as_deref().is_none_or(|source_shape| {
            source_shape == shape
                || matches!(
                    (source_shape, shape.as_str()),
                    ("square", "squareRect") | ("round", "roundedRect")
                )
        }) {
            node.fields.remove("shape");
        }
        let mut kind = if state {
            match shape.as_str() {
                "stateStart" => "start",
                "stateEnd" => "end",
                "divider" => "concurrent region",
                "rect" => "state",
                "rectWithTitle" => "state with description",
                "roundedWithTitle" => "composite",
                "noteGroup" => "note group",
                _ => &shape,
            }
            .to_owned()
        } else {
            shape.clone()
        };
        if let Some(node_type) = take_text(&mut node.fields, "type", lines)?
            && !matches!(node_type.as_str(), "default" | "group")
            && node_type != kind
        {
            kind.push_str(&format!("; type={node_type}"));
        }
        let direction = take_text(&mut node.fields, "dir", lines)?;
        let explicit = node
            .fields
            .remove("explicitDir")
            .is_some_and(|raw| raw.get() == "true");
        append_direction(&mut kind, direction.as_deref(), explicit);
        let group = node
            .fields
            .remove("isGroup")
            .is_some_and(|raw| raw.get() == "true");
        if group {
            kind.insert_str(0, "group; ");
        }
        let prefix = if !state && node.key.starts_with('G') {
            format!("{} vertex [{kind}]: ", node.key)
        } else {
            format!("{} [{kind}]: ", node.key)
        };
        let label = node.fields.remove("label");
        let label = match label.filter(|raw| raw.get() != "null") {
            Some(raw) if raw.get().starts_with('"') => {
                let mut label: String = decode(raw, lines)?;
                if state && label == id {
                    match shape.as_str() {
                        "stateStart" => label = "start".into(),
                        "stateEnd" => label = "end".into(),
                        "divider" => label = "concurrent region".into(),
                        _ => {}
                    }
                }
                NodeLabel::Text(label)
            }
            Some(raw) => {
                // State labels may be authored arrays/objects; retain their exact structure.
                NodeLabel::Structured(raw)
            }
            None => {
                let label = if state && shape == "stateStart" {
                    "start"
                } else if state && shape == "stateEnd" {
                    "end"
                } else if state && shape == "divider" {
                    "concurrent region"
                } else {
                    &id
                };
                NodeLabel::Text(label.to_owned())
            }
        };
        let parent = take_text(&mut node.fields, "parentId", lines)?
            .map(|parent| identity(identities, &parent).map(str::to_owned))
            .transpose()?;
        omit_text_default(&mut node.fields, "labelType", &["text"], lines)?;
        if node
            .fields
            .get("haveCallback")
            .is_some_and(|raw| raw.get() == "false")
        {
            node.fields.remove("haveCallback");
        }
        if state {
            omit_text_default(
                &mut node.fields,
                "cssClasses",
                &[
                    "statediagram-state",
                    "statediagram-note",
                    "statediagram-state statediagram-cluster",
                    "statediagram-state statediagram-cluster statediagram-cluster-alt",
                ],
                lines,
            )?;
        }
        // Only known struct defaults are omitted. Opaque authored metadata and unknown fields
        // retain even empty objects, false, zero, and null.
        omit_empty(
            &mut node.fields,
            &[
                "description",
                "icon",
                "form",
                "pos",
                "img",
                "constraint",
                "assetWidth",
                "assetHeight",
                "classes",
                "styles",
                "link",
                "linkTarget",
                "haveCallback",
                "cssClasses",
                "cssCompiledStyles",
                "cssStyles",
                "position",
                "labelStyle",
            ],
        );
        for field in [
            "domId",
            "padding",
            "rx",
            "ry",
            "colorIndex",
            "wrappingWidth",
            "minWidth",
        ] {
            node.fields.remove(field);
        }
        records.push(NodeRecord {
            key: node.key,
            prefix,
            label,
            fields: node.fields,
            parent,
        });
    }
    Ok(records)
}

fn write_nodes(nodes: Vec<NodeRecord<'_>>, lines: &mut SemanticFallbackWriter<'_>) -> Result<()> {
    if nodes.is_empty() {
        return Ok(());
    }
    lines.push("Nodes:".into())?;
    let mut membership: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in nodes {
        write_record(&node.parts(), lines)?;
        details(&node.key, &node.fields, lines)?;
        if let Some(parent) = node.parent {
            membership.entry(parent).or_default().push(node.key);
        }
    }
    for (group, members) in membership {
        lines.push(format!("members {group}: {}", members.join(", ")))?;
    }
    Ok(())
}

fn write_relationships<'a>(
    root: &mut Fields<'a>,
    state: bool,
    identities: &Identities,
    pending: &mut BTreeMap<String, NodeRecord<'a>>,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    if state {
        lines.push("Relationships:".into())?;
    }
    let mut explained_markers = BTreeSet::new();
    let mut explained_strokes = BTreeSet::new();
    for (index, raw) in take_array(root, "edges", lines)?.into_iter().enumerate() {
        let mut edge: Fields<'_> = decode(raw, lines)?;
        let key = format!("E{}", index + 1);
        let from = take_text(&mut edge, if state { "start" } else { "from" }, lines)?
            .ok_or_else(invalid_projection)?;
        let to = take_text(&mut edge, if state { "end" } else { "to" }, lines)?
            .ok_or_else(invalid_projection)?;
        let mut connector = if state {
            if text(&edge, "classes", lines)?
                .is_some_and(|value| value.split_whitespace().any(|class| class == "note-edge"))
            {
                "--".to_owned()
            } else {
                "-->".to_owned()
            }
        } else {
            let start =
                take_text(&mut edge, "startMarker", lines)?.unwrap_or_else(|| "none".into());
            let end = take_text(&mut edge, "endMarker", lines)?.unwrap_or_else(|| "none".into());
            let stroke =
                take_text(&mut edge, "strokeKind", lines)?.unwrap_or_else(|| "normal".into());
            let mut marker_key = Vec::new();
            for (kind, description) in [("circle", "circle o"), ("cross", "cross x")] {
                if (start == kind || end == kind) && explained_markers.insert(kind) {
                    marker_key.push(description);
                }
            }
            if start == "none" && end == "none" && explained_markers.insert("none") {
                marker_key.push("none empty");
            }
            if !marker_key.is_empty() {
                lines.push(format!(
                    "Markers (startMarker/endMarker): {}.",
                    marker_key.join("; ")
                ))?;
            }
            if stroke != "normal" && explained_strokes.insert(stroke.clone()) {
                let description = match stroke.as_str() {
                    "dotted" => "dotted ..",
                    "thick" => "thick ==",
                    _ => return Err(invalid_projection()),
                };
                lines.push(format!("Stroke: {description}."))?;
            }
            let line = match stroke.as_str() {
                "normal" => "--",
                "dotted" => "..",
                "thick" => "==",
                _ => return Err(invalid_projection()),
            };
            format!("{}{line}{}", marker(&start, true)?, marker(&end, false)?)
        };
        if take_text(&mut edge, "visibility", lines)?.as_deref() == Some("invisible") {
            connector.push_str(" (invisible constraint)");
        }
        let from = identity(identities, &from)?;
        let to = identity(identities, &to)?;
        // Removing the source first also declares a self-loop exactly once.
        let source_node = pending.remove(from);
        let target_node = pending.remove(to);
        let prefix = format!("{key}: ");
        let connector = format!(" {connector} ");
        let mut parts = vec![RecordPart::Plain(&prefix)];
        if let Some(node) = &source_node {
            parts.extend(node.parts());
        } else {
            parts.push(RecordPart::Plain(from));
        }
        parts.push(RecordPart::Plain(&connector));
        if let Some(node) = &target_node {
            parts.extend(node.parts());
        } else {
            parts.push(RecordPart::Plain(to));
        }
        let label = take_text(&mut edge, "label", lines)?.filter(|label| !label.is_empty());
        if let Some(label) = &label {
            parts.push(RecordPart::Plain(": "));
            parts.push(RecordPart::Quoted(label));
        }
        write_record(&parts, lines)?;
        for node in [&source_node, &target_node].into_iter().flatten() {
            details(&node.key, &node.fields, lines)?;
            if let Some(parent) = &node.parent {
                lines.push(format!("members {parent}: {}", node.key))?;
            }
        }
        if !edge
            .remove("isUserDefinedId")
            .is_some_and(|raw| raw.get() == "true")
        {
            edge.remove("id");
        }
        // Typed endpoint markers and strokes supersede Mermaid's redundant aggregate aliases.
        for field in ["type", "arrow", "stroke", "arrowTypeEnd"] {
            edge.remove(field);
        }
        if edge.get("length").is_some_and(|raw| raw.get() == "1") {
            edge.remove("length");
        }
        omit_text_default(&mut edge, "labelType", &["text"], lines)?;
        omit_empty(
            &mut edge,
            &[
                "interpolate",
                "classes",
                "style",
                "animate",
                "animation",
                "step",
            ],
        );
        if state {
            omit_text_default(&mut edge, "classes", &["transition"], lines)?;
        }
        details(&key, &edge, lines)?;
    }
    Ok(())
}

// Compose multiple quoted fields directly through the existing bounded writer. Escapes stay
// atomic at wraps, and labels never need an additional escaped allocation or JSON serialization.
fn write_record(parts: &[RecordPart<'_>], lines: &mut SemanticFallbackWriter<'_>) -> Result<()> {
    let (mut width, mut has_content) = lines.begin_line()?;
    for part in parts {
        let mut resources = lines.emit_resources.clone();
        match part {
            RecordPart::Plain(text) => crate::safe_text::visit_safe_line_fragments(
                &mut resources,
                text,
                lines.profile,
                |fragment, fragment_width, fragment_grapheme_bytes| {
                    lines.push_wrapped_fragment(
                        fragment,
                        fragment_width,
                        fragment_grapheme_bytes,
                        &mut width,
                        &mut has_content,
                    )
                },
            )?,
            RecordPart::Quoted(text) => {
                crate::safe_text::visit_quoted_terminal_text(text, &resources, |fragment| {
                    let mut grapheme_count = 0usize;
                    let mut grapheme_bytes = 0usize;
                    for grapheme in fragment.graphemes(true) {
                        grapheme_count = grapheme_count.saturating_add(1);
                        grapheme_bytes = grapheme_bytes.max(grapheme.len());
                    }
                    resources.charge_layout_work(grapheme_count)?;
                    lines.push_wrapped_fragment(
                        fragment,
                        display_width_with_profile(fragment, lines.profile),
                        grapheme_bytes,
                        &mut width,
                        &mut has_content,
                    )
                })?;
            }
        }
    }
    lines.finish_line()
}

fn append_direction(kind: &mut String, direction: Option<&str>, explicit: bool) {
    if let Some(direction) = direction {
        if explicit {
            kind.push_str(&format!("; direction={direction} (explicit)"));
        } else {
            kind.push_str("; direction=inherited");
        }
    }
}

fn marker(kind: &str, start: bool) -> Result<&'static str> {
    Ok(match kind {
        "none" => "",
        "point" if start => "<",
        "point" => ">",
        "circle" => "o",
        "cross" => "x",
        _ => return Err(invalid_projection()),
    })
}

fn flow_details(
    root: &mut Fields<'_>,
    identities: &Identities,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    root.remove("vertexCalls");
    root.remove("warningFacts");
    if let Some(defaults) = root
        .remove("edgeDefaults")
        .filter(|raw| raw.get() != "null")
    {
        let mut defaults: Fields<'_> = decode(defaults, lines)?;
        omit_empty(&mut defaults, &["interpolate", "style"]);
        details("edge defaults", &defaults, lines)?;
    }
    if let Some(raw) = root.remove("tooltips") {
        let tooltips: Fields<'_> = decode(raw, lines)?;
        for (id, tooltip) in tooltips {
            let tooltip: String = decode(tooltip, lines)?;
            lines.push_quoted(
                &format!("{} tooltip", authored_identity(identities, &id)),
                &tooltip,
            )?;
        }
    }
    if let Some(raw) = root
        .remove("subgraphVertexStyles")
        .filter(|raw| raw.get() != "null")
    {
        let styles: Fields<'_> = decode(raw, lines)?;
        for (_, raw) in styles {
            let mut fields: Fields<'_> = decode(raw, lines)?;
            let ordinal = fields
                .remove("declarationOrdinal")
                .ok_or_else(invalid_projection)?;
            let ordinal: usize = decode(ordinal, lines)?;
            details(&format!("G{} vertex", ordinal + 1), &fields, lines)?;
        }
    }
    for raw in take_array(root, "droppedSubGraphMembers", lines)? {
        let fields: Fields<'_> = decode(raw, lines)?;
        let member = text(&fields, "node", lines)?.ok_or_else(invalid_projection)?;
        let kept = text(&fields, "keptBy", lines)?.ok_or_else(invalid_projection)?;
        let dropped = text(&fields, "droppedFrom", lines)?.ok_or_else(invalid_projection)?;
        lines.push(format!(
            "membership: {} kept in {}; excluded from {}",
            authored_identity(identities, &member),
            authored_identity(identities, &kept),
            authored_identity(identities, &dropped)
        ))?;
    }
    Ok(())
}

fn state_details(
    root: &mut Fields<'_>,
    identities: &Identities,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    if let Some(raw) = root.remove("states") {
        let states: Fields<'_> = decode(raw, lines)?;
        for (id, raw) in states {
            let mut fields: Fields<'_> = decode(raw, lines)?;
            if text(&fields, "id", lines)?.as_deref() == Some(id.as_str()) {
                fields.remove("id");
            }
            if let Some(doc) = fields.remove("doc").filter(|raw| raw.get() != "null") {
                let doc: usize = decode(doc, lines)?;
                lines.push(format!(
                    "{} authored-document: D{}",
                    authored_identity(identities, &id),
                    doc + 1
                ))?;
            }
            omit_text_default(&mut fields, "type", &["default"], lines)?;
            omit_empty(
                &mut fields,
                &[
                    "descriptions",
                    "note",
                    "classes",
                    "styles",
                    "textStyles",
                    "start",
                ],
            );
            details(
                &format!("authored state {}", authored_identity(identities, &id)),
                &fields,
                lines,
            )?;
        }
    }
    if let Some(raw) = root.remove("links") {
        let links: Fields<'_> = decode(raw, lines)?;
        for (id, link) in links {
            lines.push(format!(
                "{} links: {}",
                authored_identity(identities, &id),
                link.get()
            ))?;
        }
    }
    // The flat statement store can retain authored facts absent from the terminal graph (for
    // example a class assignment, click, repeated declaration, or note). Keep these in scoped
    // records, pruning only known defaults and source-position bookkeeping.
    if let Some(raw) = root.remove("document") {
        let mut document: Fields<'_> = decode(raw, lines)?;
        if let Some(root) = document.remove("root").filter(|raw| raw.get() != "null") {
            let root: usize = decode(root, lines)?;
            lines.push(format!("authored root: D{}", root + 1))?;
        }
        for (index, raw) in take_array(&mut document, "documents", lines)?
            .into_iter()
            .enumerate()
        {
            let statements: Vec<&RawValue> = decode(raw, lines)?;
            for (ordinal, statement) in statements.into_iter().enumerate() {
                let scope = format!("authored D{}.{}", index + 1, ordinal + 1);
                if statement.get() == "null" {
                    continue;
                }
                if !statement.get().starts_with('{') {
                    lines.push(format!("{scope}: {}", statement.get()))?;
                    continue;
                }
                let mut fields: Fields<'_> = decode(statement, lines)?;
                let kind = take_text(&mut fields, "stmt", lines)?.unwrap_or_default();
                let id = take_text(&mut fields, "id", lines)?;
                let owner = id
                    .as_deref()
                    .map(|id| {
                        if kind == "classDef" {
                            id.to_owned()
                        } else {
                            id.split(',')
                                .map(|id| authored_identity(identities, id.trim()))
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    })
                    .unwrap_or_default();
                let scope = format!("{scope} {kind} {owner}");
                fields.remove("idSpan");
                if let Some(doc) = fields.remove("doc").filter(|raw| raw.get() != "null") {
                    let doc: usize = decode(doc, lines)?;
                    lines.push(format!("{scope} document: D{}", doc + 1))?;
                }
                let mut endpoints = Vec::new();
                for endpoint in ["state1", "state2"] {
                    if let Some(raw) = fields.remove(endpoint) {
                        let mut state: Fields<'_> = decode(raw, lines)?;
                        let id =
                            take_text(&mut state, "id", lines)?.ok_or_else(invalid_projection)?;
                        prune_statement_defaults(&mut state, lines)?;
                        let endpoint_scope =
                            format!("{scope} {endpoint} {}", authored_identity(identities, &id));
                        if let Some(doc) = state.remove("doc") {
                            let doc: usize = decode(doc, lines)?;
                            lines.push(format!("{endpoint_scope} document: D{}", doc + 1))?;
                        }
                        endpoints.push(authored_identity(identities, &id).to_owned());
                        details(&endpoint_scope, &state, lines)?;
                    }
                }
                if endpoints.len() == 2 {
                    lines.push(format!("{scope}{} --> {}", endpoints[0], endpoints[1]))?;
                }
                prune_statement_defaults(&mut fields, lines)?;
                details(scope.trim_end(), &fields, lines)?;
            }
        }
        details("authored document", &document, lines)?;
    }
    // Relations duplicate the transition table but may be independently authored on detached
    // models. Retain each one with the same reader endpoint keys, including duplicates.
    for (index, raw) in take_array(root, "relations", lines)?
        .into_iter()
        .enumerate()
    {
        let mut relation: Fields<'_> = decode(raw, lines)?;
        let from = take_text(&mut relation, "id1", lines)?.ok_or_else(invalid_projection)?;
        let to = take_text(&mut relation, "id2", lines)?.ok_or_else(invalid_projection)?;
        let scope = format!(
            "authored relation {}: {} --> {}",
            index + 1,
            authored_identity(identities, &from),
            authored_identity(identities, &to)
        );
        // Even an unlabeled relation on a detached model must keep its endpoints.
        if relation.is_empty() {
            lines.push(scope)?;
        } else {
            details(&scope, &relation, lines)?;
        }
    }
    Ok(())
}

fn prune_statement_defaults(
    fields: &mut Fields<'_>,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<()> {
    fields.remove("idSpan");
    omit_text_default(fields, "type", &["default"], lines)?;
    omit_empty(
        fields,
        &[
            "description",
            "descriptions",
            "doc",
            "note",
            "classes",
            "styles",
            "textStyles",
            "start",
        ],
    );
    Ok(())
}

fn identity<'a>(identities: &'a Identities, id: &str) -> Result<&'a str> {
    identities
        .get(id)
        .map(String::as_str)
        .ok_or_else(invalid_projection)
}

fn authored_identity<'a>(identities: &'a Identities, id: &'a str) -> &'a str {
    identities.get(id).map_or(id, String::as_str)
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
    key: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Vec<&'a RawValue>> {
    fields
        .remove(key)
        .map_or_else(|| Ok(Vec::new()), |raw| decode(raw, lines))
}

fn text(
    fields: &Fields<'_>,
    key: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Option<String>> {
    fields
        .get(key)
        .filter(|raw| raw.get() != "null")
        .map(|raw| decode(raw, lines))
        .transpose()
}

fn take_text(
    fields: &mut Fields<'_>,
    key: &str,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Option<String>> {
    fields
        .remove(key)
        .filter(|raw| raw.get() != "null")
        .map(|raw| decode(raw, lines))
        .transpose()
}

fn omit_text_default(
    fields: &mut Fields<'_>,
    key: &str,
    defaults: &[&str],
    lines: &SemanticFallbackWriter<'_>,
) -> Result<()> {
    if text(fields, key, lines)?.is_none_or(|value| defaults.contains(&value.as_str())) {
        fields.remove(key);
    }
    Ok(())
}

fn omit_empty(fields: &mut Fields<'_>, keys: &[&str]) {
    for key in keys {
        if fields
            .get(*key)
            .is_some_and(|raw| matches!(raw.get(), "null" | "[]" | "{}" | "\"\""))
        {
            fields.remove(*key);
        }
    }
}

fn details(scope: &str, fields: &Fields<'_>, lines: &mut SemanticFallbackWriter<'_>) -> Result<()> {
    if fields.is_empty() {
        return Ok(());
    }
    let overflow = || {
        lines
            .semantic_resources
            .overflow(AsciiResourceLimitId::MaxOutputBytes)
    };
    let mut length = scope.len().checked_add(12).ok_or_else(overflow)?;
    for (index, (key, raw)) in fields.iter().enumerate() {
        // Model field names are known ASCII identifiers. The opaque values remain borrowed
        // complete JSON, including empty objects inside authored metadata/configuration.
        length = length
            .checked_add(key.len())
            .and_then(|length| length.checked_add(raw.get().len()))
            .and_then(|length| length.checked_add(if index == 0 { 1 } else { 2 }))
            .ok_or_else(overflow)?;
    }
    lines
        .semantic_resources
        .check(AsciiResourceLimitId::MaxOutputBytes, length)?;
    let mut record = String::new();
    record
        .try_reserve(length)
        .map_err(|_| AsciiError::allocation_failed("ascii_graph_details"))?;
    record.push_str(scope);
    record.push_str(" details: {");
    for (index, (key, raw)) in fields.iter().enumerate() {
        if index > 0 {
            record.push(',');
        }
        record.push_str(key);
        record.push(':');
        record.push_str(raw.get());
    }
    record.push('}');
    lines.push(record)
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "ascii",
        feature: "invalid serialized graph projection",
    }
}
