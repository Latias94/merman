//! Resolve the marker's source declarations before publishing its portable paint.

use super::{
    Color, Error, GanttBuilder, OperationPhase, PortableStyleResolver, Result, unavailable,
};
use cssparser::{Delimiter, ParseError, ParseErrorKind, Parser, ParserInput, Token};

impl GanttBuilder<'_> {
    pub(super) fn resolve_today_marker(&self, marker: &str) -> Result<(Color, f64, f64)> {
        // Mermaid replaces every comma before its final placeholder cleanup. In particular,
        // numeric entities that later expand to commas must not become declaration separators.
        self.session
            .work_meter()
            .charge_at(marker.len(), OperationPhase::Emit)?;
        let mut separators = String::new();
        separators.try_reserve_exact(marker.len()).map_err(|_| {
            Error::DrawingListAllocationFailed {
                collection: "Gantt today marker declarations",
            }
        })?;
        for part in marker.split_inclusive(',') {
            self.session.checkpoint(OperationPhase::Emit)?;
            if let Some(prefix) = part.strip_suffix(',') {
                separators.push_str(prefix);
                separators.push(';');
            } else {
                separators.push_str(part);
            }
        }
        let normalized = self.document.resolve_mermaid_layout_text(&separators)?;
        let mut input = ParserInput::new(&normalized);
        let mut parser = Parser::new(&mut input);
        let mut color = (self.today_fill, false);
        let mut width = (2.0, false);
        let mut opacity = (1.0, false);
        let resolver = PortableStyleResolver::new("gantt");
        while !parser.is_exhausted() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let declaration = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
                let property = declaration.expect_ident_cloned()?;
                declaration.expect_colon()?;
                let value = declaration.parse_until_before(Delimiter::Bang, |value| {
                    let start = value.position();
                    while value.next_including_whitespace().is_ok() {
                        self.session
                            .checkpoint(OperationPhase::Emit)
                            .map_err(|error| value.new_custom_error(error))?;
                    }
                    Ok(value.slice_from(start).trim())
                })?;
                let important = declaration.try_parse(cssparser::parse_important).is_ok();
                declaration.expect_exhausted()?;
                Ok::<_, ParseError<'_, Error>>((property, value, important))
            });
            let (property, value, important) = match declaration {
                Ok(declaration) => declaration,
                Err(error) => match error.kind {
                    ParseErrorKind::Basic(_) => continue,
                    ParseErrorKind::Custom(error) => return Err(error),
                },
            };
            match property.to_ascii_lowercase().as_str() {
                "stroke" => {
                    if provably_invalid_stroke(value) {
                        continue;
                    }
                    if important || !color.1 {
                        color = (resolver.color("todayMarker.stroke", value)?, important);
                    }
                }
                "stroke-width" if important || !width.1 => {
                    width = (
                        resolver.positive_length("todayMarker.stroke-width", value)?,
                        important,
                    );
                }
                "opacity" if important || !opacity.1 => {
                    opacity = (resolver.opacity("todayMarker.opacity", value)?, important);
                }
                "stroke-width" | "opacity" => {}
                "fill" if value.eq_ignore_ascii_case("none") => {}
                _ => {
                    return Err(unavailable(format!(
                        "Gantt todayMarker property `{property}` is not portable"
                    )));
                }
            }
        }
        Ok((color.0, width.0, opacity.0))
    }
}

/// Ignore only syntax that cannot be a CSS paint. A failed portable color parse alone does
/// not prove invalid CSS: variables, system colors, and modern color functions must fail closed.
fn provably_invalid_stroke(value: &str) -> bool {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    match parser.next() {
        Ok(Token::Delim('&')) => true,
        Ok(Token::Function(name))
            if ["rgb", "rgba", "hsl", "hsla"]
                .iter()
                .any(|known| name.eq_ignore_ascii_case(known)) =>
        {
            parser
                .parse_nested_block(|body| {
                    let mut invalid = false;
                    while let Ok(token) = body.next() {
                        if matches!(token, Token::Semicolon) {
                            invalid = true;
                        }
                    }
                    Ok::<_, ParseError<'_, ()>>(invalid)
                })
                .unwrap_or(false)
        }
        _ => false,
    }
}
