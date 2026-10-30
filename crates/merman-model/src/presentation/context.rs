//! Family-specific output declarations paired with a full model.

/// Declared output facts for one family, without parser construction state.
///
/// The default uses the model's ordinary output shape. A checked presentation diagram validates
/// the explicit variant against its model. These declarations do not certify source history.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub enum PresentationContext {
    #[default]
    None,
    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    Flowchart(crate::families::flowchart::FlowchartRenderContext),
    #[cfg(feature = "diagram-class")]
    Class(crate::families::class::ClassStylePrecedenceFacts),
    #[cfg(feature = "diagram-mindmap")]
    Mindmap { diagram_id: String },
    #[cfg(feature = "diagram-info")]
    Info(crate::families::info::InfoOutputData),
    #[cfg(feature = "diagram-pie")]
    Pie(crate::families::pie::PieOutputData),
    #[cfg(feature = "diagram-journey")]
    Journey(crate::families::journey::JourneyOutputData),
    #[cfg(feature = "diagram-timeline")]
    Timeline(crate::families::timeline::TimelineOutputData),
    #[cfg(feature = "diagram-packet")]
    Packet(crate::families::packet::PacketOutputData),
    #[cfg(feature = "diagram-radar")]
    Radar(crate::families::radar::RadarOutputData),
}

impl PresentationContext {
    /// Returns the model kind required by explicit family facts.
    pub fn model_kind(&self) -> Option<&'static str> {
        match self {
            Self::None => None,
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            ))]
            Self::Flowchart(_) => Some("flowchart"),
            #[cfg(feature = "diagram-class")]
            Self::Class(_) => Some("class"),
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap { .. } => Some("mindmap"),
            #[cfg(feature = "diagram-info")]
            Self::Info(_) => Some("info"),
            #[cfg(feature = "diagram-pie")]
            Self::Pie(_) => Some("pie"),
            #[cfg(feature = "diagram-journey")]
            Self::Journey(_) => Some("journey"),
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(_) => Some("timeline"),
            #[cfg(feature = "diagram-packet")]
            Self::Packet(_) => Some("packet"),
            #[cfg(feature = "diagram-radar")]
            Self::Radar(_) => Some("radar"),
        }
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub fn flowchart_render_context(
        &self,
    ) -> Option<&crate::families::flowchart::FlowchartRenderContext> {
        match self {
            Self::Flowchart(context) => Some(context),
            _ => None,
        }
    }

    #[cfg(feature = "diagram-class")]
    pub fn class_style_precedence_facts(
        &self,
    ) -> Option<&crate::families::class::ClassStylePrecedenceFacts> {
        match self {
            Self::Class(facts) => Some(facts),
            _ => None,
        }
    }

    #[cfg(feature = "diagram-mindmap")]
    pub fn mindmap_diagram_id(&self) -> Option<&str> {
        match self {
            Self::Mindmap { diagram_id } => Some(diagram_id),
            _ => None,
        }
    }

    /// Counts the same retained authored family text as the source producer's former context.
    ///
    /// A frozen Mindmap operation identity remains output metadata; freezing it does not add
    /// that metadata to the existing authored-model text budget.
    pub fn retained_text_bytes(&self) -> usize {
        match self {
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            ))]
            Self::Flowchart(context) => context.retained_bytes(),
            #[cfg(feature = "diagram-class")]
            Self::Class(facts) => facts.retained_bytes(),
            _ => 0,
        }
    }
}

macro_rules! family_output_accessors {
    ($(($feature:literal, $method:ident, $variant:ident, $family:ident, $output:ident)),+ $(,)?) => {
        impl PresentationContext {
            $(
                #[cfg(feature = $feature)]
                pub fn $method(&self) -> crate::families::$family::$output {
                    match self {
                        Self::$variant(output) => *output,
                        _ => crate::families::$family::$output::default(),
                    }
                }
            )+
        }
    };
}

family_output_accessors! {
    ("diagram-info", info_output_data, Info, info, InfoOutputData),
    ("diagram-pie", pie_output_data, Pie, pie, PieOutputData),
    ("diagram-journey", journey_output_data, Journey, journey, JourneyOutputData),
    ("diagram-timeline", timeline_output_data, Timeline, timeline, TimelineOutputData),
    ("diagram-packet", packet_output_data, Packet, packet, PacketOutputData),
    ("diagram-radar", radar_output_data, Radar, radar, RadarOutputData),
}
