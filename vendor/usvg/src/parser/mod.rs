// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod clippath;
mod converter;
mod filter;
mod image;
mod marker;
mod mask;
mod options;
mod paint_server;
mod shapes;
mod style;
mod svgtree;
mod switch;
mod units;
mod use_node;

#[cfg(feature = "text")]
mod text;
#[cfg(feature = "text")]
pub use image::ImageHrefPolicy;
pub use image::{ImageHrefDataResolverFn, ImageHrefResolver, ImageHrefStringResolverFn};
pub use options::{DEFAULT_MAX_DEPTH, Options};
#[cfg(feature = "writer")]
pub(crate) use svgtree::{AId, EId};

/// List of all errors.
#[derive(Debug)]
pub enum Error {
    /// Only UTF-8 content are supported.
    NotAnUtf8Str,

    /// `svgz` feature is required to parse SVGZ data.
    SvgzFeatureNotEnabled,

    /// Compressed SVG must use the GZip algorithm.
    MalformedGZip,

    /// We do not allow SVG with more than 1_000_000 elements for security reasons.
    ElementsLimitReached,

    /// SVG doesn't have a valid size.
    ///
    /// Occurs when width and/or height are <= 0.
    ///
    /// Also occurs if width, height and viewBox are not set.
    InvalidSize,

    /// Failed to parse an SVG data.
    ParsingFailed(roxmltree::Error),
}

impl From<roxmltree::Error> for Error {
    fn from(e: roxmltree::Error) -> Self {
        Error::ParsingFailed(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::NotAnUtf8Str => {
                write!(f, "provided data has not an UTF-8 encoding")
            }
            Self::SvgzFeatureNotEnabled => {
                write!(f, "enable svgz cargo feature to decode SVGZ data")
            }
            Error::MalformedGZip => {
                write!(f, "provided data has a malformed GZip content")
            }
            Error::ElementsLimitReached => {
                write!(f, "the maximum number of SVG elements has been reached")
            }
            Error::InvalidSize => {
                write!(f, "SVG has an invalid size")
            }
            Error::ParsingFailed(ref e) => {
                write!(f, "SVG data parsing failed cause {}", e)
            }
        }
    }
}

impl std::error::Error for Error {}

pub(crate) trait OptionLog {
    fn log_none<F: FnOnce()>(self, f: F) -> Self;
}

impl<T> OptionLog for Option<T> {
    #[inline]
    fn log_none<F: FnOnce()>(self, f: F) -> Self {
        self.or_else(|| {
            f();
            None
        })
    }
}

impl crate::Tree {
    /// Parses `Tree` from an SVG data.
    ///
    /// Can contain an SVG string or a gzip compressed data.
    pub fn from_data(data: &[u8], opt: &Options) -> Result<Self, Error> {
        Self::parse_data_with(data, |text| Self::from_str(text, opt))
    }

    fn parse_data_with(
        data: &[u8],
        parse: impl FnOnce(&str) -> Result<Self, Error>,
    ) -> Result<Self, Error> {
        if data.starts_with(&[0x1f, 0x8b]) {
            #[cfg(feature = "svgz")]
            {
                let data = decompress_svgz(data)?;
                let text = std::str::from_utf8(&data).map_err(|_| Error::NotAnUtf8Str)?;
                parse(text)
            }

            #[cfg(not(feature = "svgz"))]
            Err(Error::SvgzFeatureNotEnabled)
        } else {
            let text = std::str::from_utf8(data).map_err(|_| Error::NotAnUtf8Str)?;
            parse(text)
        }
    }

    /// Similar to the `from_data` method, except that it ignores all `image` elements linking to
    /// external files, as required by the SVG specification when SVG files are loaded
    /// for `<image href="..." />` tags.
    pub fn from_data_nested(data: &[u8], opt: &Options) -> Result<Self, Error> {
        Self::from_data(data, &Self::nested_options(opt))
    }

