//! Flowchart node style evidence produced by the concrete SVG emitter.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::svg::parity::flowchart) enum FlowchartNodeFacetReach {
    None,
    Unverified,
    Verified,
}

impl FlowchartNodeFacetReach {
    pub(in crate::svg::parity::flowchart) const fn is_verified(self) -> bool {
        matches!(self, Self::Verified)
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartNodeSourceFacetEmission {
    reach: FlowchartNodeFacetReach,
    generated_class_css: bool,
}

impl FlowchartNodeSourceFacetEmission {
    pub(in crate::svg::parity::flowchart) const fn reach(self) -> FlowchartNodeFacetReach {
        self.reach
    }

    pub(in crate::svg::parity::flowchart) const fn generated_class_css(self) -> bool {
        self.generated_class_css
    }
}

#[derive(Debug, Clone, Copy)]
struct FlowchartNodeFacetEmission {
    admitted_source_transport: FlowchartNodeFacetReach,
    raw_source_transport: FlowchartNodeFacetReach,
    generated_selector: FlowchartNodeFacetReach,
    generated_selector_overrides_transport: bool,
    typed_theme: FlowchartNodeFacetReach,
}

impl FlowchartNodeFacetEmission {
    const fn new(
        admitted_source_transport: FlowchartNodeFacetReach,
        raw_source_transport: FlowchartNodeFacetReach,
        generated_selector: FlowchartNodeFacetReach,
        generated_selector_overrides_transport: bool,
        typed_theme: FlowchartNodeFacetReach,
    ) -> Self {
        Self {
            admitted_source_transport,
            raw_source_transport,
            generated_selector,
            generated_selector_overrides_transport,
            typed_theme,
        }
    }

    fn source_emission(
        self,
        admitted: bool,
        inline: bool,
        wrapper_has_class: bool,
    ) -> FlowchartNodeSourceFacetEmission {
        let transport = if admitted {
            self.admitted_source_transport
        } else {
            self.raw_source_transport
        };
        if inline || !wrapper_has_class || self.generated_selector == FlowchartNodeFacetReach::None
        {
            FlowchartNodeSourceFacetEmission {
                reach: transport,
                generated_class_css: false,
            }
        } else if transport == FlowchartNodeFacetReach::None
            || self.generated_selector_overrides_transport
        {
            FlowchartNodeSourceFacetEmission {
                reach: self.generated_selector,
                generated_class_css: true,
            }
        } else {
            FlowchartNodeSourceFacetEmission {
                reach: transport,
                generated_class_css: false,
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartNodeShapeEmissionReceipt {
    fill: FlowchartNodeFacetEmission,
    stroke: FlowchartNodeFacetEmission,
    stroke_width: FlowchartNodeFacetEmission,
    stroke_dasharray: FlowchartNodeFacetEmission,
    radius: FlowchartNodeFacetEmission,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartNodeLabelEmissionReceipt {
    source_typography: FlowchartNodeFacetReach,
    prepared_typography: Option<FlowchartNodeFacetReach>,
    label_fill: Option<FlowchartNodeFacetReach>,
    text_paint_facts: Option<crate::flowchart::FlowchartTextPaintFacts>,
    background_has_area: bool,
    html_font_stack_status: crate::flowchart::FlowchartSourceFacetStatus,
    html_font_size_status: crate::flowchart::FlowchartSourceFacetStatus,
}

impl FlowchartNodeLabelEmissionReceipt {
    const fn facet_reach(applicable: bool, verified: bool) -> Option<FlowchartNodeFacetReach> {
        if applicable {
            Some(if verified {
                FlowchartNodeFacetReach::Verified
            } else {
                FlowchartNodeFacetReach::Unverified
            })
        } else {
            None
        }
    }

    pub(in crate::svg::parity::flowchart) const fn verified() -> Self {
        Self {
            source_typography: FlowchartNodeFacetReach::Verified,
            prepared_typography: None,
            label_fill: None,
            text_paint_facts: None,
            background_has_area: false,
            html_font_stack_status: crate::flowchart::FlowchartSourceFacetStatus::Absent,
            html_font_size_status: crate::flowchart::FlowchartSourceFacetStatus::Absent,
        }
    }

    pub(in crate::svg::parity::flowchart) const fn unverified() -> Self {
        Self {
            source_typography: FlowchartNodeFacetReach::Unverified,
            prepared_typography: None,
            label_fill: None,
            text_paint_facts: None,
            background_has_area: false,
            html_font_stack_status: crate::flowchart::FlowchartSourceFacetStatus::Absent,
            html_font_size_status: crate::flowchart::FlowchartSourceFacetStatus::Absent,
        }
    }

    pub(in crate::svg::parity::flowchart) const fn with_prepared_typography_reach(
        mut self,
        applicable: bool,
        verified: bool,
    ) -> Self {
        self.prepared_typography = Self::facet_reach(applicable, verified);
        self
    }

    pub(in crate::svg::parity::flowchart) const fn with_label_fill_reach(
        mut self,
        applicable: bool,
        verified: bool,
    ) -> Self {
        self.label_fill = Self::facet_reach(applicable, verified);
        self
    }

    pub(in crate::svg::parity::flowchart) fn with_background_area(mut self, area: bool) -> Self {
        self.background_has_area = area;
        self
    }

    pub(in crate::svg::parity::flowchart) fn background_has_area(self) -> bool {
        self.background_has_area
    }

    pub(in crate::svg::parity::flowchart) fn with_text_paint_facts(
        mut self,
        facts: Option<crate::flowchart::FlowchartTextPaintFacts>,
    ) -> Self {
        self.text_paint_facts = facts;
        self
    }

    pub(in crate::svg::parity::flowchart) const fn text_paint_facts(
        self,
    ) -> Option<crate::flowchart::FlowchartTextPaintFacts> {
        self.text_paint_facts
    }

    pub(in crate::svg::parity::flowchart) const fn with_html_typography_statuses(
        mut self,
        font_stack: crate::flowchart::FlowchartSourceFacetStatus,
        font_size: crate::flowchart::FlowchartSourceFacetStatus,
    ) -> Self {
        self.html_font_stack_status = font_stack;
        self.html_font_size_status = font_size;
        self
    }

    pub(in crate::svg::parity::flowchart) const fn typography_verified(self) -> bool {
        self.source_typography.is_verified()
    }

    pub(in crate::svg::parity::flowchart) const fn typography_applicable(self) -> bool {
        self.prepared_typography.is_some()
    }

    pub(in crate::svg::parity::flowchart) const fn prepared_typography_reach(
        self,
    ) -> Option<FlowchartNodeFacetReach> {
        self.prepared_typography
    }

    pub(in crate::svg::parity::flowchart) const fn label_fill_reach(
        self,
    ) -> Option<FlowchartNodeFacetReach> {
        self.label_fill
    }

    pub(in crate::svg::parity::flowchart) const fn html_font_stack_status(
        self,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.html_font_stack_status
    }

    pub(in crate::svg::parity::flowchart) const fn html_font_size_status(
        self,
    ) -> crate::flowchart::FlowchartSourceFacetStatus {
        self.html_font_size_status
    }
}

impl FlowchartNodeShapeEmissionReceipt {
    pub(in crate::svg::parity::flowchart) const fn classic_process(radius_emitted: bool) -> Self {
        let facet = FlowchartNodeFacetEmission::new(
            FlowchartNodeFacetReach::Verified,
            FlowchartNodeFacetReach::Verified,
            FlowchartNodeFacetReach::Verified,
            false,
            FlowchartNodeFacetReach::Verified,
        );
        Self {
            fill: facet,
            stroke: facet,
            stroke_width: facet,
            stroke_dasharray: facet,
            radius: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::Verified,
                FlowchartNodeFacetReach::Verified,
                FlowchartNodeFacetReach::Verified,
                false,
                if radius_emitted {
                    FlowchartNodeFacetReach::Verified
                } else {
                    FlowchartNodeFacetReach::Unverified
                },
            ),
        }
    }

    pub(in crate::svg::parity::flowchart) const fn hand_drawn_process() -> Self {
        let facet = FlowchartNodeFacetEmission::new(
            FlowchartNodeFacetReach::Verified,
            FlowchartNodeFacetReach::None,
            FlowchartNodeFacetReach::Unverified,
            true,
            FlowchartNodeFacetReach::Unverified,
        );
        Self {
            fill: facet,
            stroke: facet,
            stroke_width: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::Unverified,
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::Unverified,
                true,
                FlowchartNodeFacetReach::Unverified,
            ),
            stroke_dasharray: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::Unverified,
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::Unverified,
                true,
                FlowchartNodeFacetReach::Unverified,
            ),
            radius: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::Unverified,
                true,
                FlowchartNodeFacetReach::Unverified,
            ),
        }
    }

    pub(in crate::svg::parity::flowchart) const fn start() -> Self {
        let facet = FlowchartNodeFacetEmission::new(
            FlowchartNodeFacetReach::None,
            FlowchartNodeFacetReach::None,
            FlowchartNodeFacetReach::Verified,
            true,
            FlowchartNodeFacetReach::Verified,
        );
        Self {
            fill: facet,
            stroke: facet,
            stroke_width: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::Unverified,
                true,
                FlowchartNodeFacetReach::Unverified,
            ),
            stroke_dasharray: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::Unverified,
                true,
                FlowchartNodeFacetReach::Unverified,
            ),
            radius: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::None,
                FlowchartNodeFacetReach::Unverified,
                true,
                FlowchartNodeFacetReach::Unverified,
            ),
        }
    }

