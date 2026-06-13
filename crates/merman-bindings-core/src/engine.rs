use crate::{BindingError, common};

#[derive(Clone)]
pub struct BindingEngine {
    #[cfg(feature = "render")]
    render: crate::render::CachedRenderEngine,
    #[cfg(feature = "ascii")]
    ascii: crate::ascii::CachedAsciiEngine,
}

impl BindingEngine {
    pub fn new(options_json: &[u8]) -> Result<Self, BindingError> {
        #[cfg(any(feature = "render", feature = "ascii"))]
        {
            let options = common::parse_options(options_json)?;
            Ok(Self {
                #[cfg(feature = "render")]
                render: crate::render::CachedRenderEngine::new(&options)?,
                #[cfg(feature = "ascii")]
                ascii: crate::ascii::CachedAsciiEngine::new(&options)?,
            })
        }

        #[cfg(not(any(feature = "render", feature = "ascii")))]
        {
            let _ = options_json;
            Ok(Self {})
        }
    }

    pub fn render_svg(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "render")]
        {
            self.render.render_svg(source)
        }

        #[cfg(not(feature = "render"))]
        {
            let _ = source;
            Err(common::feature_required_error("SVG rendering", "render"))
        }
    }

    pub fn render_ascii(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "ascii")]
        {
            self.ascii.render_ascii(source)
        }

        #[cfg(not(feature = "ascii"))]
        {
            let _ = source;
            Err(common::feature_required_error("ASCII rendering", "ascii"))
        }
    }

    pub fn parse_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "render")]
        {
            self.render.parse_json(source)
        }

        #[cfg(not(feature = "render"))]
        {
            let _ = source;
            Err(common::feature_required_error("parse_json", "render"))
        }
    }

    pub fn layout_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "render")]
        {
            self.render.layout_json(source)
        }

        #[cfg(not(feature = "render"))]
        {
            let _ = source;
            Err(common::feature_required_error("layout_json", "render"))
        }
    }

    pub fn validate_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        #[cfg(feature = "render")]
        {
            self.render.validate_json(source)
        }

        #[cfg(not(feature = "render"))]
        {
            let _ = source;
            common::validation_payload_json(Err(common::feature_required_error(
                "validation",
                "render",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::Arc;

    #[test]
    fn engine_reuses_options_for_rendering() {
        let engine = BindingEngine::new(
            br#"{
                "layout": { "text_measurer": "deterministic" },
                "svg": { "diagram_id": "cached engine", "pipeline": "readable" }
            }"#,
        )
        .unwrap();

        let svg = engine.render_svg(b"flowchart TD\nA[Hello]");
        if cfg!(feature = "render") {
            let svg = String::from_utf8(svg.unwrap()).unwrap();
            assert!(svg.contains("id=\"cached-engine\""));
            assert!(svg.contains("data-merman-foreignobject"));
        } else {
            assert_eq!(
                svg.unwrap_err().status(),
                crate::BindingStatus::UnsupportedFormat
            );
        }
    }

    #[test]
    fn engine_validates_with_cached_renderer() {
        let engine = BindingEngine::new(b"").unwrap();
        let validation: Value =
            serde_json::from_slice(&engine.validate_json(b"").unwrap()).unwrap();

        if cfg!(feature = "render") {
            assert_eq!(validation["valid"], false);
            assert_eq!(validation["code_name"], "MERMAN_NO_DIAGRAM");
        } else {
            assert_eq!(validation["valid"], false);
            assert_eq!(validation["code_name"], "MERMAN_UNSUPPORTED_FORMAT");
        }
    }

    #[test]
    fn engine_can_render_concurrently() {
        let engine = Arc::new(BindingEngine::new(b"").unwrap());
        let mut handles = Vec::new();

        for _ in 0..8 {
            let engine = Arc::clone(&engine);
            handles.push(std::thread::spawn(move || {
                for _ in 0..8 {
                    let svg = engine.render_svg(b"flowchart TD\nA[Hello] --> B[World]");
                    if cfg!(feature = "render") {
                        let svg = String::from_utf8(svg.unwrap()).unwrap();
                        assert!(svg.contains("<svg"));
                    } else {
                        let err = svg.unwrap_err();
                        assert_eq!(err.status(), crate::BindingStatus::UnsupportedFormat);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
