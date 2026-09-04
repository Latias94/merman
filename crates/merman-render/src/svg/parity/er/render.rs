use super::super::css::er_css_with_resolved_typography;
use super::super::*;

// ER diagram SVG renderer implementation (split from parity.rs).

fn is_er_redux_color_theme(effective_config: &serde_json::Value) -> bool {
    matches!(
        SvgTheme::new(effective_config).theme_name().as_str(),
        "redux-color" | "redux-dark-color"
    )
}

fn er_redux_color_id(border_colors: &[String], color_index: usize) -> Option<String> {
    (!border_colors.is_empty()).then(|| format!("color-{}", color_index % border_colors.len()))
}

fn er_color_indices(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
) -> std::collections::HashMap<String, usize> {
    fn insertion_index(id: &str) -> Option<usize> {
        id.rsplit_once('-')?.1.parse().ok()
    }

    let mut entities: Vec<_> = model.entities.values().collect();
    entities.sort_by(|left, right| {
        (insertion_index(&left.id), left.id.as_str())
            .cmp(&(insertion_index(&right.id), right.id.as_str()))
    });
    entities
        .into_iter()
        .enumerate()
        .map(|(index, entity)| (entity.id.clone(), index))
        .collect()
}

fn er_theme_color_limit(effective_config: &serde_json::Value) -> usize {
    config_f64(effective_config, &["themeVariables", "THEME_COLOR_LIMIT"])
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 64.0).ceil() as usize)
        .unwrap_or(12)
}

fn write_er_redux_color_css(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    data_look: &str,
    border_colors: &[String],
    background_colors: &[String],
    theme_color_limit: usize,
) -> Result<()> {
    let diagram_id = crate::svg::escape_css_identifier(diagram_id);
    for index in 0..theme_color_limit {
        let Some(border_color) = border_colors.get(index) else {
            continue;
        };
        let fill = background_colors
            .get(index)
            .map(|color| format!("fill:{color};"))
            .unwrap_or_default();
        let _ = write!(
            out,
            r#"#{} [data-look="{}"][data-color-id="color-{}"].node path{{stroke:{};{}}}#{} [data-look="{}"][data-color-id="color-{}"].node rect{{stroke:{};{}}}"#,
            diagram_id.as_str(),
            escape_xml(data_look),
            index,
            border_color,
            fill,
            diagram_id.as_str(),
            escape_xml(data_look),
            index,
            border_color,
            fill,
        );
        out.checkpoint()?;
    }
    Ok(())
}

fn er_redux_color_css_insertion_point(css: &str, diagram_id: &str) -> usize {
    let escaped_id = crate::svg::escape_css_identifier(diagram_id);
    let family_rule = format!("#{escaped_id} .entityBox");
    let root_rule = format!("#{escaped_id} :root");
    css.find(&family_rule)
        .or_else(|| css.find(&root_rule))
        .unwrap_or(css.len())
}

fn write_er_style_with_font_family(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    data_look: &str,
    effective_config: &serde_json::Value,
    border_colors: &[String],
    background_colors: &[String],
    theme_color_limit: usize,
    resolved_font_family: Option<&str>,
    resolved_font_size: Option<&str>,
) -> Result<(String, String)> {
    let emission = er_css_with_resolved_typography(
        diagram_id,
        effective_config,
        resolved_font_family,
        resolved_font_size,
    )?;
    let css = emission.css;
    let insertion_point = er_redux_color_css_insertion_point(&css, diagram_id);

    out.push_str("<style>");
    out.push_str(&css[..insertion_point]);
    out.checkpoint()?;
    write_er_redux_color_css(
        out,
        diagram_id,
        data_look,
        border_colors,
        background_colors,
        theme_color_limit,
    )?;
    out.push_str(&css[insertion_point..]);
    out.push_str("</style>");
    out.checkpoint()?;
    Ok((emission.font_family.into(), emission.font_size.into()))
}

type ErStyleDeclaration = crate::diagram_theme::PreparedSourceStyleDeclaration;

fn write_er_style_declaration(
    out: &mut String,
    declaration: &ErStyleDeclaration,
    force_important: bool,
) {
    let _ = write!(
        out,
        "{}:{}",
        declaration.property_css(),
        declaration.value()
    );
    if force_important || declaration.important() {
        out.push_str(" !important");
    }
}

fn style_decls_with_important_join(decls: &[ErStyleDeclaration], join: &str) -> String {
    let mut out = String::new();
    for (index, declaration) in decls.iter().enumerate() {
        if index != 0 {
            out.push_str(join);
        }
        write_er_style_declaration(&mut out, declaration, true);
    }
    out
}

fn style_decls_with_important(decls: &[ErStyleDeclaration]) -> String {
    style_decls_with_important_join(decls, "; ")
}

// The ER parser also accepts source declarations represented as raw strings for
// subgraph compatibility styling. Keep that formatting path separate from the
// prepared declaration path used by the typed theme receipts.
fn insert_er_redux_color_css<I>(css: &mut String, diagram_id: I, color_css: &str)
where
    I: SvgDiagramIdValue,
{
    if color_css.is_empty() {
        return;
    }

    let escaped_id = diagram_id.semantic_value();
    let family_rule = format!("#{escaped_id} .entityBox");
    let root_rule = format!("#{escaped_id} :root");
    let insertion_point = css
        .find(&family_rule)
        .or_else(|| css.find(&root_rule))
        .unwrap_or(css.len());
    css.insert_str(insertion_point, color_css);
}

fn compile_er_entity_styles(
    entity: &crate::er::ErEntity,
    classes: &indexmap::IndexMap<String, crate::er::ErClassDef>,
) -> (Vec<String>, Vec<String>) {
    let mut compiled_box: Vec<String> = Vec::new();
    let mut compiled_text: Vec<String> = Vec::new();
    let mut seen_classes: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for class_name in entity.css_classes.split_whitespace() {
        if !seen_classes.insert(class_name) {
            continue;
        }
        let Some(def) = classes.get(class_name) else {
            continue;
        };
        for s in &def.styles {
            let t = s.trim();
            if t.is_empty() {
                continue;
            }
            compiled_box.push(t.to_string());
        }
        for s in &def.text_styles {
            let t = s.trim();
            if t.is_empty() {
                continue;
            }
            compiled_text.push(t.to_string());
        }
    }

    let mut rect_map: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    let mut text_map: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();

    // Box styles: classDef styles + `style` statements.
    for s in compiled_box.iter().chain(entity.css_styles.iter()) {
        let Some((k, v)) = parse_style_decl(s) else {
            continue;
        };
        if is_rect_style_key(k) {
            rect_map.insert(k.to_string(), v.to_string());
        }
        // Mermaid treats `color:` as the HTML label text color (even if it comes from the style list).
        if k == "color" {
            text_map.insert("color".to_string(), v.to_string());
        }
    }

    // Text styles: classDef textStyles + `style` statements (only text-related keys).
    for s in compiled_text.iter().chain(entity.css_styles.iter()) {
        let Some((k, v)) = parse_style_decl(s) else {
            continue;
        };
        if !is_text_style_key(k) {
            continue;
        }
        if k == "color" {
            text_map.insert("color".to_string(), v.to_string());
        } else {
            text_map.insert(k.to_string(), v.to_string());
        }
    }

    let mut rect_decls: Vec<String> = Vec::new();
    for k in [
        "fill",
        "stroke",
        "stroke-width",
        "stroke-dasharray",
        "opacity",
        "fill-opacity",
        "stroke-opacity",
    ] {
        if let Some(v) = rect_map.get(k) {
            rect_decls.push(format!("{k}:{v}"));
        }
    }

    let mut text_decls: Vec<String> = Vec::new();
    for k in [
        "color",
        "font-family",
        "font-size",
        "font-weight",
        "opacity",
    ] {
        if let Some(v) = text_map.get(k) {
            text_decls.push(format!("{k}:{v}"));
        }
    }

    (rect_decls, text_decls)
}

fn compile_er_subgraph_styles(
    subgraph: &crate::er::ErSubgraph,
    classes: &indexmap::IndexMap<String, crate::er::ErClassDef>,
) -> (Vec<String>, Vec<String>) {
    let mut compiled_box = Vec::new();
    let mut compiled_text = Vec::new();
    let mut seen_classes = std::collections::HashSet::new();
    for class_name in &subgraph.classes {
        if !seen_classes.insert(class_name.as_str()) {
            continue;
        }
        let Some(def) = classes.get(class_name) else {
            continue;
        };
        compiled_box.extend(def.styles.iter().map(|style| style.trim().to_string()));
        compiled_text.extend(def.text_styles.iter().map(|style| style.trim().to_string()));
    }

    let mut rect_map = std::collections::BTreeMap::new();
    let mut text_map = std::collections::BTreeMap::new();
    for style in compiled_box.iter().chain(subgraph.css_styles.iter()) {
        let Some((key, value)) = parse_style_decl(style) else {
            continue;
        };
        if is_rect_style_key(key) {
            rect_map.insert(key.to_string(), value.to_string());
        }
        if key == "color" {
            text_map.insert("color".to_string(), value.to_string());
        }
    }
    for style in compiled_text.iter().chain(subgraph.css_styles.iter()) {
        let Some((key, value)) = parse_style_decl(style) else {
            continue;
        };
        if is_text_style_key(key) {
            text_map.insert(key.to_string(), value.to_string());
        }
    }

    let rect_decls = [
        "fill",
        "stroke",
        "stroke-width",
        "stroke-dasharray",
        "opacity",
        "fill-opacity",
        "stroke-opacity",
    ]
    .into_iter()
    .filter_map(|key| rect_map.get(key).map(|value| format!("{key}:{value}")))
    .collect();
    let text_decls = [
        "color",
        "font-family",
        "font-size",
        "font-weight",
        "opacity",
    ]
    .into_iter()
    .filter_map(|key| text_map.get(key).map(|value| format!("{key}:{value}")))
    .collect();
    (rect_decls, text_decls)
}

fn er_subgraph_label_fragment(subgraph: &crate::er::ErSubgraph) -> String {
    if subgraph.label_type == "string" || subgraph.label_type == "text" {
        escape_xml(&subgraph.title)
    } else {
        crate::text::mermaid_markdown_to_xhtml_label_fragment(&subgraph.title, true)
    }
}

fn er_svg_label_source(label: &crate::er::ErBoxLabel) -> &str {
    label.svg_text_source()
}

fn write_er_svg_box_label(
    out: &mut impl SvgOutput,
    label: &crate::er::ErBoxLabel,
    style: &crate::text::TextStyle,
    measurer: &dyn TextMeasurer,
    max_width_px: Option<f64>,
) {
    let source = er_svg_label_source(label);
    if let Some(max_width_px) = max_width_px {
        crate::svg::parity::flowchart::write_flowchart_svg_text_markdown_wrapped(
            out,
            source,
            true,
            measurer,
            style,
            Some(max_width_px),
        );
    } else {
        crate::svg::parity::flowchart::write_flowchart_svg_text_markdown(out, source, true);
    }
}

