#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisStatus {
    Ok = 0,
    InvalidArgument = 1,
    Utf8Error = 2,
    OptionsJsonError = 3,
    NoDiagram = 4,
    ParseError = 5,
    RenderError = 6,
    UnsupportedFormat = 7,
    Panic = 8,
    InternalError = 9,
    ResourceLimitExceeded = 10,
}

impl AnalysisStatus {
    pub const fn code(self) -> i32 {
        self as i32
    }

    pub const fn code_name(self) -> &'static str {
        match self {
            Self::Ok => "MERMAN_OK",
            Self::InvalidArgument => "MERMAN_INVALID_ARGUMENT",
            Self::Utf8Error => "MERMAN_UTF8_ERROR",
            Self::OptionsJsonError => "MERMAN_OPTIONS_JSON_ERROR",
            Self::NoDiagram => "MERMAN_NO_DIAGRAM",
            Self::ParseError => "MERMAN_PARSE_ERROR",
            Self::RenderError => "MERMAN_RENDER_ERROR",
            Self::UnsupportedFormat => "MERMAN_UNSUPPORTED_FORMAT",
            Self::Panic => "MERMAN_PANIC",
            Self::InternalError => "MERMAN_INTERNAL_ERROR",
            Self::ResourceLimitExceeded => "MERMAN_RESOURCE_LIMIT_EXCEEDED",
        }
    }
}
