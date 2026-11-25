use criterion::{Criterion, criterion_group, criterion_main};
use std::env::{self, VarError};

#[cfg(feature = "ascii")]
#[path = "pipeline/ascii.rs"]
mod ascii;

#[cfg(all(
    feature = "svg",
    feature = "compat-json",
    feature = "editor-facts",
    feature = "layout-dagre"
))]
#[path = "pipeline/stages.rs"]
mod stages;

#[cfg(all(
    feature = "png",
    feature = "pdf",
    feature = "native-fonts",
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "diagram-xychart",
    feature = "layout-dagre"
))]
#[path = "pipeline/export.rs"]
mod export;

#[cfg(all(
    feature = "svg",
    feature = "diagram-flowchart",
    feature = "compat-json",
    feature = "layout-dagre"
))]
#[path = "pipeline/flowchart.rs"]
mod flowchart;

#[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
#[path = "pipeline/architecture.rs"]
mod architecture;

#[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
#[path = "pipeline/architecture_layout.rs"]
mod architecture_layout;

#[cfg(all(feature = "diagram-mindmap", feature = "layout-cytoscape"))]
#[path = "pipeline/mindmap.rs"]
mod mindmap;

#[cfg(feature = "svg")]
#[path = "pipeline/text.rs"]
mod text;

#[cfg(all(
    feature = "native-fonts",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
#[path = "pipeline/fonts.rs"]
mod fonts;

#[cfg(any(
    feature = "ascii",
    all(
        feature = "svg",
        feature = "compat-json",
        feature = "editor-facts",
        feature = "layout-dagre"
    )
))]
#[path = "pipeline/identity.rs"]
mod identity;

type Register = fn(&mut Criterion);

const LANES: &[(&str, Register)] = &[
    #[cfg(feature = "ascii")]
    ("ascii", ascii::register),
    #[cfg(all(
        feature = "svg",
        feature = "compat-json",
        feature = "editor-facts",
        feature = "layout-dagre"
    ))]
    ("native", stages::register),
    #[cfg(all(
        feature = "png",
        feature = "pdf",
        feature = "native-fonts",
        feature = "diagram-flowchart",
        feature = "diagram-sequence",
        feature = "diagram-xychart",
        feature = "layout-dagre"
    ))]
    ("export", export::register),
    #[cfg(all(
        feature = "svg",
        feature = "diagram-flowchart",
        feature = "compat-json",
        feature = "layout-dagre"
    ))]
    ("flowchart-stress", flowchart::register),
    #[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
    ("architecture-stress", architecture::register),
    #[cfg(all(feature = "diagram-architecture", feature = "layout-cytoscape"))]
    ("architecture-layout-stress", architecture_layout::register),
    #[cfg(all(feature = "diagram-mindmap", feature = "layout-cytoscape"))]
    ("mindmap-layout-stress", mindmap::register),
    #[cfg(feature = "svg")]
    ("text-stress", text::register),
    #[cfg(all(
        feature = "native-fonts",
        feature = "diagram-flowchart",
        feature = "layout-dagre"
    ))]
    ("fonts", fonts::register),
];

fn register(c: &mut Criterion) {
    // Select a lane before registering any fixtures or running untimed probes.
    match env::var("MERMAN_BENCH_PIPELINE_LANE") {
        Ok(lane) => {
            let (_, register) = LANES
                .iter()
                .find(|(name, _)| *name == lane)
                .unwrap_or_else(|| panic!("unavailable Criterion pipeline lane {lane:?}"));
            register(c);
        }
        Err(VarError::NotPresent) => {
            for (_, register) in LANES {
                register(c);
            }
        }
        Err(error) => panic!("invalid MERMAN_BENCH_PIPELINE_LANE: {error}"),
    }
}

criterion_group!(benches, register);
criterion_main!(benches);