fn write_er_svg_plain_label(out: &mut impl SvgOutput, text: &str) {
    let lines = crate::flowchart::flowchart_non_markdown_svg_source_word_lines(text);
    crate::svg::parity::flowchart::write_flowchart_svg_source_word_lines(out, &lines, true);
}

fn write_er_svg_label_background(out: &mut impl SvgOutput) {
    out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
}

#[derive(Clone, Copy)]
struct ErSubgraphRenderContext<'a> {
    diagram_id: SvgDiagramId<'a>,
    data_look: &'a str,
    classes: &'a indexmap::IndexMap<String, crate::er::ErClassDef>,
    use_html_labels: bool,
    translate_x: f64,
    translate_y: f64,
}

fn render_er_subgraph_cluster(
    out: &mut impl SvgOutput,
    cluster: &crate::model::LayoutCluster,
    subgraph: &crate::er::ErSubgraph,
    context: ErSubgraphRenderContext<'_>,
) {
    let (rect_styles, text_styles) = compile_er_subgraph_styles(subgraph, context.classes);
    let rect_style_attr = if rect_styles.is_empty() {
        "style=\"\"".to_string()
    } else {
        format!(
            r#"style="{}""#,
            escape_xml(&string_style_decls_with_important(&rect_styles))
        )
    };
    let text_style_attr = if text_styles.is_empty() {
        "style=\"\"".to_string()
    } else {
        format!(
            r#"style="{}""#,
            escape_xml(&string_style_decls_with_important(&text_styles))
        )
    };
    let class_attr = if subgraph.classes.is_empty() {
        "cluster".to_string()
    } else {
        format!("cluster {}", subgraph.classes.join(" "))
    };
    let width = cluster.width.max(1.0);
    let height = cluster.height.max(1.0);
    let left = cluster.x - width / 2.0 + context.translate_x;
    let top = cluster.y - height / 2.0 + context.translate_y;
    let title_width = cluster.title_label.width.max(0.0);
    let title_height = cluster.title_label.height.max(0.0);
    let title_x = cluster.title_label.x + context.translate_x;
    let title_y = cluster.title_label.y + context.translate_y;
    let _ = write!(
        out,
        r#"<g id="{}-{}" class="{}" data-look="{}"><rect class="basic label-container" {} x="{}" y="{}" width="{}" height="{}"/><g class="cluster-label" transform="translate({}, {})">"#,
        escape_xml(&format!("{}", context.diagram_id)),
        escape_xml(&subgraph.id),
        escape_xml(&class_attr),
        escape_xml(context.data_look),
        rect_style_attr,
        fmt(left),
        fmt(top),
        fmt(width),
        fmt(height),
        fmt(title_x),
        fmt(title_y),
    );
    if context.use_html_labels {
        let title_fragment = er_subgraph_label_fragment(subgraph);
        let _ = write!(
            out,
            r#"<foreignObject x="{}" y="{}" width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; text-align: center;"><span class="nodeLabel" {}>{}</span></div></foreignObject>"#,
            fmt(-title_width / 2.0),
            fmt(-title_height / 2.0),
            fmt(title_width),
            fmt(title_height),
            text_style_attr,
            title_fragment,
        );
    } else {
        let _ = write!(out, "<g {}>", text_style_attr);
        write_er_svg_label_background(out);
        if subgraph.label_type == "string" || subgraph.label_type == "text" {
            write_er_svg_plain_label(out, &subgraph.title);
        } else {
            crate::svg::parity::flowchart::write_flowchart_svg_text_markdown(
                out,
                &subgraph.title,
                true,
            );
        }
        out.push_str("</g>");
    }
    out.push_str("</g></g>");
    out.push('\n');
}

fn string_style_decls_with_important_join(decls: &[String], join: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for d in decls {
        let Some((k, v)) = parse_style_decl(d) else {
            continue;
        };
        out.push(format!("{k}:{v} !important"));
    }
    out.join(join)
}

fn render_er_subgraph_clusters(
    out: &mut impl SvgOutput,
    clusters: &[crate::model::LayoutCluster],
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    context: ErSubgraphRenderContext<'_>,
) {
    for cluster in clusters {
        let Some(subgraph) = model.subgraphs.iter().find(|item| item.id == cluster.id) else {
            continue;
        };
        render_er_subgraph_cluster(out, cluster, subgraph, context);
    }
}

fn string_style_decls_with_important(decls: &[String]) -> String {
    string_style_decls_with_important_join(decls, "; ")
}

fn typed_text_terminal_style(typed_paint: Option<(usize, &str)>) -> String {
    typed_paint
        .map(|(_, css)| format!("color:{css};fill:{css}"))
        .unwrap_or_default()
}

