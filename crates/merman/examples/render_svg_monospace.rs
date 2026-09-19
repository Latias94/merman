//! Renders Mermaid with measurements from an installed monospace font.
//!
//! The example selects a system monospace face, shapes every candidate string with Rustybuzz, and
//! emits the same family in the SVG. The SVG consumer must have that font installed too.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use merman::svg::{
    DeterministicTextMeasurer, MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity, TextStyle,
};
use merman::{
    Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment,
    SvgRequest,
};

const SOURCE: &str = r#"%%{init: {"flowchart": {"wrappingWidth": 220}}}%%
flowchart TD
    A["Receive a new deployment request from the automated release pipeline"] --> B{"Did every required validation and security check pass successfully?"}
    B -->|Yes| C["Publish the approved application version to the production environment"]
    B -->|No| D["Return the detailed validation errors to the requesting developer"]
"#;

struct ShapedTextWidth {
    font_data: Arc<[u8]>,
    face_index: u32,
    cache: Mutex<HashMap<String, f64>>,
}

impl ShapedTextWidth {
    fn new(font_data: Arc<[u8]>, face_index: u32) -> Result<Self, &'static str> {
        let face = rustybuzz::Face::from_slice(&font_data, face_index)
            .ok_or("failed to parse selected font")?;
        if face.units_per_em() == 0 {
            return Err("selected font has zero units per em");
        }
        Ok(Self {
            font_data,
            face_index,
            cache: Mutex::new(HashMap::new()),
        })
    }

    fn measure(&self, text: &str, style: &TextStyle) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(width) = cache.get(text) {
            return *width * style.font_size.max(1.0);
        }

        let advance_em = self.shape_advance_em(text);
        cache.insert(text.to_string(), advance_em);
        advance_em * style.font_size.max(1.0)
    }

    fn shape_advance_em(&self, text: &str) -> f64 {
        let face = rustybuzz::Face::from_slice(&self.font_data, self.face_index)
            .expect("font face was validated during construction");
        let units_per_em = face.units_per_em();
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        let shaped = rustybuzz::shape(&face, &[], buffer);
        let advance = shaped
            .glyph_positions()
            .iter()
            .map(|glyph| i64::from(glyph.x_advance))
            .sum::<i64>();
        advance as f64 / f64::from(units_per_em)
    }
}

fn installed_monospace_font() -> Result<(String, String, Arc<[u8]>, u32), Box<dyn std::error::Error>>
{
    let mut fonts = usvg::fontdb::Database::new();
    fonts.load_system_fonts();
    let face = fonts
        .faces()
        .find(|face| {
            face.monospaced
                && face.weight == usvg::fontdb::Weight::NORMAL
                && face.style == usvg::fontdb::Style::Normal
        })
        .or_else(|| fonts.faces().find(|face| face.monospaced))
        .ok_or("no installed monospace font found")?;
    let face_id = face.id;
    let family = face
        .families
        .first()
        .map(|(family, _)| family.clone())
        .ok_or("selected font has no family name")?;
    let post_script_name = face.post_script_name.clone();
    let (font_data, face_index) = fonts
        .with_face_data(face_id, |data, index| (Arc::from(data), index))
        .ok_or("failed to load selected font data")?;
    Ok((family, post_script_name, font_data, face_index))
}

fn quote_css_family(family: &str) -> String {
    let escaped = family.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (family, post_script_name, font_data, face_index) = installed_monospace_font()?;
    eprintln!("measuring and rendering with {family} ({post_script_name})");

    let width = Arc::new(ShapedTextWidth::new(font_data, face_index)?);
    let measurer = DeterministicTextMeasurer::default()
        .with_width_callback(move |text, style| width.measure(text, style));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("example.installed-monospace")?,
        format!("1:{post_script_name}"),
    )?;
    let policy = TextMeasurementPolicy::uniform(TextMeasurementProfile::new(identity, measurer));

    let site_config = MermaidConfig::from_value(serde_json::json!({
        "theme": "base",
        "fontFamily": format!("{}, monospace", quote_css_family(&family)),
        "themeVariables": {
            "primaryColor": "#e0f2fe",
            "primaryBorderColor": "#0284c7",
            "primaryTextColor": "#0f172a",
            "lineColor": "#16a34a"
        }
    }));
    let renderer = Renderer::new()
        .with_engine(Engine::new().with_site_config(site_config))
        .with_parse_options(merman::ParseOptions::strict());
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic().with_text_measurement_policy(policy),
        ..Default::default()
    };
    let output = renderer.render(RenderRequest::svg(SOURCE, OperationControl::new(), request))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };
    print!("{}", svg.svg());
    Ok(())
}
