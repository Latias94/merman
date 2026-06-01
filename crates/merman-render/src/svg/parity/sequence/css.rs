use super::super::*;

pub(super) fn sequence_css(diagram_id: &str, font_size_px: f64) -> String {
    // Mirrors Mermaid@11.12.2 `diagrams/sequence/styles.js` + shared base stylesheet ordering.
    // Keep `:root` last (matches upstream fixtures).
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;
    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:{}px;fill:#333;}}"#,
        id,
        font,
        fmt(font_size_px)
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:#552222;}}#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}#{} .marker.cross{{stroke:#333333;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:{}px;}}#{} p{{margin:0;}}"#,
        id,
        font,
        fmt(font_size_px),
        id
    );

    // Sequence styles.
    let actor_border = "hsl(259.6261682243, 59.7765363128%, 87.9019607843%)";
    let actor_fill = "#ECECFF";
    let note_border = "#aaaa33";
    let note_fill = "#fff5ad";
    let _ = write!(
        &mut out,
        r#"#{} .actor{{stroke:{};fill:{};}}"#,
        id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} text.actor>tspan{{fill:black;stroke:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .actor-line{{stroke:{};}}"#,
        id, actor_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .innerArc{{stroke-width:1.5;stroke-dasharray:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine0{{stroke-width:1.5;stroke-dasharray:none;stroke:#333;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageLine1{{stroke-width:1.5;stroke-dasharray:2,2;stroke:#333;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} [id$="-arrowhead"] path{{fill:#333;stroke:#333;}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .sequenceNumber{{fill:white;}}"#, id);
    let _ = write!(&mut out, r#"#{} [id$="-sequencenumber"]{{fill:#333;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} [id$="-crosshead"] path{{fill:#333;stroke:#333;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .messageText{{fill:#333;stroke:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelBox{{stroke:{};fill:{};}}"#,
        id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .labelText,#{} .labelText>tspan{{fill:black;stroke:none;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .loopText,#{} .loopText>tspan{{fill:black;stroke:none;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .loopLine{{stroke-width:2px;stroke-dasharray:2,2;stroke:{};fill:{};}}"#,
        id, actor_border, actor_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .note{{stroke:{};fill:{};}}"#,
        id, note_border, note_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .noteText,#{} .noteText>tspan{{fill:black;stroke:none;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .activation0{{fill:#f4f4f4;stroke:#666;}}#{} .activation1{{fill:#f4f4f4;stroke:#666;}}#{} .activation2{{fill:#f4f4f4;stroke:#666;}}"#,
        id, id, id
    );
    let _ = write!(&mut out, r#"#{} .actorPopupMenu{{position:absolute;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .actorPopupMenuPanel{{position:absolute;fill:{};box-shadow:0px 8px 16px 0px rgba(0,0,0,0.2);filter:drop-shadow(3px 5px 2px rgb(0 0 0 / 0.4));}}"#,
        id, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .actor-man line{{stroke:{};fill:{};}}"#,
        id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} .actor-man circle,#{} line{{stroke:{};fill:{};stroke-width:2px;}}"#,
        id, id, actor_border, actor_fill
    );
    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_css_uses_configured_font_size() {
        let css = sequence_css("seq", 24.0);

        assert!(css.contains(
            r#"#seq{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;fill:#333;}"#
        ));
        assert!(css.contains(r#"#seq svg{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:24px;}#seq p{margin:0;}"#));
    }
}