fn er_text_style_attr(
    source_declarations: &[ErStyleDeclaration],
    typed_paint: Option<(usize, &str)>,
) -> String {
    let mut declarations = Vec::new();
    if !source_declarations.is_empty() {
        declarations.push(style_decls_with_important(source_declarations));
    }
    let typed_style = typed_text_terminal_style(typed_paint);
    if !typed_style.is_empty() {
        declarations.push(typed_style);
    }
    format!(r#"style="{}""#, escape_attr(&declarations.join("; ")))
}

#[derive(Debug, Clone, Copy)]
struct ErEntityPaintEmission<'a> {
    emitted_fill: Option<(usize, &'a str)>,
    emitted_stroke: Option<(usize, &'a str)>,
}

fn er_entity_paint_emission<'a>(
    typed_fill: Option<(usize, &'a str)>,
    typed_stroke: Option<(usize, &'a str)>,
) -> ErEntityPaintEmission<'a> {
    ErEntityPaintEmission {
        emitted_fill: typed_fill,
        emitted_stroke: typed_stroke,
    }
}

fn entity_rect_style_attr<'a>(
    source_decls: &[ErStyleDeclaration],
    typed_fill: Option<(usize, &'a str)>,
    typed_stroke: Option<(usize, &'a str)>,
) -> (String, ErEntityPaintEmission<'a>) {
    let mut declarations = Vec::new();
    if !source_decls.is_empty() {
        declarations.push(style_decls_with_important(source_decls));
    }
    if let Some((_, fill)) = typed_fill {
        declarations.push(format!("fill:{fill}"));
    }
    if let Some((_, stroke)) = typed_stroke {
        declarations.push(format!("stroke:{stroke}"));
    }
    (
        format!(r#"style="{}""#, escape_attr(&declarations.join("; "))),
        er_entity_paint_emission(typed_fill, typed_stroke),
    )
}

fn style_keys_join(
    decls: &[ErStyleDeclaration],
    keys: &[&str],
    join: &str,
    force_important: bool,
) -> String {
    let mut out = String::new();
    for declaration in decls
        .iter()
        .filter(|declaration| keys.contains(&declaration.property()))
    {
        if !out.is_empty() {
            out.push_str(join);
        }
        write_er_style_declaration(&mut out, declaration, force_important);
    }
    out
}

fn concat_style_keys(decls: &[ErStyleDeclaration], keys: &[&str]) -> String {
    style_keys_join(decls, keys, ";", false)
}

fn parse_px_f64(v: &str) -> Option<f64> {
    let raw = v.trim().trim_end_matches(';').trim();
    let raw = raw.trim_end_matches("px").trim();
    if raw.is_empty() {
        return None;
    }
    raw.parse::<f64>().ok()
}

fn er_rel_idx_from_edge_id(edge_id: &str) -> Option<usize> {
    let rest = edge_id.strip_prefix("er-rel-")?;
    let digits_len = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .map(char::len_utf8)
        .sum::<usize>();
    if digits_len == 0 {
        return None;
    }
    rest[..digits_len].parse::<usize>().ok()
}

fn er_edge_dom_id(edge_id: &str, relationships: &[crate::er::ErRelationship]) -> String {
    let Some(idx) = er_rel_idx_from_edge_id(edge_id) else {
        return edge_id.to_string();
    };
    let Some(rel) = relationships.get(idx) else {
        return edge_id.to_string();
    };
    // The SVG-facing layout projection merges Dagre's three internal self-loop segments into
    // one logical relationship edge. Mermaid's common painter consequently uses the ordinary
    // relationship id for that visible edge; the `cyclic-special-*` ids belong only to the
    // hidden helper labels that remain in the layout model.
    format!("id_{}_{}_{}", rel.entity_a, rel.entity_b, idx)
}

#[allow(clippy::too_many_arguments)]
fn write_er_marker_definition(
    out: &mut impl SvgOutput,
    marker_id: &str,
    marker_class: &str,
    ref_x: &str,
    ref_y: &str,
    marker_width: &str,
    marker_height: &str,
    content: &str,
    typed_stroke: Option<(usize, &str)>,
) -> Result<()> {
    let _ = write!(
        out,
        r#"<defs><marker id="{}" class="{}" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" orient="auto""#,
        escape_attr(marker_id),
        escape_attr(marker_class),
        ref_x,
        ref_y,
        marker_width,
        marker_height,
    );
    if let Some((_, css)) = typed_stroke {
        let _ = write!(out, r#" style="stroke:{} !important""#, escape_attr(css));
    }
    let _ = writeln!(out, ">{content}</marker></defs>");
    out.checkpoint()
}

pub(crate) fn render_er_diagram_svg_model(
    layout: &ErDiagramLayout,
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    entity_theme: &crate::er::ErEntityThemePlan,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let effective_config = effective_config.as_value();
    let diagram_id = options.diagram_id_or("merman");
    // Mermaid's internal diagram type for ER is `er` (not `erDiagram`), and marker ids are derived
    // from this type (e.g. `<diagramId>_er-zeroOrMoreEnd`).
    let diagram_type = "er";
    let er_render_settings = crate::er::ErConfigView::new(effective_config)
        .render_settings_with_resolved_typography(
            Some(entity_theme.font_family_css()),
            entity_theme.font_size_override(),
        );
    let is_elk_layout = er_render_settings.is_elk_layout;
    let data_look = er_render_settings.diagram_look.as_str();
    let redux_color_theme = is_er_redux_color_theme(effective_config);
    let svg_theme = SvgTheme::new(effective_config);
    let redux_border_colors = if redux_color_theme {
        svg_theme.string_array("borderColorArray")
    } else {
        Vec::new()
    };
    let redux_background_colors = if redux_color_theme {
        svg_theme.string_array("bkgColorArray")
    } else {
        Vec::new()
    };
    let theme_color_limit = er_theme_color_limit(effective_config);
    let color_indices = er_color_indices(model);

    // Mermaid's computed theme variables are not currently present in `effective_config`.
    // Use Mermaid default theme fallbacks so Stage-B SVGs match upstream defaults more closely.
    let _stroke = theme_token(effective_config, "lineColor", "#333333");
    let node_border = theme_token(effective_config, "nodeBorder", "#9370DB");
    let main_bkg = theme_token(effective_config, "mainBkg", "#ECECFF");
    let _tertiary = theme_token(
        effective_config,
        "tertiaryColor",
        "hsl(80, 100%, 96.2745098039%)",
    );
    let text_color = theme_token(effective_config, "textColor", "#333333");
    let _node_text_color = theme_token(effective_config, "nodeTextColor", &text_color);
    let font_family = er_render_settings.font_family.clone();
    let font_size = er_render_settings.font_size;
    let title_top_margin = er_render_settings.title_top_margin;
    let use_max_width = er_render_settings.use_max_width;
    let label_style = er_render_settings.label_style.clone();
    let attr_style = er_render_settings.attr_style.clone();
    let edge_html_labels = er_render_settings.relationship_html_labels;
    let entity_wrap_mode = er_render_settings.entity_html_label_wrap_mode;
    let entity_measurement = er_render_settings.entity_measurement;
    let hand_drawn_seed =
        options.rough_randomness(er_render_settings.hand_drawn_seed, "render.er.roughjs");
    let insert_title_top_margin = er_render_settings.insert_title_top_margin;
    fn parse_trailing_index(id: &str) -> Option<i64> {
        let (_, tail) = id.rsplit_once('-')?;
        tail.parse::<i64>().ok()
    }
    fn er_node_sort_key(id: &str) -> (i64, i64) {
        if id.contains("---") {
            return (1, parse_trailing_index(id).unwrap_or(i64::MAX));
        }
        (0, parse_trailing_index(id).unwrap_or(i64::MAX))
    }

    let mut nodes = layout.nodes.clone();
    nodes.sort_by_key(|n| er_node_sort_key(&n.id));

    // Layout keeps Dagre's helper segments for reproducible layout artifacts. The family
    // preparation path also carries Mermaid's SVG-facing projection, where self-loops are
    // already merged into one visible edge. Fall back to the canonical edges for hand-built
    // layouts used by unit tests and older callers.
    let mut edges = if layout.render_edges.is_empty() {
        layout.edges.clone()
    } else {
        layout.render_edges.clone()
    };
    fn er_edge_sort_key(edge: &crate::model::LayoutEdge) -> (i64, i64, i64) {
        // Mermaid's getEdgesToRender() appends merged self-loops after ordinary graph edges.
        let self_loop = i64::from(edge.from == edge.to);
        let id = edge.id.as_str();
        let Some(rest) = id.strip_prefix("er-rel-") else {
            return (self_loop, i64::MAX, i64::MAX);
        };
        let mut digits_len = 0usize;
        for ch in rest.chars() {
            if !ch.is_ascii_digit() {
                break;
            }
            digits_len += ch.len_utf8();
        }
        if digits_len == 0 {
            return (self_loop, i64::MAX, i64::MAX);
        }
        let Ok(idx) = rest[..digits_len].parse::<i64>() else {
            return (self_loop, i64::MAX, i64::MAX);
        };
        (self_loop, idx, 0)
    }
    edges.sort_by_key(er_edge_sort_key);

    let visible_relation_edges = edges
        .iter()
        .filter(|edge| options.debug.include_edges && edge.points.len() >= 2)
        .collect::<Vec<_>>();
    let relation_terminals = visible_relation_edges
        .iter()
        .map(|edge| {
            let relationship_index = er_rel_idx_from_edge_id(&edge.id);
            crate::er::ErRelationTerminalExpectation::new(
                edge.id.clone(),
                edge.start_marker
                    .as_deref()
                    .map(|marker| er_unified_marker_id(diagram_id, diagram_type, marker)),
                edge.end_marker
                    .as_deref()
                    .map(|marker| er_unified_marker_id(diagram_id, diagram_type, marker)),
                relationship_index.and_then(|index| entity_theme.typed_relation_stroke(index)),
            )
        })
        .collect::<Vec<_>>();
    let mut entity_theme_receipt =
        entity_theme.begin_terminal_receipt(relation_terminals, options.debug.include_edges);

    let include_md_parent = edges.iter().any(|e| {
        matches!(
            e.start_marker.as_deref(),
            Some("MD_PARENT_START") | Some("MD_PARENT_END")
        ) || matches!(
            e.end_marker.as_deref(),
            Some("MD_PARENT_START") | Some("MD_PARENT_END")
        )
    });

    let diagram_title = diagram_title.map(str::trim).filter(|t| !t.is_empty());
    let is_empty_diagram = nodes.is_empty() && edges.is_empty() && diagram_title.is_none();

    let bounds = compute_layout_bounds(&layout.clusters, &nodes, &edges).unwrap_or({
        if is_empty_diagram {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            }
        } else {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            }
        }
    });

    let mut content_bounds = bounds.clone();
    let diagram_title_x = diagram_title.map(|title| {
        let title_style = crate::text::TextStyle {
            font_family: Some(font_family.clone()),
            font_size,
            font_weight: None,
            font_style: None,
        };
        let (title_left, title_right) = measurer.measure_svg_title_bbox_x(title, &title_style);
        entity_theme_receipt.record_diagram_title_measurement_with_typography(
            title,
            title_style.font_family.as_deref(),
            title_style.font_size,
        );
        let (title_ascent, title_descent) =
            crate::text::svg_title_bbox_vertical_extents_px(&title_style);
        let w = (content_bounds.max_x - content_bounds.min_x).max(1.0);
        let title_x = content_bounds.min_x + w / 2.0;
        let title_y = -title_top_margin;
        let title_min_x = title_x - title_left;
        let title_max_x = title_x + title_right;
        let title_min_y = title_y - title_ascent;
        let title_max_y = title_y + title_descent;
        content_bounds.min_x = content_bounds.min_x.min(title_min_x);
        content_bounds.max_x = content_bounds.max_x.max(title_max_x);
        content_bounds.min_y = content_bounds.min_y.min(title_min_y);
        content_bounds.max_y = content_bounds.max_y.max(title_max_y);
        title_x
    });

    let pad = options.viewbox_padding.max(0.0);
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let (translate_x, translate_y, root_bounds, root_width_for_title) = if is_empty_diagram {
        let empty_span = (pad * 2.0).max(1.0);
        (
            0.0,
            0.0,
            root_svg::DiagramBounds::from_view_box(-pad, -pad, empty_span, empty_span),
            empty_span,
        )
    } else {
        let content_w = (content_bounds.max_x - content_bounds.min_x).max(1.0);
        let content_h = (content_bounds.max_y - content_bounds.min_y).max(1.0);
        let vb_w = content_w + pad * 2.0;
        let vb_h = content_h + pad * 2.0;
        (
            0.0,
            0.0,
            root_svg::DiagramBounds::from_view_box(
                content_bounds.min_x - pad,
                content_bounds.min_y - pad,
                vb_w,
                vb_h,
            ),
            vb_w,
        )
    };
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width).with_max_width(
        root_svg::RootMaxWidth::CssSixSignificant(root_width_for_title),
    );
    let root_viewport = root_svg::RootViewportContext::new(crate::DiagramFamilyId::ER, diagram_id);
    let root_plan = root_viewport.plan(root_spec)?;

    let has_acc_title = model.acc_title.as_ref().is_some_and(|s| !s.is_empty());
    let has_acc_descr = model.acc_descr.as_ref().is_some_and(|s| !s.is_empty());
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, diagram_type);
    root_chrome.class = Some("erDiagram");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    let root_document = root_viewport.write_plan(&mut out, &root_plan, root_chrome)?;
    options.checkpoint_emit()?;

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}"#,
            diagram_id,
            escape_xml(model.acc_title.as_deref().unwrap_or_default())
        );
        out.push_str("</title>");
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}"#,
            diagram_id,
            escape_xml(model.acc_descr.as_deref().unwrap_or_default())
        );
        out.push_str("</desc>");
    }
    out.checkpoint()?;

    let (emitted_font_family, emitted_font_size) = write_er_style_with_font_family(
        &mut out,
        diagram_id.semantic_str(),
        data_look,
        effective_config,
        &redux_border_colors,
        &redux_background_colors,
        theme_color_limit,
        Some(entity_theme.font_family_css()),
        entity_theme.font_size_override_css(),
    )?;
    entity_theme_receipt
        .record_typography_css_emission_with_font_size(&emitted_font_family, &emitted_font_size);

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");

    // Markers ported from Mermaid `@11.12.2` `erMarkers.js`.
    // Note: ids follow Mermaid marker rules: `${diagramId}_${diagramType}-${markerType}{Start|End}`.
    // Mermaid's ER unified renderer enables four marker types by default; include MD_PARENT only if used.
    // Mermaid emits one `<defs>` wrapper per marker. Only marker definitions referenced by a
    // visible relationship receive the direct Relation.stroke terminal override.
    if include_md_parent {
        for (suffix, ref_x, marker_width) in
            [("mdParentStart", "0", "190"), ("mdParentEnd", "19", "20")]
        {
            let marker_id = format!("{diagram_id}_{diagram_type}-{suffix}");
            let expected = entity_theme_receipt.expects_relation_marker(&marker_id);
            let expected_stroke = entity_theme_receipt.relation_marker_stroke(&marker_id);
            let emitted_stroke = expected_stroke
                .as_ref()
                .map(|(rule_index, css)| (*rule_index, css.as_str()));
            write_er_marker_definition(
                &mut out,
                &marker_id,
                "marker mdParent er",
                ref_x,
                "7",
                marker_width,
                if suffix.ends_with("Start") {
                    "240"
                } else {
                    "28"
                },
                r#"<path d="M 18,7 L9,13 L1,7 L9,1 Z"/>"#,
                emitted_stroke,
            )?;
            if expected {
                entity_theme_receipt
                    .record_checkpointed_relation_marker(&marker_id, emitted_stroke);
            }
        }
    }

    for (suffix, class, ref_x, ref_y, width, height, content) in [
        (
            "onlyOneStart",
            "marker onlyOne er",
            "0",
            "9",
            "18",
            "18",
            r#"<path d="M9,0 L9,18 M15,0 L15,18"/>"#,
        ),
        (
            "onlyOneEnd",
            "marker onlyOne er",
            "18",
            "9",
            "18",
            "18",
            r#"<path d="M3,0 L3,18 M9,0 L9,18"/>"#,
        ),
        (
            "zeroOrOneStart",
            "marker zeroOrOne er",
            "0",
            "9",
            "30",
            "18",
            r#"<circle fill="white" cx="21" cy="9" r="6"/><path d="M9,0 L9,18"/>"#,
        ),
        (
            "zeroOrOneEnd",
            "marker zeroOrOne er",
            "30",
            "9",
            "30",
            "18",
            r#"<circle fill="white" cx="9" cy="9" r="6"/><path d="M21,0 L21,18"/>"#,
        ),
        (
            "oneOrMoreStart",
            "marker oneOrMore er",
            "18",
            "18",
            "45",
            "36",
            r#"<path d="M0,18 Q 18,0 36,18 Q 18,36 0,18 M42,9 L42,27"/>"#,
        ),
        (
            "oneOrMoreEnd",
            "marker oneOrMore er",
            "27",
            "18",
            "45",
            "36",
            r#"<path d="M3,9 L3,27 M9,18 Q27,0 45,18 Q27,36 9,18"/>"#,
        ),
        (
            "zeroOrMoreStart",
            "marker zeroOrMore er",
            "18",
            "18",
            "57",
            "36",
            r#"<circle fill="white" cx="48" cy="18" r="6"/><path d="M0,18 Q18,0 36,18 Q18,36 0,18"/>"#,
        ),
        (
            "zeroOrMoreEnd",
            "marker zeroOrMore er",
            "39",
            "18",
            "57",
            "36",
            r#"<circle fill="white" cx="9" cy="18" r="6"/><path d="M21,18 Q39,0 57,18 Q39,36 21,18"/>"#,
        ),
    ] {
        let marker_id = format!("{diagram_id}_{diagram_type}-{suffix}");
        let expected = entity_theme_receipt.expects_relation_marker(&marker_id);
        let expected_stroke = entity_theme_receipt.relation_marker_stroke(&marker_id);
        let emitted_stroke = expected_stroke
            .as_ref()
            .map(|(rule_index, css)| (*rule_index, css.as_str()));
        write_er_marker_definition(
            &mut out,
            &marker_id,
            class,
            ref_x,
            ref_y,
            width,
            height,
            content,
            emitted_stroke,
        )?;
        if expected {
            entity_theme_receipt.record_checkpointed_relation_marker(&marker_id, emitted_stroke);
        }
    }

    let mut entity_by_id: std::collections::HashMap<&str, &crate::er::ErEntity> =
        std::collections::HashMap::new();
    for e in model.entities.values() {
        entity_by_id.insert(e.id.as_str(), e);
    }
    let subgraph_context = ErSubgraphRenderContext {
        diagram_id,
        data_look,
        classes: &model.classes,
        use_html_labels: matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike),
        translate_x,
        translate_y,
    };

    // Mermaid 11.17 renders both providers through the common layout painter. The provider only
    // changes the edge group name and z-order: ELK lowers `.edges` beneath `.clusters`, while
    // Dagre keeps the ordinary `edgePaths` group after clusters.
    let _ = writeln!(&mut out, r#"<g class="root">"#);
    if !is_elk_layout {
        if layout.clusters.is_empty() {
            out.push_str(r#"<g class="clusters"/>"#);
        } else {
            out.push_str(r#"<g class="clusters">"#);
            render_er_subgraph_clusters(&mut out, &layout.clusters, model, subgraph_context);
            out.push_str("</g>");
        }
    }

    if is_elk_layout {
        out.push_str(r#"<g class="edges edgePath">"#);
    } else {
        out.push_str(r#"<g class="edgePaths">"#);
    }
    out.checkpoint()?;
    if options.debug.include_edges {
        for e in &edges {
            if e.points.len() < 2 {
                continue;
            }
            let edge_dom_id = er_edge_dom_id(&e.id, &model.relationships);
            let edge_svg_id = format!("{diagram_id}-{edge_dom_id}");
            options.checkpoint_emit()?;
            let is_dashed = e.stroke_dasharray.as_deref() == Some("8,8");
            let pattern_class = if is_dashed {
                "edge-pattern-dashed"
            } else {
                "edge-pattern-solid"
            };
            let line_classes = format!("edge-thickness-normal {pattern_class} relationshipLine");
            let shifted: Vec<crate::model::LayoutPoint> = e
                .points
                .iter()
                .map(|p| crate::model::LayoutPoint {
                    x: p.x + translate_x,
                    y: p.y + translate_y,
                })
                .collect();
            let data_points = base64::engine::general_purpose::STANDARD
                .encode(serde_json::to_vec(&shifted).unwrap_or_default());
            let mut curve_points = shifted.clone();
            if curve_points.len() == 2 {
                let a = &curve_points[0];
                let b = &curve_points[1];
                curve_points.insert(
                    1,
                    crate::model::LayoutPoint {
                        x: (a.x + b.x) / 2.0,
                        y: (a.y + b.y) / 2.0,
                    },
                );
            }
            let d = curve_basis_path_d(&curve_points);
            let typed_relation_stroke = er_rel_idx_from_edge_id(&e.id)
                .and_then(|index| entity_theme.typed_relation_stroke(index));
            let path_style = typed_relation_stroke.map_or_else(
                || "undefined;;;undefined".to_string(),
                |(_, css)| format!("undefined;;;undefined;stroke:{css}"),
            );

            let _ = write!(
                &mut out,
                r#"<path d="{}" id="{}" class="{}" style="{}" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
                escape_xml(&d),
                escape_xml(&edge_svg_id),
                escape_xml(&line_classes),
                escape_attr(&path_style),
                escape_xml(&edge_dom_id),
                escape_xml(&data_points),
                escape_xml(data_look)
            );
            let start_marker_id = e
                .start_marker
                .as_deref()
                .map(|marker| er_unified_marker_id(diagram_id, diagram_type, marker));
            let end_marker_id = e
                .end_marker
                .as_deref()
                .map(|marker| er_unified_marker_id(diagram_id, diagram_type, marker));
            if let Some(marker) = &start_marker_id {
                let _ = write!(&mut out, r#" marker-start="url(#{})""#, escape_attr(marker));
            }
            if let Some(marker) = &end_marker_id {
                let _ = write!(&mut out, r#" marker-end="url(#{})""#, escape_attr(marker));
            }
            options.checkpoint_emit()?;
            out.push_str(" />");
            out.checkpoint()?;
            entity_theme_receipt.record_checkpointed_relation_path(
                &e.id,
                typed_relation_stroke,
                start_marker_id.as_deref(),
                end_marker_id.as_deref(),
            );
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;

    // The published `@mermaid-js/layout-elk@0.2.3` common painter lowers the edge group after
    // insertion, so its root order is edges, clusters, edgeLabels, nodes.
    if is_elk_layout {
        if layout.clusters.is_empty() {
            out.push_str(r#"<g class="clusters"/>"#);
        } else {
            out.push_str(r#"<g class="clusters">"#);
            render_er_subgraph_clusters(&mut out, &layout.clusters, model, subgraph_context);
            out.push_str("</g>");
        }
    }

    out.push_str(r#"<g class="edgeLabels">"#);
    out.checkpoint()?;
    if options.debug.include_edges {
        for e in &edges {
            let rel_idx = er_rel_idx_from_edge_id(&e.id)
                .and_then(|idx| model.relationships.get(idx).map(|r| (idx, r)));

            let rel_text_raw = rel_idx.map(|(_, r)| r.role_a.as_str()).unwrap_or("");
            let rel_text = rel_text_raw.trim();
            let edge_dom_id = er_edge_dom_id(&e.id, &model.relationships);
            let relationship_index = rel_idx.map(|(index, _)| index);
            let prepared_relationship =
                relationship_index.and_then(|index| layout.prepared_labels.relationship(index));
            let typed_relation_label =
                relationship_index.and_then(|index| entity_theme.typed_relation_label(index));
            let relation_label_terminal_style = typed_text_terminal_style(typed_relation_label);

            let has_label_text = !rel_text.is_empty();
            let has_whitespace_only_label = !rel_text_raw.is_empty() && rel_text.is_empty();
            let (w, h, mut cx, mut cy) = if has_label_text {
                if let Some(lbl) = &e.label {
                    (
                        lbl.width.max(0.0),
                        lbl.height.max(0.0),
                        lbl.x + translate_x,
                        lbl.y + translate_y,
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                }
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

            if has_label_text && w > 0.0 && h > 0.0 {
                // Mermaid positions edge labels using Dagre's `edge.x/edge.y` by default, but it
                // recomputes the label position along the polyline when the edge path `d` doesn't
                // contain the midpoint coordinates (see `edges.js:isLabelCoordinateInPath`).
                //
                // Replicate that behavior here to match upstream DOM parity for certain curved
                // edges (notably parallel relationship edges in ER diagrams).
                let shifted: Vec<crate::model::LayoutPoint> = e
                    .points
                    .iter()
                    .map(|p| crate::model::LayoutPoint {
                        x: p.x + translate_x,
                        y: p.y + translate_y,
                    })
                    .collect();
                let mut curve_points = shifted.clone();
                if curve_points.len() == 2 {
                    let a = &curve_points[0];
                    let b = &curve_points[1];
                    curve_points.insert(
                        1,
                        crate::model::LayoutPoint {
                            x: (a.x + b.x) / 2.0,
                            y: (a.y + b.y) / 2.0,
                        },
                    );
                }
                let rendered_d = curve_basis_path_d(&curve_points);
                let position = super::super::edge_label_geometry::position_edge_label(
                    crate::model::LayoutPoint { x: cx, y: cy },
                    &shifted,
                    &rendered_d,
                    false,
                );
                cx = position.x;
                cy = position.y;
            }

            if has_label_text && w > 0.0 && h > 0.0 {
                // Mermaid ER relationship labels follow Mermaid's effective HTML-label
                // resolution: root `htmlLabels`, then `flowchart.htmlLabels`, then default
                // `true`. When both are unset, upstream still emits HTML `<foreignObject>`
                // labels through `createText(...)`.
                let _ = write!(
                    &mut out,
                    r#"<g class="edgeLabel" transform="translate({}, {})">"#,
                    fmt(cx),
                    fmt(cy)
                );
                let _ = write!(
                    &mut out,
                    r#"<g class="label" data-id="{}" transform="translate({}, {})""#,
                    escape_xml_display(&edge_dom_id),
                    fmt(-w / 2.0),
                    fmt(-h / 2.0)
                );
                if let Some((_, css)) = typed_relation_label {
                    let _ = write!(&mut out, r#" style="fill:{} !important""#, escape_attr(css));
                }
                out.push('>');
                if edge_html_labels {
                    let _ = write!(
                        &mut out,
                        r#"<foreignObject width="{}" height="{}">"#,
                        fmt(w),
                        fmt(h)
                    );
                    out.push_str(r#"<div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel""#);
                    if !relation_label_terminal_style.is_empty() {
                        let _ = write!(
                            &mut out,
                            r#" style="{}""#,
                            escape_attr(&relation_label_terminal_style)
                        );
                    }
                    out.push('>');
                    // Mermaid ER relationship labels use the generic HTML edge-label path, so they
                    // inherit `markdownToHTML()` semantics (Markdown emphasis + inline `<br/>`
                    // handling) rather than rendering literal `**...**` marker text.
                    out.push_str(
                        prepared_relationship
                            .map(crate::er::ErPreparedRelationshipLabel::xhtml_fragment)
                            .unwrap_or_default(),
                    );
                    out.push_str(r#"</span></div></foreignObject></g></g>"#);
                } else {
                    if relation_label_terminal_style.is_empty() {
                        out.push_str("<g>");
                    } else {
                        let _ = write!(
                            &mut out,
                            r#"<g style="{}">"#,
                            escape_attr(&relation_label_terminal_style)
                        );
                    }
                    let _ = write!(
                        &mut out,
                        r#"<rect class="background" style="" x="{}" y="-1" width="{}" height="{}"/>"#,
                        fmt(-w / 2.0),
                        fmt(w),
                        fmt(h)
                    );
                    if relation_label_terminal_style.is_empty() {
                        crate::svg::parity::flowchart::write_flowchart_svg_text_centered(
                            &mut out, rel_text, true,
                        );
                    } else {
                        let source_lines =
                            crate::flowchart::flowchart_non_markdown_svg_source_word_lines(
                                rel_text,
                            );
                        crate::svg::parity::flowchart::write_flowchart_svg_source_word_lines_centered_with_style(
                            &mut out,
                            &source_lines,
                            &relation_label_terminal_style,
                        );
                    }
                    out.push_str("</g></g></g>");
                }
                if entity_theme_receipt.records_text_terminals()
                    && let Some(relationship_index) = relationship_index
                    && let Some(facts) = prepared_relationship
                        .map(crate::er::ErPreparedRelationshipLabel::visible_style_facts)
                    && facts.parse_valid()
                    && facts.has_visible_runs()
                {
                    entity_theme_receipt.record_relation_label_text(
                        relationship_index,
                        &relation_label_terminal_style,
                        facts.visible_run_count(),
                        facts.inherited_color_run_count(),
                        facts.inherited_font_family_run_count(),
                        facts.unverified_font_family_run_count(),
                        facts.inherited_font_size_run_count(),
                        facts.unverified_font_size_run_count(),
                    );
                }
            } else {
                if edge_html_labels {
                    // Mermaid emits a `translate(undefined,NaN)` transform for relationship labels
                    // that are whitespace-only (but not for fully empty strings). Preserve that
                    // oddity for DOM parity in `structure` mode (see upstream Cypress fixture
                    // `*_blank_or_empty_labels_007`).
                    if has_whitespace_only_label {
                        out.push_str(r#"<g class="edgeLabel" transform="translate(undefined,NaN)"><g class="label""#);
                    } else {
                        out.push_str(r#"<g class="edgeLabel"><g class="label""#);
                    }
                    let _ = write!(
                        &mut out,
                        r#" data-id="{}""#,
                        escape_xml_display(&edge_dom_id)
                    );
                    out.push_str(r#" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#);
                } else {
                    if has_whitespace_only_label {
                        out.push_str(r#"<g class="edgeLabel" transform="translate(undefined,NaN)"><g class="label""#);
                    } else {
                        out.push_str(r#"<g class="edgeLabel"><g class="label""#);
                    }
                    let _ = write!(
                        &mut out,
                        r#" data-id="{}""#,
                        escape_xml_display(&edge_dom_id)
                    );
                    out.push_str(r#" transform="translate(0, 0)"><g><rect class="background" style="" x="0" y="-1" width="0" height="0"/>"#);
                    crate::svg::parity::flowchart::write_flowchart_svg_text_centered(
                        &mut out, "", true,
                    );
                    out.push_str("</g></g></g>");
                }
            }
            out.checkpoint()?;
        }
    }
    out.push_str("</g>\n");
    out.checkpoint()?;

    // Entities drawn after relationships so they cover markers when overlapping.
    out.push_str(r#"<g class="nodes">"#);
    out.checkpoint()?;
    for n in &nodes {
        if n.is_cluster {
            continue;
        }
        let Some(entity) = entity_by_id.get(n.id.as_str()).copied() else {
            if n.id.contains("---") {
                let cx = n.x + translate_x;
                let cy = n.y + translate_y;
                let _ = write!(
                    &mut out,
                    r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})">"#,
                    escape_xml(&n.id),
                    fmt(cx),
                    fmt(cy)
                );
                out.push_str(r#"<rect width="0.1" height="0.1"/>"#);
                if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
                    out.push_str(r#"<g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#);
                } else {
                    out.push_str(
                        r#"<g class="label" style="" transform="translate(0, 0)"><rect/>"#,
                    );
                    write_er_svg_label_background(&mut out);
                    write_er_svg_plain_label(&mut out, "");
                    out.push_str("</g></g>");
                }
            }
            out.checkpoint()?;
            continue;
        };

        let Some(entity_index) = entity_theme.index_for_entity_id(&entity.id) else {
            return Err(Error::InvalidModel {
                message: format!(
                    "ER entity theme plan is missing semantic entity {}",
                    entity.id
                ),
            });
        };

        let source_style =
            entity_theme
                .source_style(entity_index)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "ER entity theme plan is missing source style for {}",
                        entity.id
                    ),
                })?;
        let rect_style_decls = source_style.rect_declarations();
        let text_style_decls = source_style.text_declarations();
        let source_fill = source_style.fill();
        let source_stroke = source_style.stroke();
        let typed_fill = entity_theme.typed_fill(entity_index);
        let typed_stroke = entity_theme.typed_stroke(entity_index);
        let escaped_text_style = (!text_style_decls.is_empty())
            .then(|| escape_xml(&style_decls_with_important(text_style_decls)));

        let measure =
            layout
                .prepared_labels
                .entity(&entity.id)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("ER prepared labels are missing entity {}", entity.id),
                })?;
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        if (measure.width - w).abs() > 1e-3 || (measure.height - h).abs() > 1e-3 {
            return Err(Error::InvalidModel {
                message: format!(
                    "ER entity measured size mismatch for {}: layout=({},{}), measure=({}, {})",
                    n.id, w, h, measure.width, measure.height
                ),
            });
        }

        let cx = n.x + translate_x;
        let cy = n.y + translate_y;
        let ox = -w / 2.0;
        let oy = -h / 2.0;

        let group_class = if entity.css_classes.trim().is_empty() {
            "node".to_string()
        } else {
            format!("node {}", entity.css_classes.trim())
        };
        let color_id_attr = color_indices
            .get(entity.id.as_str())
            .and_then(|index| er_redux_color_id(&redux_border_colors, *index))
            .map(|color_id| format!(r#" data-color-id="{}""#, escape_xml(&color_id)))
            .unwrap_or_default();
        let _ = write!(
            &mut out,
            r#"<g id="{}-{}" class="{}" data-look="{}"{} transform="translate({}, {})">"#,
            diagram_id,
            escape_xml(&entity.id),
            escape_xml(&group_class),
            escape_xml(data_look),
            color_id_attr,
            fmt(cx),
            fmt(cy)
        );
        out.checkpoint()?;

        if entity.attributes.is_empty() {
            let typed_entity_name = entity_theme.typed_entity_name(&entity.id);
            let entity_name_style_attr = er_text_style_attr(text_style_decls, typed_entity_name);
            let entity_name_terminal_style = typed_text_terminal_style(typed_entity_name);
            let paint_emission = {
                let (rect_style_attr, paint_emission) =
                    entity_rect_style_attr(rect_style_decls, typed_fill, typed_stroke);
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic label-container" {} x="{}" y="{}" width="{}" height="{}"/>"#,
                    rect_style_attr,
                    fmt(ox),
                    fmt(oy),
                    fmt(w),
                    fmt(h)
                );
                out.checkpoint()?;
                paint_emission
            };
            let wrap_mode = entity_wrap_mode;
            let label_metrics = measurer.measure_wrapped(
                measure.label.rendered_text(),
                &label_style,
                None,
                wrap_mode,
            );
            let lw = if wrap_mode == crate::text::WrapMode::HtmlLike {
                measure.label_html_width.max(0.0)
            } else {
                label_metrics.width.max(0.0)
            };
            let lh = label_metrics.height.max(0.0);

            if matches!(wrap_mode, crate::text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label" transform="translate({}, {})" {}><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;">{}</div></foreignObject></g>"#,
                    fmt(-lw / 2.0),
                    fmt(-lh / 2.0),
                    entity_name_style_attr,
                    fmt(lw),
                    fmt(lh),
                    measure.label_max_width_px.max(0),
                    html_label_content(&measure.label, "", true)
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label" transform="translate({}, {})" {}><rect/><g transform="translate({}, 0)">"#,
                    fmt(0.0),
                    fmt(-lh / 2.0),
                    entity_name_style_attr,
                    fmt(-lw / 2.0),
                );
                write_er_svg_label_background(&mut out);
                write_er_svg_box_label(&mut out, &measure.label, &label_style, measurer, None);
                out.push_str("</g></g>");
            }
            out.push_str("</g>");
            out.checkpoint()?;
            entity_theme_receipt.record_checkpointed_entity(
                entity_index,
                paint_emission.emitted_fill,
                paint_emission.emitted_stroke,
            );
            if entity_theme_receipt.records_text_terminals()
                && measure.label.visible_style_facts().parse_valid()
                && measure.label.visible_style_facts().has_visible_runs()
            {
                entity_theme_receipt.record_entity_name_text(
                    &entity.id,
                    &entity_name_terminal_style,
                    measure.label.visible_style_facts().visible_run_count(),
                    measure
                        .label
                        .visible_style_facts()
                        .inherited_color_run_count(),
                    measure
                        .label
                        .visible_style_facts()
                        .inherited_font_family_run_count(),
                    measure
                        .label
                        .visible_style_facts()
                        .unverified_font_family_run_count(),
                    measure
                        .label
                        .visible_style_facts()
                        .inherited_font_size_run_count(),
                    measure
                        .label
                        .visible_style_facts()
                        .unverified_font_size_run_count(),
                    label_style.font_size,
                );
            }
            continue;
        }

        fn html_label_content(
            label: &crate::er::ErBoxLabel,
            span_style_attr: &str,
            markdown_node_label: bool,
        ) -> String {
            let span_class = if markdown_node_label {
                "nodeLabel markdown-node-label"
            } else {
                "nodeLabel"
            };
            let text = label.rendered_text();
            if text.is_empty() {
                return format!(r#"<span class="{}"{}></span>"#, span_class, span_style_attr);
            }

            if label.uses_generic_workaround() {
                return escape_xml(text);
            }

            format!(
                r#"<span class="{}"{}>{}</span>"#,
                span_class,
                span_style_attr,
                label.xhtml_fragment()
            )
        }

        let label_div_color_prefix = source_style
            .text_value("color")
            .map(|value| {
                format!(
                    "color: {} !important; ",
                    super::super::util::cssom_color_value(value)
                )
            })
            .unwrap_or_default();
        let span_style_attr = escaped_text_style
            .as_deref()
            .map(|style| format!(r#" style="{style}""#))
            .unwrap_or_default();

        // Mermaid ER attribute tables (erBox.ts) use HTML labels (`foreignObject`) and paths for the table rows.
        let name_row_h = (measure.label_height + measure.text_padding).max(1.0);
        let box_x0 = ox;
        let box_y0 = oy;
        let box_x1 = ox + w;
        let box_y1 = oy + h;
        let sep_y = oy + name_row_h;

        let box_fill = source_fill
            .or_else(|| typed_fill.map(|(_, css)| css))
            .unwrap_or(main_bkg.as_str());
        let box_stroke = source_stroke
            .or_else(|| typed_stroke.map(|(_, css)| css))
            .unwrap_or(node_border.as_str());
        let box_stroke_width = source_style
            .rect_value("stroke-width")
            .and_then(parse_px_f64)
            .unwrap_or(1.3)
            .max(0.0);
        let stroke_width_attr = fmt(box_stroke_width);

        let group_style = concat_style_keys(rect_style_decls, &["fill", "stroke", "stroke-width"]);
        let group_style_attr = if group_style.is_empty() {
            r#"style="""#.to_string()
        } else {
            format!(r#"style="{}""#, escape_xml(&group_style))
        };

        let mut override_decls = Vec::new();
        if let Some(value) = source_stroke {
            override_decls.push(format!("stroke:{value} !important"));
        } else if let Some((_, value)) = typed_stroke {
            override_decls.push(format!("stroke:{value}"));
        }
        if let Some(v) = source_style.rect_value("stroke-width") {
            override_decls.push(format!("stroke-width:{v} !important"));
        }
        let override_style_attr = if override_decls.is_empty() {
            String::new()
        } else {
            format!(r#" style="{}""#, escape_attr(&override_decls.join("; ")))
        };
        let base_fill_style_attr = if let Some(value) = source_fill {
            format!(r#" style="fill:{} !important""#, escape_attr(value))
        } else if let Some((_, value)) = typed_fill {
            format!(r#" style="fill:{}""#, escape_attr(value))
        } else {
            String::new()
        };

        // Mermaid erBox.ts uses Rough.js with `roughness=0` for default (non-handDrawn) nodes.
        //
        // Even with roughness=0, Rough.js still depends on seeded randomness via `divergePoint`.
        // For strict SVG parity we use the same Rough.js algorithm (v4.6.6) here instead of a
        // generic sketchy-stroke renderer.
        fn roughjs46_diverge_point(options: &mut roughr::core::Options) -> f64 {
            0.2 + options.random() * 0.2
        }

        fn roughjs46_double_line_path_d(
            options: &mut roughr::core::Options,
            x0: f64,
            y0: f64,
            x1: f64,
            y1: f64,
        ) -> String {
            let mut out = String::new();
            let dx = x1 - x0;
            let dy = y1 - y0;

            for _ in 0..2 {
                let d = roughjs46_diverge_point(options);
                // Rough.js `_line()` continues to call into `_offsetOpt()` even when `roughness=0`
                // (the random terms get multiplied by zero, but the PRNG state still advances).
                //
                // In Rough.js v4.6.6 `_line()` uses:
                // - 2 random() calls for `midDispX/midDispY` offsetOpt
                // - 2 random() calls for moveTo (x1/y1)
                // - 6 random() calls for bcurveTo (cp1/cp2/x2/y2)
                // Total: 10 random() calls after divergePoint.
                for _ in 0..10 {
                    let _ = options.random();
                }
                let cx1 = x0 + dx * d;
                let cy1 = y0 + dy * d;
                let cx2 = x0 + dx * 2.0 * d;
                let cy2 = y0 + dy * 2.0 * d;
                let _ = write!(
                    &mut out,
                    "M{} {} C{} {}, {} {}, {} {} ",
                    x0, y0, cx1, cy1, cx2, cy2, x1, y1
                );
            }

            out.trim_end().to_string()
        }

        fn rough_rect_border_path_d(
            randomness: &roughr::core::RoughRandomness,
            x0: f64,
            y0: f64,
            x1: f64,
            y1: f64,
        ) -> String {
            let w = (x1 - x0).max(0.0);
            let h = (y1 - y0).max(0.0);
            let mut options = roughr::core::OptionsBuilder::default()
                .randomness(randomness.clone())
                .build()
                .expect("ER rough rectangle options must be valid");

            // Rough.js v4.6.6 renderer.rectangle -> polygon -> linearPath:
            //   segments: (x,y)->(x+w,y)->(x+w,y+h)->(x,y+h)->(x,y)
            let mut out = String::new();
            let x2 = x0 + w;
            let y2 = y0 + h;

            let segs = [
                (x0, y0, x2, y0),
                (x2, y0, x2, y2),
                (x2, y2, x0, y2),
                (x0, y2, x0, y0),
            ];
            for (ax, ay, bx, by) in segs {
                let d = roughjs46_double_line_path_d(&mut options, ax, ay, bx, by);
                out.push_str(&d);
                out.push(' ');
            }

            out.trim_end().to_string()
        }

        fn roughjs46_rect_fill_path_d(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
            format!(
                "M{} {} L{} {} L{} {} L{} {}",
                x0, y0, x1, y0, x1, y1, x0, y1
            )
        }

        fn thin_divider_rect_bounds(x0: f64, y0: f64, x1: f64, y1: f64) -> (f64, f64, f64, f64) {
            let half = 0.00005;
            if (y1 - y0).abs() <= (x1 - x0).abs() {
                (x0, y0 - half, x1, y0 + half)
            } else {
                (x0 - half, y0, x0 + half, y1)
            }
        }

        // Base box (fill + border)
        let _ = write!(&mut out, r#"<g {} class="outer-path">"#, group_style_attr);
        let _ = write!(
            &mut out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"{} />"#,
            roughjs46_rect_fill_path_d(box_x0, box_y0, box_x1, box_y1),
            escape_attr(&box_fill),
            base_fill_style_attr
        );
        let _ = write!(
            &mut out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
            rough_rect_border_path_d(&hand_drawn_seed, box_x0, box_y0, box_x1, box_y1),
            escape_attr(&box_stroke),
            stroke_width_attr,
            override_style_attr
        );
        out.push_str("</g>");
        out.checkpoint()?;
        let paint_emission = er_entity_paint_emission(typed_fill, typed_stroke);

        // Row rectangles
        let odd_fill = theme_token(effective_config, "rowOdd", "hsl(240, 100%, 100%)");
        let even_fill = theme_token(
            effective_config,
            "rowEven",
            "hsl(240, 100%, 97.2745098039%)",
        );
        let even_row_override_style_attr = if source_fill.is_some() {
            let style = style_keys_join(
                rect_style_decls,
                &["fill", "stroke", "stroke-width"],
                ";",
                true,
            );
            if style.is_empty() {
                override_style_attr.clone()
            } else {
                format!(r#" style="{}""#, escape_xml(&style))
            }
        } else {
            override_style_attr.clone()
        };
        let mut y = sep_y;
        for (idx, row) in measure.rows.iter().enumerate() {
            let row_h = row.height.max(1.0);
            let y0 = y;
            let y1 = y + row_h;
            y = y1;
            let is_odd = idx % 2 == 0;
            let row_class = if is_odd {
                "row-rect-odd"
            } else {
                "row-rect-even"
            };
            let row_fill = if is_odd {
                odd_fill.as_str()
            } else {
                even_fill.as_str()
            };
            let _ = write!(
                &mut out,
                r#"<g {} class="{}">"#,
                group_style_attr, row_class
            );
            let row_override_style_attr = if is_odd {
                override_style_attr.as_str()
            } else {
                even_row_override_style_attr.as_str()
            };
            let typed_row_fill = entity_theme.typed_table_row(&entity.id, idx);
            let typed_row_terminal_style = typed_row_fill
                .map(|(_, css)| format!("fill:{css}"))
                .unwrap_or_default();
            let typed_row_style_attr = typed_row_fill.map(|(_, css)| {
                let mut declarations = vec![format!("fill:{css}")];
                declarations.extend(override_decls.iter().cloned());
                format!(r#" style="{}""#, escape_attr(&declarations.join("; ")))
            });
            let row_fill_style_attr = typed_row_style_attr
                .as_deref()
                .unwrap_or(row_override_style_attr);
            let _ = write!(
                &mut out,
                r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"{} />"#,
                roughjs46_rect_fill_path_d(box_x0, y0, box_x1, y1),
                escape_xml(row_fill),
                row_fill_style_attr
            );
            let _ = write!(
                &mut out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                rough_rect_border_path_d(&hand_drawn_seed, box_x0, y0, box_x1, y1),
                escape_xml(&node_border),
                stroke_width_attr,
                row_override_style_attr
            );
            out.push_str("</g>");
            out.checkpoint()?;
            entity_theme_receipt.record_table_row(&entity.id, idx, &typed_row_terminal_style);
        }

        // ER entity tables read root `htmlLabels` directly. This intentionally differs from
        // relationship labels, whose renderer consults `flowchart.htmlLabels` as a fallback.
        // The HTML path keeps Mermaid's foreignObject structure; the SVG path below mirrors
        // createText(..., useHtmlLabels: false) with a background group and text/tspan output.
        let line_h = (font_size * 1.5).max(1.0);
        let mut pad = entity_measurement.diagram_padding;
        // Keep parity with Mermaid's erBox.ts `if (!config.htmlLabels) { PADDING *= 1.25; }`:
        // when `htmlLabels` is unset (undefined), upstream still applies the 1.25 multiplier.
        if !entity_measurement.html_labels_raw {
            pad *= 1.25;
        }

        let name_w = measure.label_html_width.max(0.0);
        let name_x = -name_w / 2.0;
        let name_y = oy + name_row_h / 2.0 - line_h / 2.0;
        let name_mw_px = crate::er::calculate_text_width_like_mermaid_px(
            measurer,
            &label_style,
            measure.label.markdown_input(),
        ) + 100;
        let typed_entity_name = entity_theme.typed_entity_name(&entity.id);
        let entity_name_style_attr = er_text_style_attr(text_style_decls, typed_entity_name);
        let entity_name_terminal_style = typed_text_terminal_style(typed_entity_name);
        if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
            let _ = write!(
                &mut out,
                r#"<g class="label name" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: start;">{}"#,
                fmt(name_x),
                fmt(name_y),
                entity_name_style_attr,
                fmt(name_w),
                fmt(line_h),
                escape_xml(&label_div_color_prefix),
                name_mw_px.max(0),
                html_label_content(&measure.label, &span_style_attr, false)
            );
            out.push_str("</div></foreignObject></g>");
        } else {
            let _ = write!(
                &mut out,
                r#"<g class="label name" transform="translate({}, {})" {}>"#,
                fmt(name_x),
                fmt(name_y),
                entity_name_style_attr,
            );
            write_er_svg_label_background(&mut out);
            write_er_svg_box_label(
                &mut out,
                &measure.label,
                &label_style,
                measurer,
                Some(name_mw_px.max(0) as f64),
            );
            out.push_str("</g>");
        }
        out.checkpoint()?;
        if entity_theme_receipt.records_text_terminals()
            && measure.label.visible_style_facts().parse_valid()
            && measure.label.visible_style_facts().has_visible_runs()
        {
            entity_theme_receipt.record_entity_name_text(
                &entity.id,
                &entity_name_terminal_style,
                measure.label.visible_style_facts().visible_run_count(),
                measure
                    .label
                    .visible_style_facts()
                    .inherited_color_run_count(),
                measure
                    .label
                    .visible_style_facts()
                    .inherited_font_family_run_count(),
                measure
                    .label
                    .visible_style_facts()
                    .unverified_font_family_run_count(),
                measure
                    .label
                    .visible_style_facts()
                    .inherited_font_size_run_count(),
                measure
                    .label
                    .visible_style_facts()
                    .unverified_font_size_run_count(),
                label_style.font_size,
            );
        }

        let type_col_w = measure.type_col_w.max(0.0);
        let name_col_w = measure.name_col_w.max(0.0);
        let key_col_w = measure.key_col_w.max(0.0);
        let _comment_col_w = measure.comment_col_w.max(0.0);

        let left_text_x = ox + pad / 2.0;
        let type_left = left_text_x;
        let name_left = left_text_x + type_col_w;
        let key_left = left_text_x + type_col_w + name_col_w;
        let comment_left = left_text_x + type_col_w + name_col_w + key_col_w;

        let mut row_top = sep_y;
        for (row_index, row) in measure.rows.iter().enumerate() {
            let row_h = row.height.max(1.0);
            let cell_y = row_top + row_h / 2.0 - line_h / 2.0;

            let type_w = crate::er::er_box_label_metrics_with_wrap_mode(
                &row.type_label,
                measurer,
                &attr_style,
                entity_wrap_mode,
            )
            .width
            .max(0.0);
            let name_w = crate::er::er_box_label_metrics_with_wrap_mode(
                &row.name_label,
                measurer,
                &attr_style,
                entity_wrap_mode,
            )
            .width
            .max(0.0);
            let keys_w = crate::er::er_box_label_metrics_with_wrap_mode(
                &row.key_label,
                measurer,
                &attr_style,
                entity_wrap_mode,
            )
            .width
            .max(0.0);
            let comment_w = crate::er::er_box_label_metrics_with_wrap_mode(
                &row.comment_label,
                measurer,
                &attr_style,
                entity_wrap_mode,
            )
            .width
            .max(0.0);

            let type_mw_px = crate::er::calculate_text_width_like_mermaid_px(
                measurer,
                &attr_style,
                row.type_label.markdown_input(),
            ) + 100;
            let name_mw_px = crate::er::calculate_text_width_like_mermaid_px(
                measurer,
                &attr_style,
                row.name_label.markdown_input(),
            ) + 100;
            let keys_mw_px = crate::er::calculate_text_width_like_mermaid_px(
                measurer,
                &attr_style,
                row.key_label.markdown_input(),
            ) + 100;
            let comment_mw_px = crate::er::calculate_text_width_like_mermaid_px(
                measurer,
                &attr_style,
                row.comment_label.markdown_input(),
            ) + 100;

            let type_paint = entity_theme.typed_attribute_text(
                &entity.id,
                row_index,
                crate::er::ErAttributeTextRole::Type,
            );
            let name_paint = entity_theme.typed_attribute_text(
                &entity.id,
                row_index,
                crate::er::ErAttributeTextRole::Name,
            );
            let keys_paint = entity_theme.typed_attribute_text(
                &entity.id,
                row_index,
                crate::er::ErAttributeTextRole::Keys,
            );
            let comment_paint = entity_theme.typed_attribute_text(
                &entity.id,
                row_index,
                crate::er::ErAttributeTextRole::Comment,
            );
            let type_style_attr = er_text_style_attr(text_style_decls, type_paint);
            let name_style_attr = er_text_style_attr(text_style_decls, name_paint);
            let keys_style_attr = er_text_style_attr(text_style_decls, keys_paint);
            let comment_style_attr = er_text_style_attr(text_style_decls, comment_paint);

            if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-type" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: start;">{}"#,
                    fmt(type_left),
                    fmt(cell_y),
                    type_style_attr,
                    fmt(type_w),
                    fmt(line_h),
                    escape_xml(&label_div_color_prefix),
                    type_mw_px.max(0),
                    html_label_content(&row.type_label, &span_style_attr, false)
                );
                out.push_str("</div></foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-type" transform="translate({}, {})" {}>"#,
                    fmt(type_left),
                    fmt(cell_y),
                    type_style_attr,
                );
                write_er_svg_label_background(&mut out);
                write_er_svg_box_label(
                    &mut out,
                    &row.type_label,
                    &attr_style,
                    measurer,
                    Some(type_mw_px.max(0) as f64),
                );
                out.push_str("</g>");
            }

            if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-name" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: start;">{}"#,
                    fmt(name_left),
                    fmt(cell_y),
                    name_style_attr,
                    fmt(name_w),
                    fmt(line_h),
                    escape_xml(&label_div_color_prefix),
                    name_mw_px.max(0),
                    html_label_content(&row.name_label, &span_style_attr, false)
                );
                out.push_str("</div></foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-name" transform="translate({}, {})" {}>"#,
                    fmt(name_left),
                    fmt(cell_y),
                    name_style_attr,
                );
                write_er_svg_label_background(&mut out);
                write_er_svg_box_label(
                    &mut out,
                    &row.name_label,
                    &attr_style,
                    measurer,
                    Some(name_mw_px.max(0) as f64),
                );
                out.push_str("</g>");
            }

            if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-keys" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: start;">{}"#,
                    fmt(key_left),
                    fmt(cell_y),
                    keys_style_attr,
                    fmt(keys_w),
                    fmt(if row.key_label.rendered_text().is_empty() {
                        0.0
                    } else {
                        line_h
                    }),
                    escape_xml(&label_div_color_prefix),
                    keys_mw_px.max(0),
                    html_label_content(&row.key_label, &span_style_attr, false)
                );
                out.push_str("</div></foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-keys" transform="translate({}, {})" {}>"#,
                    fmt(key_left),
                    fmt(cell_y),
                    keys_style_attr,
                );
                write_er_svg_label_background(&mut out);
                write_er_svg_box_label(
                    &mut out,
                    &row.key_label,
                    &attr_style,
                    measurer,
                    Some(keys_mw_px.max(0) as f64),
                );
                out.push_str("</g>");
            }

            if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-comment" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: start;">{}"#,
                    fmt(comment_left),
                    fmt(cell_y),
                    comment_style_attr,
                    fmt(comment_w),
                    fmt(if row.comment_label.rendered_text().is_empty() {
                        0.0
                    } else {
                        line_h
                    }),
                    escape_xml(&label_div_color_prefix),
                    comment_mw_px.max(0),
                    html_label_content(&row.comment_label, &span_style_attr, false)
                );
                out.push_str("</div></foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-comment" transform="translate({}, {})" {}>"#,
                    fmt(comment_left),
                    fmt(cell_y),
                    comment_style_attr,
                );
                write_er_svg_label_background(&mut out);
                write_er_svg_box_label(
                    &mut out,
                    &row.comment_label,
                    &attr_style,
                    measurer,
                    Some(comment_mw_px.max(0) as f64),
                );
                out.push_str("</g>");
            }

            out.checkpoint()?;

            if entity_theme_receipt.records_text_terminals() {
                for (role, label, paint) in [
                    (
                        crate::er::ErAttributeTextRole::Type,
                        &row.type_label,
                        type_paint,
                    ),
                    (
                        crate::er::ErAttributeTextRole::Name,
                        &row.name_label,
                        name_paint,
                    ),
                    (
                        crate::er::ErAttributeTextRole::Keys,
                        &row.key_label,
                        keys_paint,
                    ),
                    (
                        crate::er::ErAttributeTextRole::Comment,
                        &row.comment_label,
                        comment_paint,
                    ),
                ] {
                    let facts = label.visible_style_facts();
                    if facts.parse_valid() && facts.has_visible_runs() {
                        entity_theme_receipt.record_attribute_text(
                            &entity.id,
                            row_index,
                            role,
                            &typed_text_terminal_style(paint),
                            facts.visible_run_count(),
                            facts.inherited_color_run_count(),
                            facts.inherited_font_family_run_count(),
                            facts.unverified_font_family_run_count(),
                            facts.inherited_font_size_run_count(),
                            facts.unverified_font_size_run_count(),
                            attr_style.font_size,
                        );
                    }
                }
            }

            row_top += row_h;
        }

        // Dividers (header separator + column boundaries)
        let divider_style = override_style_attr.clone();
        let divider_path_attrs = format!(
            r#" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{}"#,
            escape_xml(&box_stroke),
            stroke_width_attr,
            divider_style
        );
        // Mermaid `erBox.ts` draws the header separator twice:
        // - once as the explicit "Name line"
        // - once via the later `yOffsets` pass (which always contains `0`)
        #[allow(clippy::too_many_arguments)]
        fn write_divider_group(
            out: &mut impl SvgOutput,
            hand_drawn_seed: &roughr::core::RoughRandomness,
            x0: f64,
            y0: f64,
            x1: f64,
            y1: f64,
            fill: &str,
            divider_path_attrs: &str,
        ) -> Result<()> {
            let (rx0, ry0, rx1, ry1) = thin_divider_rect_bounds(x0, y0, x1, y1);
            let _ = write!(
                out,
                r#"<g class="divider"><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd"/><path d="{}"{} /></g>"#,
                roughjs46_rect_fill_path_d(rx0, ry0, rx1, ry1),
                escape_xml(fill),
                rough_rect_border_path_d(hand_drawn_seed, rx0, ry0, rx1, ry1),
                divider_path_attrs
            );
            out.checkpoint()
        }

        write_divider_group(
            &mut out,
            &hand_drawn_seed,
            box_x0,
            sep_y,
            box_x1,
            sep_y,
            &box_fill,
            &divider_path_attrs,
        )?;

        let mut divider_xs: Vec<f64> = Vec::new();
        divider_xs.push(ox + type_col_w);
        if measure.has_key {
            divider_xs.push(ox + type_col_w + name_col_w);
        }
        if measure.has_comment {
            divider_xs.push(ox + type_col_w + name_col_w + key_col_w);
        }
        for x in divider_xs {
            write_divider_group(
                &mut out,
                &hand_drawn_seed,
                x,
                sep_y,
                x,
                box_y1,
                &box_fill,
                &divider_path_attrs,
            )?;
        }

        write_divider_group(
            &mut out,
            &hand_drawn_seed,
            box_x0,
            sep_y,
            box_x1,
            sep_y,
            &box_fill,
            &divider_path_attrs,
        )?;

        out.push_str("</g>");
        out.checkpoint()?;
        entity_theme_receipt.record_checkpointed_entity(
            entity_index,
            paint_emission.emitted_fill,
            paint_emission.emitted_stroke,
        );
    }
    out.push_str("</g>\n");
    out.checkpoint()?;
    out.push_str("</g>\n</g>\n");
    out.checkpoint()?;

    push_er_gradient(&mut out, diagram_id.semantic_str(), effective_config)?;

    if let Some(title) = diagram_title {
        // Mermaid `utils.insertTitle(...)` appends the title after rendering the graph content.
        // - `text-anchor="middle"`
        // - `x = bounds.x + bounds.width / 2`
        // - `y = -titleTopMargin` (default: 25)
        let fallback_title_x = root_plan
            .view_box()
            .map(|view_box| view_box.min_x + view_box.width / 2.0)
            .unwrap_or(root_width_for_title / 2.0);
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="erDiagramTitleText">{}"#,
            fmt(diagram_title_x.unwrap_or(fallback_title_x)),
            fmt(-insert_title_top_margin),
            escape_xml(title)
        );
        out.push_str("</text>\n");
        out.checkpoint()?;
        entity_theme_receipt.record_diagram_title_emission("erDiagramTitleText", title);
    }

    push_er_shadow_defs(&mut out, diagram_id.semantic_str(), effective_config)?;

    out.push_str("</svg>\n");
    let rooted_svg = root_document.complete(out.finish()?)?;
    if !entity_theme.record_terminal(entity_theme_receipt) {
        return Err(Error::InvalidModel {
            message: "ER theme terminal evidence could not be sealed".to_string(),
        });
    }
    Ok(rooted_svg)
}

fn push_er_shadow_defs(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    effective_config_value: &serde_json::Value,
) -> Result<()> {
    let flood_color = effective_config_value
        .get("theme")
        .and_then(|v| v.as_str())
        .filter(|theme| theme.contains("dark"))
        .map(|_| "#FFFFFF")
        .unwrap_or("#000000");
    let _ = write!(
        out,
        r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        diagram_id, flood_color, diagram_id, flood_color
    );
    out.checkpoint()
}

fn push_er_gradient(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    effective_config_value: &serde_json::Value,
) -> Result<()> {
    if !config_bool(effective_config_value, &["themeVariables", "useGradient"]).unwrap_or(false) {
        return Ok(());
    }

    let gradient_start =
        config_string(effective_config_value, &["themeVariables", "gradientStart"])
            .or_else(|| {
                config_string(
                    effective_config_value,
                    &["themeVariables", "primaryBorderColor"],
                )
            })
            .unwrap_or_else(|| "#9370DB".to_string());
    let gradient_stop = config_string(effective_config_value, &["themeVariables", "gradientStop"])
        .or_else(|| {
            config_string(
                effective_config_value,
                &["themeVariables", "secondaryBorderColor"],
            )
        })
        .unwrap_or_else(|| gradient_start.clone());

    let gradient_start = escape_xml(&gradient_start);
    let gradient_stop = escape_xml(&gradient_stop);
    let _ = write!(
        out,
        r#"<linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
        diagram_id,
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
    out.checkpoint()
}

fn er_unified_marker_id(
    diagram_id: SvgDiagramId<'_>,
    diagram_type: &str,
    upstream_marker: &str,
) -> String {
    let upstream_marker = upstream_marker.trim();
    let (base, suffix) = if let Some(v) = upstream_marker.strip_suffix("_START") {
        (v, "Start")
    } else if let Some(v) = upstream_marker.strip_suffix("_END") {
        (v, "End")
    } else {
        return upstream_marker.to_string();
    };

    let marker_type = match base {
        "ONLY_ONE" => "onlyOne",
        "ZERO_OR_ONE" => "zeroOrOne",
        "ONE_OR_MORE" => "oneOrMore",
        "ZERO_OR_MORE" => "zeroOrMore",
        "MD_PARENT" => "mdParent",
        _ => return upstream_marker.to_string(),
    };

    format!("{diagram_id}_{diagram_type}-{marker_type}{suffix}")
}

#[cfg(test)]
mod tests {
    use crate::DiagramFamilyId;
    use crate::model::{Bounds, ErDiagramLayout, LayoutNode};
    use crate::svg::{SvgRenderOptions, with_test_svg_execution};
    use indexmap::IndexMap;
    use merman_core::MermaidConfig;
    use merman_core::diagrams::er::{ErDiagramRenderModel, ErEntityRenderModel};
    use serde_json::json;
    use std::fmt;
    use std::ops::Range;

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl super::SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> crate::Result<()> {
            if self.rejected {
                Err(crate::Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn er_source_important_declarations_are_serialized_once() {
        let entity = ErEntityRenderModel {
            css_styles: vec![
                "fill:#123456 !important".to_string(),
                "stroke:#654321 !important".to_string(),
            ],
            ..ErEntityRenderModel::default()
        };
        let source_style = crate::er::compile_er_entity_source_style(&entity, &IndexMap::new());
        let rect_decls = source_style.rect_declarations();
        let css = super::style_decls_with_important(rect_decls);

        assert_eq!(source_style.fill(), Some("#123456"));
        assert_eq!(css.matches("!important").count(), 2, "{css}");
        assert!(!css.contains("!important !important"), "{css}");
    }

    #[test]
    fn er_redux_color_css_stops_after_the_first_svg_sink_failure() {
        let border_colors = vec!["#e879f9".to_string(), "#2dd4bf".to_string()];
        let background_colors = vec!["#fdf4ff".to_string(), "#f0fdfa".to_string()];
        let mut out = RejectAfterFirstWrite::default();

        let error = super::write_er_redux_color_css(
            &mut out,
            "er",
            "classic",
            &border_colors,
            &background_colors,
            2,
        )
        .expect_err("the rejecting sink must stop ER Redux color CSS emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "ER Redux color CSS emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn er_redux_color_theme_emits_per_entity_color_rules() {
        let config = json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": ["#e879f9", "#2dd4bf"],
                "bkgColorArray": ["#fdf4ff", "#f0fdfa"],
                "THEME_COLOR_LIMIT": 2
            }
        });
        let theme = super::SvgTheme::new(&config);
        let borders = theme.string_array("borderColorArray");
        let backgrounds = theme.string_array("bkgColorArray");

        assert!(super::is_er_redux_color_theme(&config));
        assert_eq!(
            super::er_redux_color_id(&borders, 0).as_deref(),
            Some("color-0")
        );
        assert_eq!(
            super::er_redux_color_id(&borders, 3).as_deref(),
            Some("color-1")
        );
        assert_eq!(
            super::er_redux_color_id(&[], 0),
            None,
            "an empty color palette must not produce a modulo-by-zero color id"
        );

        assert_eq!(super::er_theme_color_limit(&config), 2);
        let mut css = String::new();
        super::write_er_redux_color_css(&mut css, "er", "classic", &borders, &backgrounds, 2)
            .unwrap();
        assert!(css.contains(
            r##"#er [data-look="classic"][data-color-id="color-0"].node path{stroke:#e879f9;fill:#fdf4ff;}"##
        ));
        assert!(css.contains(
            r##"#er [data-look="classic"][data-color-id="color-1"].node rect{stroke:#2dd4bf;fill:#f0fdfa;}"##
        ));

        let mut dark_css = String::new();
        super::write_er_redux_color_css(&mut dark_css, "er", "classic", &borders, &[], 2).unwrap();
        assert!(dark_css.contains(
            r##"#er [data-look="classic"][data-color-id="color-0"].node path{stroke:#e879f9;}"##
        ));
        assert!(!dark_css.contains("fill:"), "{dark_css}");

        let mut merged = String::new();
        super::write_er_style_with_font_family(
            &mut merged,
            "er",
            "classic",
            &config,
            &borders,
            &backgrounds,
            2,
            None,
            None,
        )
        .unwrap();
        let colors = merged.find("[data-color-id=").unwrap();
        let family = merged.find("#er .entityBox").unwrap();
        let root = merged.find("#er :root").unwrap();
        assert!(colors < family && family < root, "{merged}");
    }

    #[test]
    fn er_redux_colors_follow_entity_insertion_ids() {
        let config = json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": ["#e879f9", "#2dd4bf"],
                "bkgColorArray": ["#fdf4ff", "#f0fdfa"],
                "THEME_COLOR_LIMIT": 2
            }
        });
        let zeta = ErEntityRenderModel {
            id: "entity-ZETA-0".to_string(),
            label: "ZETA".to_string(),
            shape: "erBox".to_string(),
            css_classes: "default".to_string(),
            ..ErEntityRenderModel::default()
        };
        let alpha = ErEntityRenderModel {
            id: "entity-ALPHA-1".to_string(),
            label: "ALPHA".to_string(),
            shape: "erBox".to_string(),
            css_classes: "default".to_string(),
            ..ErEntityRenderModel::default()
        };
        let model = ErDiagramRenderModel {
            direction: "TB".to_string(),
            entities: IndexMap::from([
                ("ALPHA".to_string(), alpha.clone()),
                ("ZETA".to_string(), zeta.clone()),
            ]),
            ..ErDiagramRenderModel::default()
        };
        let color_indices = super::er_color_indices(&model);
        assert_eq!(color_indices.get(&zeta.id), Some(&0));
        assert_eq!(color_indices.get(&alpha.id), Some(&1));
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let settings = crate::er::ErConfigView::new(&config).render_settings_with_font_family(None);
        let measure = |entity: &ErEntityRenderModel| {
            crate::er::measure_entity_box(
                entity,
                &measurer,
                &settings.label_style,
                &settings.attr_style,
                settings.entity_measurement,
            )
        };
        let zeta_measure = measure(&zeta);
        let alpha_measure = measure(&alpha);
        let layout = ErDiagramLayout {
            // Deliberately reverse the layout vector. Mermaid colorIndex belongs to the entity,
            // not to whichever order the layout backend returns nodes in.
            nodes: vec![
                LayoutNode {
                    id: alpha.id.clone(),
                    x: 220.0,
                    y: 80.0,
                    width: alpha_measure.width,
                    height: alpha_measure.height,
                    is_cluster: false,
                    label_width: None,
                    label_height: None,
                },
                LayoutNode {
                    id: zeta.id.clone(),
                    x: 80.0,
                    y: 80.0,
                    width: zeta_measure.width,
                    height: zeta_measure.height,
                    is_cluster: false,
                    label_width: None,
                    label_height: None,
                },
            ],
            edges: Vec::new(),
            render_edges: Vec::new(),
            clusters: Vec::new(),
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 300.0,
                max_y: 160.0,
            }),
            prepared_labels: crate::er::prepare_er_labels(&model, &config, &measurer),
        };
        let options = SvgRenderOptions {
            diagram_id: Some("er-colors".to_string()),
            ..SvgRenderOptions::default()
        };
        let effective_config = MermaidConfig::from_value(config.clone());
        let svg = with_test_svg_execution(DiagramFamilyId::ER, &options, |options| {
            let entity_theme = crate::er::ErEntityThemePlan::resolve(
                None,
                &effective_config,
                crate::family::InheritedFontStackPlan::resolve_property_local(
                    None,
                    &effective_config,
                ),
                crate::er::ErBaseFontSizePlan::resolve(None, &effective_config),
                crate::er::ErConfigView::new(effective_config.as_value())
                    .relationship_html_labels(),
                None,
                &model,
                &layout,
                options.work_meter(),
            )
            .expect("resolve baseline ER entity theme");
            super::render_er_diagram_svg_model(
                &layout,
                &model,
                &entity_theme,
                &effective_config,
                None,
                &measurer,
                options,
            )
        })
        .and_then(|svg| svg.into_string_for(DiagramFamilyId::ER))
        .unwrap();

        let zeta_dom = r#"<g id="er-colors-entity-ZETA-0" class="node default" data-look="classic" data-color-id="color-0""#;
        let alpha_dom = r#"<g id="er-colors-entity-ALPHA-1" class="node default" data-look="classic" data-color-id="color-1""#;
        assert!(svg.contains(zeta_dom), "{svg}");
        assert!(svg.contains(alpha_dom), "{svg}");
        assert!(svg.find(zeta_dom).unwrap() < svg.find(alpha_dom).unwrap());
    }

    #[test]
    fn er_font_family_css_uses_mermaid_default_fallback_for_raw_config() {
        assert_eq!(
            crate::er::ErConfigView::new(&json!({}))
                .text_style_with_font_family(None)
                .font_family
                .as_deref(),
            Some(crate::config::MERMAID_DEFAULT_FONT_FAMILY_CSS)
        );
    }

    #[test]
    fn er_font_family_css_prefers_theme_variables() {
        assert_eq!(
            crate::er::ErConfigView::new(&json!({
                "fontFamily": "Courier, monospace",
                "themeVariables": {
                    "fontFamily": "\"IBM Plex Sans\", Arial, sans-serif"
                }
            }))
            .text_style_with_font_family(None)
            .font_family
            .as_deref(),
            Some(r#""IBM Plex Sans",Arial,sans-serif"#)
        );
    }
}