    fn nested_options<'a>(opt: &'a Options) -> Options<'a> {
        Options {
            resources_dir: None,
            dpi: opt.dpi,
            font_family: opt.font_family.clone(),
            font_size: opt.font_size,
            languages: opt.languages.clone(),
            shape_rendering: opt.shape_rendering,
            text_rendering: opt.text_rendering,
            image_rendering: opt.image_rendering,
            default_size: opt.default_size,
            image_href_resolver: ImageHrefResolver {
                resolve_data: Box::new(|a, b, c| (opt.image_href_resolver.resolve_data)(a, b, c)),
                // External images should be ignored.
                resolve_string: Box::new(|_, _| None),
            },
            // In the referenced SVG, we start with the unmodified user-provided
            // fontdb, not the one from the cache.
            #[cfg(feature = "text")]
            fontdb: opt.fontdb.clone(),
            // Can't clone the resolver, so we create a new one that forwards to it.
            #[cfg(feature = "text")]
            font_resolver: crate::FontResolver {
                select_font: Box::new(|font, db| (opt.font_resolver.select_font)(font, db)),
                select_fallback: Box::new(|c, used_fonts, db| {
                    (opt.font_resolver.select_fallback)(c, used_fonts, db)
                }),
            },
            #[cfg(feature = "text")]
            text_fallback: crate::TextFallback {
                language: opt.text_fallback.language.clone(),
                select: opt
                    .text_fallback
                    .select
                    .as_ref()
                    .map(|select| Box::new(select) as crate::ClusterFallbackFn<'_>),
                select_instance: opt
                    .text_fallback
                    .select_instance
                    .as_ref()
                    .map(|select| Box::new(select) as crate::FontInstanceSelectionFn<'_>),
                fallback_instance: opt
                    .text_fallback
                    .fallback_instance
                    .as_ref()
                    .map(|select| Box::new(select) as crate::FontInstanceFallbackFn<'_>),
            },
            max_depth: opt.max_depth,
            ..Options::default()
        }
    }

    /// Parses `Tree` from an SVG string.
    pub fn from_str(text: &str, opt: &Options) -> Result<Self, Error> {
        let doc = parse_document(text, opt)?;
        self::converter::convert_doc(&doc, opt)
    }

    /// Parses an SVG once and configures fonts at the first nonempty text span selected
    /// for conversion, after reference expansion and whitespace trimming.
    ///
    /// Only the font database, default font family, and text resolvers supplied by `configure`
    /// are used. If no text needs fonts, `configure` is not called. Returning `false` declines
    /// conversion and the method returns `Ok(None)` without publishing a partial tree.
    /// Built-in SVG images share this configuration. With [`ImageHrefPolicy::Configured`],
    /// each custom resolver supplies its finished image and controls any nested parsing.
    #[cfg(feature = "text")]
    pub fn try_from_str_with_text_fonts(
        text: &str,
        opt: &Options,
        images: ImageHrefPolicy,
        configure: impl FnOnce(&mut Options<'static>) -> bool,
    ) -> Result<Option<Self>, Error> {
        let configure = std::cell::Cell::new(Some(configure));
        let initialize = || {
            let mut options = Options::default();
            configure.take()?(&mut options).then_some(options)
        };
        let session = converter::TextFontSession::new(&initialize);
        let tree = Self::from_str_with_session(text, opt, &session, images)?;
        Ok((!session.declined()).then_some(tree))
    }

    #[cfg(feature = "text")]
    fn from_str_with_session(
        text: &str,
        opt: &Options,
        session: &converter::TextFontSession,
        images: ImageHrefPolicy,
    ) -> Result<Self, Error> {
        let doc = parse_document(text, opt)?;
        let mut cache = converter::Cache::new(&doc, opt.fontdb.clone());
        cache.text_font_session = Some((session, images));
        converter::convert_doc_with_cache(&mut cache, opt)
    }

    /// Parses a metric probe and also resolves the metric bbox x of its end-anchored twin.
    ///
    /// The targeted text must be a single horizontal, start-anchored chunk without `textLength`.
    /// Returns `Ok(None)` for unsupported text. The second result is absent if the end anchor
    /// would be pruned. The normal tree contains only the input text at its original anchor.
    #[cfg(feature = "text")]
    pub fn try_from_str_with_text_anchor_probe(
        source: impl FnOnce() -> String,
        opt: &Options,
        id: &str,
    ) -> Result<Option<(Self, Option<f32>)>, Error> {
        let source = source();
        let doc = parse_document(&source, opt)?;
        let mut cache = converter::Cache::new(&doc, opt.fontdb.clone());
        cache.text_anchor_probe = Some(converter::TextAnchorProbe {
            id: id.into(),
            supported: true,
            end_x: None,
        });
        let tree = converter::convert_doc_with_cache(&mut cache, opt)?;
        Ok(cache
            .text_anchor_probe
            .and_then(|probe| probe.supported.then_some((tree, probe.end_x))))
    }

    /// Parses `Tree` from `roxmltree::Document`.
    pub fn from_xmltree(doc: &roxmltree::Document, opt: &Options) -> Result<Self, Error> {
        let doc = svgtree::Document::parse_tree(doc, opt.style_sheet.as_deref(), opt.max_depth)?;
        self::converter::convert_doc(&doc, opt)
    }
}

fn parse_document<'input>(
    text: &'input str,
    opt: &'input Options,
) -> Result<svgtree::Document<'input>, Error> {
    let xml_opt = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    // Drop the XML document before converting the simplified SVG tree.
    let xml = roxmltree::Document::parse_with_options(text, xml_opt)?;
    Ok(svgtree::Document::parse_tree(
        &xml,
        opt.style_sheet.as_deref(),
        opt.max_depth,
    )?)
}

/// Decompresses an SVGZ file.
#[cfg(feature = "svgz")]
pub fn decompress_svgz(data: &[u8]) -> Result<Vec<u8>, Error> {
    use std::io::Read;

    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut decoded = Vec::with_capacity(data.len() * 2);
    decoder
        .read_to_end(&mut decoded)
        .map_err(|_| Error::MalformedGZip)?;
    Ok(decoded)
}

#[inline]
pub(crate) fn f32_bound(min: f32, val: f32, max: f32) -> f32 {
    debug_assert!(min.is_finite());
    debug_assert!(max.is_finite());

    if val > max {
        max
    } else if val >= min {
        val
    } else {
        // Catches `val < min` as well as a NaN `val`.
        min
    }
}
