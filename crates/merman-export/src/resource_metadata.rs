//! Resource limits for native export formats, available without the encoders.

use merman_model::resources::ResourceProfile;

pub const DEFAULT_MAX_RASTER_SIDE_LENGTH: u32 = 4096;
pub const DEFAULT_MAX_RASTER_PIXELS: u64 =
    (DEFAULT_MAX_RASTER_SIDE_LENGTH as u64) * (DEFAULT_MAX_RASTER_SIDE_LENGTH as u64);
/// Aggregate pixels retained as localized PDF filter images before sampling is reduced.
pub const DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;
/// Maximum intrinsic pixels accepted for one embedded raster image by default.
pub const DEFAULT_MAX_DECODED_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;
/// Maximum aggregate intrinsic pixels accepted across embedded raster images by default.
pub const DEFAULT_MAX_TOTAL_DECODED_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;
/// Maximum decoded data-URL bytes accepted for one embedded image before usvg parsing.
pub const DEFAULT_MAX_EMBEDDED_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum aggregate decoded data-URL bytes accepted before usvg parsing.
pub const DEFAULT_MAX_TOTAL_EMBEDDED_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
pub const DEFAULT_MAX_SVG_ISOLATION_DEPTH: usize = 8;
// Two sRGB shadows lower to at most ten standard SVG primitives. Keep this finite backend cap
// aligned with that bounded composition; the separate total-primitive ceiling still applies.
pub const DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER: usize = 10;
pub const DEFAULT_MAX_TOTAL_FILTER_PRIMITIVES: usize = 128;
pub const DEFAULT_MAX_SVG_SUBROOTS: usize = 4096;
pub const DEFAULT_MAX_NESTED_SVG_IMAGES: usize = 64;

pub const MAX_RASTER_WIDTH_RESOURCE_LIMIT_ID: &str = "max_raster_width";
pub const MAX_RASTER_HEIGHT_RESOURCE_LIMIT_ID: &str = "max_raster_height";
pub const MAX_RASTER_PIXELS_RESOURCE_LIMIT_ID: &str = "max_raster_pixels";
pub const MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID: &str = "max_embedded_image_bytes";
pub const MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID: &str = "max_total_embedded_image_bytes";
pub const MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID: &str = "max_embedded_image_pixels";
pub const MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID: &str =
    "max_total_embedded_image_pixels";
pub const MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID: &str =
    "max_svg_conversion_isolation_depth";
pub const MAX_SVG_CONVERSION_FILTER_PRIMITIVES_PER_FILTER_RESOURCE_LIMIT_ID: &str =
    "max_svg_conversion_filter_primitives_per_filter";
pub const MAX_TOTAL_SVG_CONVERSION_FILTER_PRIMITIVES_RESOURCE_LIMIT_ID: &str =
    "max_total_svg_conversion_filter_primitives";
pub const MAX_SVG_CONVERSION_SUBROOTS_RESOURCE_LIMIT_ID: &str = "max_svg_conversion_subroots";
pub const MAX_NESTED_SVG_IMAGES_RESOURCE_LIMIT_ID: &str = "max_nested_svg_images";
pub const MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID: &str = "max_pdf_filter_image_pixels";

/// Stable binding metadata for native export limits owned by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportResourceLimitDescriptor {
    pub stable_id: &'static str,
    pub phase: &'static str,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
    pub minimum_value: usize,
}

/// A view of the resource limits that apply to a set of export formats.
#[derive(Debug, Clone, Copy)]
pub struct ExportResourceLimits {
    output_ids: &'static [&'static str],
}

impl ExportResourceLimits {
    /// Includes every export format, regardless of the enabled encoders.
    pub const ALL: Self = Self {
        output_ids: ALL_OUTPUT_IDS,
    };

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(crate) const COMPILED: Self = Self {
        output_ids: &[
            #[cfg(feature = "png")]
            "png",
            #[cfg(feature = "jpeg")]
            "jpeg",
            #[cfg(feature = "pdf")]
            "pdf",
        ],
    };

    /// Returns the selected limits in resource contract order.
    #[must_use]
    pub fn descriptors(self) -> Vec<ExportResourceLimitDescriptor> {
        self.limits()
            .map(|limit| ExportResourceLimitDescriptor {
                stable_id: limit.stable_id,
                phase: limit.group.phase,
                description: limit.description,
                overridable: !limit.group.hard_cap,
                hard_cap: limit.group.hard_cap,
                minimum_value: 1,
            })
            .collect()
    }

    /// Returns `None` for an unselected or unknown limit and `Some(None)` for a disabled budget.
    #[must_use]
    pub fn profile_value(self, profile: ResourceProfile, stable_id: &str) -> Option<Option<usize>> {
        let limit = self.limit(stable_id)?;
        Some(match profile {
            ResourceProfile::UnboundedForTrustedInput if !limit.group.hard_cap => None,
            ResourceProfile::Interactive
            | ResourceProfile::Constrained
            | ResourceProfile::TrustedNative
            | ResourceProfile::UnboundedForTrustedInput => Some(limit.finite_value),
        })
    }