    pub(in crate::svg::parity::flowchart) const fn common_style_unverified() -> Self {
        let facet = FlowchartNodeFacetEmission::new(
            FlowchartNodeFacetReach::Verified,
            FlowchartNodeFacetReach::Verified,
            FlowchartNodeFacetReach::Unverified,
            false,
            FlowchartNodeFacetReach::Unverified,
        );
        Self {
            fill: facet,
            stroke: facet,
            stroke_width: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::Unverified,
                FlowchartNodeFacetReach::Unverified,
                FlowchartNodeFacetReach::Unverified,
                false,
                FlowchartNodeFacetReach::Unverified,
            ),
            stroke_dasharray: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::Unverified,
                FlowchartNodeFacetReach::Unverified,
                FlowchartNodeFacetReach::Unverified,
                false,
                FlowchartNodeFacetReach::Unverified,
            ),
            radius: FlowchartNodeFacetEmission::new(
                FlowchartNodeFacetReach::Unverified,
                FlowchartNodeFacetReach::Unverified,
                FlowchartNodeFacetReach::Unverified,
                false,
                FlowchartNodeFacetReach::Unverified,
            ),
        }
    }

    pub(in crate::svg::parity::flowchart) const fn unverified() -> Self {
        let facet = FlowchartNodeFacetEmission::new(
            FlowchartNodeFacetReach::Unverified,
            FlowchartNodeFacetReach::Unverified,
            FlowchartNodeFacetReach::Unverified,
            true,
            FlowchartNodeFacetReach::Unverified,
        );
        Self {
            fill: facet,
            stroke: facet,
            stroke_width: facet,
            stroke_dasharray: facet,
            radius: facet,
        }
    }

    pub(in crate::svg::parity::flowchart) fn source_fill_emission(
        self,
        admitted: bool,
        inline: bool,
        wrapper_has_class: bool,
    ) -> FlowchartNodeSourceFacetEmission {
        self.fill
            .source_emission(admitted, inline, wrapper_has_class)
    }

    pub(in crate::svg::parity::flowchart) fn source_stroke_emission(
        self,
        admitted: bool,
        inline: bool,
        wrapper_has_class: bool,
    ) -> FlowchartNodeSourceFacetEmission {
        self.stroke
            .source_emission(admitted, inline, wrapper_has_class)
    }

    pub(in crate::svg::parity::flowchart) fn source_shape_emission(
        self,
        inline: bool,
        wrapper_has_class: bool,
    ) -> FlowchartNodeSourceFacetEmission {
        self.fill.source_emission(false, inline, wrapper_has_class)
    }

    pub(in crate::svg::parity::flowchart) fn source_stroke_width_emission(
        self,
        admitted: bool,
        inline: bool,
        wrapper_has_class: bool,
    ) -> FlowchartNodeSourceFacetEmission {
        self.stroke_width
            .source_emission(admitted, inline, wrapper_has_class)
    }

    pub(in crate::svg::parity::flowchart) fn source_radius_emission(
        self,
        admitted: bool,
        inline: bool,
        wrapper_has_class: bool,
    ) -> FlowchartNodeSourceFacetEmission {
        self.radius
            .source_emission(admitted, inline, wrapper_has_class)
    }

    pub(in crate::svg::parity::flowchart) fn source_stroke_dasharray_emission(
        self,
        admitted: bool,
        inline: bool,
        wrapper_has_class: bool,
    ) -> FlowchartNodeSourceFacetEmission {
        self.stroke_dasharray
            .source_emission(admitted, inline, wrapper_has_class)
    }

    pub(in crate::svg::parity::flowchart) const fn generated_selector_overrides_direct(
        self,
    ) -> bool {
        self.fill.generated_selector_overrides_transport
    }

    pub(in crate::svg::parity::flowchart) const fn stroke_width_selector_overrides_direct(
        self,
    ) -> bool {
        self.stroke_width.generated_selector_overrides_transport
    }

    pub(in crate::svg::parity::flowchart) const fn radius_selector_overrides_direct(self) -> bool {
        self.radius.generated_selector_overrides_transport
    }

    pub(in crate::svg::parity::flowchart) const fn stroke_dasharray_selector_overrides_direct(
        self,
    ) -> bool {
        self.stroke_dasharray.generated_selector_overrides_transport
    }

    pub(in crate::svg::parity::flowchart) const fn typed_fill_verified(self) -> bool {
        self.fill.typed_theme.is_verified()
    }

    pub(in crate::svg::parity::flowchart) const fn typed_stroke_verified(self) -> bool {
        self.stroke.typed_theme.is_verified()
    }

    pub(in crate::svg::parity::flowchart) const fn typed_stroke_width_verified(self) -> bool {
        self.stroke_width.typed_theme.is_verified()
    }

    pub(in crate::svg::parity::flowchart) const fn typed_radius_verified(self) -> bool {
        self.radius.typed_theme.is_verified()
    }

    pub(in crate::svg::parity::flowchart) const fn typed_stroke_dasharray_verified(self) -> bool {
        self.stroke_dasharray.typed_theme.is_verified()
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartNodeShapeRenderOutcome {
    closes_wrapper: bool,
    emission: FlowchartNodeShapeEmissionReceipt,
    label: FlowchartNodeLabelEmissionReceipt,
}

impl FlowchartNodeShapeRenderOutcome {
    pub(in crate::svg::parity::flowchart) const fn new(
        closes_wrapper: bool,
        emission: FlowchartNodeShapeEmissionReceipt,
    ) -> Self {
        Self {
            closes_wrapper,
            emission,
            label: FlowchartNodeLabelEmissionReceipt::unverified(),
        }
    }

    pub(in crate::svg::parity::flowchart) const fn with_label(
        mut self,
        label: FlowchartNodeLabelEmissionReceipt,
    ) -> Self {
        self.label = label;
        self
    }

    pub(in crate::svg::parity::flowchart) const fn closes_wrapper(self) -> bool {
        self.closes_wrapper
    }

    pub(in crate::svg::parity::flowchart) const fn emission(
        self,
    ) -> FlowchartNodeShapeEmissionReceipt {
        self.emission
    }

    pub(in crate::svg::parity::flowchart) const fn label(
        self,
    ) -> FlowchartNodeLabelEmissionReceipt {
        self.label
    }
}
