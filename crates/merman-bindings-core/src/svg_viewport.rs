use crate::BindingError;
use quick_xml::Reader;
use quick_xml::XmlVersion;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

/// Logical CSS-pixel viewport of the returned SVG, independent of raster scaling.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BindingSvgViewport {
    pub min_x: f64,
    pub min_y: f64,
    pub width: f64,
    pub height: f64,
}

impl BindingSvgViewport {
    pub(crate) fn validate(self) -> Result<Self, BindingError> {
        if [self.min_x, self.min_y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
            && self.width > 0.0
            && self.height > 0.0
        {
            Ok(self)
        } else {
            Err(BindingError::invalid_argument(
                "invalid SVG viewport dimensions",
            ))
        }
    }
}

pub(crate) fn from_svg(svg: &[u8]) -> Result<BindingSvgViewport, BindingError> {
    // Read only the root emitted by the renderer, not another SVG tree or layout.
    let mut reader = Reader::from_str(std::str::from_utf8(svg).map_err(|_| invalid_root())?);
    loop {
        match reader.read_event() {
            Ok(Event::Start(root) | Event::Empty(root)) if root.local_name().as_ref() == "svg" => {
                for attribute in root.attributes() {
                    let attribute = attribute.map_err(|_| invalid_root())?;
                    if attribute.key.as_ref() == "viewBox" {
                        let value = attribute
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map_err(|_| invalid_root())?;
                        let numbers = value
                            .split(|character: char| {
                                character == ',' || character.is_ascii_whitespace()
                            })
                            .filter(|part| !part.is_empty())
                            .map(str::parse::<f64>)
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|_| invalid_root())?;
                        let [min_x, min_y, width, height] = numbers.as_slice() else {
                            return Err(invalid_root());
                        };
                        return BindingSvgViewport {
                            min_x: *min_x,
                            min_y: *min_y,
                            width: *width,
                            height: *height,
                        }
                        .validate();
                    }
                }
                return Err(invalid_root());
            }
            Ok(Event::Decl(_) | Event::Comment(_) | Event::PI(_)) => {}
            Ok(Event::Text(text))
                if text.as_ref().bytes().all(|byte| byte.is_ascii_whitespace()) => {}
            _ => return Err(invalid_root()),
        }
    }
}

fn invalid_root() -> BindingError {
    BindingError::internal("rendered SVG has no valid root viewBox")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_uses_logical_view_box_not_percentage_width() {
        let viewport =
            from_svg(br#"<svg width="100%" height="90" viewBox="-4 2 120 90"><path/></svg>"#)
                .unwrap();
        assert_eq!(
            (
                viewport.min_x,
                viewport.min_y,
                viewport.width,
                viewport.height
            ),
            (-4.0, 2.0, 120.0, 90.0)
        );
    }

    #[test]
    fn invalid_or_nonfinite_viewports_are_rejected() {
        for svg in [
            r#"<svg viewBox="0 0 NaN 10"/>"#,
            r#"<svg viewBox="0 0 10 0"/>"#,
            r#"<g><svg viewBox="0 0 10 10"/></g>"#,
        ] {
            assert!(from_svg(svg.as_bytes()).is_err());
        }
    }
}