    /// Returns the selected limit's full list of enforcing formats.
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(crate) fn output_ids(self, stable_id: &str) -> Option<&'static [&'static str]> {
        self.limit(stable_id).map(|limit| limit.group.output_ids)
    }

    fn limit(self, stable_id: &str) -> Option<&'static Limit> {
        self.limits().find(|limit| limit.stable_id == stable_id)
    }

    fn limits(self) -> impl Iterator<Item = &'static Limit> {
        LIMITS.iter().filter(move |limit| {
            limit
                .group
                .output_ids
                .iter()
                .any(|id| self.output_ids.contains(id))
        })
    }
}

#[derive(Clone, Copy)]
struct LimitGroup {
    phase: &'static str,
    output_ids: &'static [&'static str],
    hard_cap: bool,
}

struct Limit {
    stable_id: &'static str,
    description: &'static str,
    finite_value: usize,
    group: LimitGroup,
}

const ALL_OUTPUT_IDS: &[&str] = &["png", "jpeg", "pdf"];
const RASTER_ALLOCATION: LimitGroup = LimitGroup {
    phase: "raster_allocation",
    output_ids: &["png", "jpeg"],
    hard_cap: false,
};
const EMBEDDED_IMAGE_DECODE: LimitGroup = LimitGroup {
    phase: "embedded_image_decode",
    output_ids: ALL_OUTPUT_IDS,
    hard_cap: false,
};
const PDF_FILTER_RASTERIZATION: LimitGroup = LimitGroup {
    phase: "pdf_filter_rasterization",
    output_ids: &["pdf"],
    hard_cap: false,
};
// Backend recursion guards remain active for the unbounded trusted-input profile.
const SVG_CONVERSION: LimitGroup = LimitGroup {
    phase: "svg_conversion",
    output_ids: ALL_OUTPUT_IDS,
    hard_cap: true,
};

const LIMITS: &[Limit] = &[
    Limit {
        stable_id: MAX_RASTER_WIDTH_RESOURCE_LIMIT_ID,
        description: "Maximum final PNG or JPEG width in pixels",
        finite_value: DEFAULT_MAX_RASTER_SIDE_LENGTH as usize,
        group: RASTER_ALLOCATION,
    },
    Limit {
        stable_id: MAX_RASTER_HEIGHT_RESOURCE_LIMIT_ID,
        description: "Maximum final PNG or JPEG height in pixels",
        finite_value: DEFAULT_MAX_RASTER_SIDE_LENGTH as usize,
        group: RASTER_ALLOCATION,
    },
    Limit {
        stable_id: MAX_RASTER_PIXELS_RESOURCE_LIMIT_ID,
        description: "Maximum final PNG or JPEG pixel count",
        finite_value: DEFAULT_MAX_RASTER_PIXELS as usize,
        group: RASTER_ALLOCATION,
    },
    Limit {
        stable_id: MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
        description: "Maximum decoded data-URL bytes for one embedded image",
        finite_value: DEFAULT_MAX_EMBEDDED_IMAGE_BYTES as usize,
        group: EMBEDDED_IMAGE_DECODE,
    },
    Limit {
        stable_id: MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
        description: "Maximum aggregate decoded data-URL bytes across embedded images",
        finite_value: DEFAULT_MAX_TOTAL_EMBEDDED_IMAGE_BYTES as usize,
        group: EMBEDDED_IMAGE_DECODE,
    },
    Limit {
        stable_id: MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
        description: "Maximum intrinsic pixels for one embedded raster image",
        finite_value: DEFAULT_MAX_DECODED_IMAGE_PIXELS as usize,
        group: EMBEDDED_IMAGE_DECODE,
    },
    Limit {
        stable_id: MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
        description: "Maximum aggregate intrinsic pixels across embedded raster images",
        finite_value: DEFAULT_MAX_TOTAL_DECODED_IMAGE_PIXELS as usize,
        group: EMBEDDED_IMAGE_DECODE,
    },
    Limit {
        stable_id: MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
        description: "Maximum aggregate pixels retained for localized PDF filter images",
        finite_value: DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS as usize,
        group: PDF_FILTER_RASTERIZATION,
    },
    Limit {
        stable_id: MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID,
        description: "Maximum nested SVG isolation depth accepted by native export",
        finite_value: DEFAULT_MAX_SVG_ISOLATION_DEPTH,
        group: SVG_CONVERSION,
    },
    Limit {
        stable_id: MAX_SVG_CONVERSION_FILTER_PRIMITIVES_PER_FILTER_RESOURCE_LIMIT_ID,
        description: "Maximum primitives accepted in one SVG filter",
        finite_value: DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER,
        group: SVG_CONVERSION,
    },
    Limit {
        stable_id: MAX_TOTAL_SVG_CONVERSION_FILTER_PRIMITIVES_RESOURCE_LIMIT_ID,
        description: "Maximum aggregate SVG filter primitives accepted by native export",
        finite_value: DEFAULT_MAX_TOTAL_FILTER_PRIMITIVES,
        group: SVG_CONVERSION,
    },
    Limit {
        stable_id: MAX_SVG_CONVERSION_SUBROOTS_RESOURCE_LIMIT_ID,
        description: "Maximum resolved SVG subroots accepted by native export",
        finite_value: DEFAULT_MAX_SVG_SUBROOTS,
        group: SVG_CONVERSION,
    },
    Limit {
        stable_id: MAX_NESTED_SVG_IMAGES_RESOURCE_LIMIT_ID,
        description: "Maximum nested SVG images accepted by native export",
        finite_value: DEFAULT_MAX_NESTED_SVG_IMAGES,
        group: SVG_CONVERSION,
    },
];
