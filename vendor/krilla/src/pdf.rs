//! Including other PDF files.

use std::collections::HashMap;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use hayro_write::hayro_syntax::page::Page;
use hayro_write::hayro_syntax::PdfVersion as HayroPdfVersion;
use hayro_write::{ExtractionError, ExtractionQuery};
use pdf_writer::{Name, Ref};

pub use hayro_write::hayro_syntax::Pdf;

use crate::chunk_container::{ChunkContainer, EmbeddedPdfChunk};
use crate::configure::{PdfVersion, ValidationError};
use crate::error::{KrillaError, KrillaResult};
use crate::graphics::color::rgb;
use crate::resource::Resource;
use crate::serialize::{MaybeDeviceColorSpace, SerializeContext};
use crate::surface::Location;
use crate::util::{Deferred, Prehashed};

/// An error that can occur when embedding a PDF document.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PdfError {
    /// A page was requested that doesn't exist.
    InvalidPage(usize),
    /// The PDF version of the embedded PDF is not compatible with the version of the PDF
    /// produced by krilla. This happens if the version of the krilla document is lower than
    /// the one of the embedded PDF.
    ///
    /// The argument indicates the version of the embedded PDF document.
    VersionMismatch(PdfVersion),
}

impl Display for PdfError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PdfError::InvalidPage(index) => write!(f, "invalid page index {index}"),
            PdfError::VersionMismatch(version) => write!(
                f,
                "embedded PDF requires {}, which is newer than the selected PDF version",
                version.as_str()
            ),
        }
    }
}

impl std::error::Error for PdfError {}

struct PdfDocumentRepr(Arc<Pdf>);

impl Debug for PdfDocumentRepr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "PdfDocumentRepr {{ .. }}")
    }
}

impl Hash for PdfDocumentRepr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.data().as_ref().as_ref().hash(state);
    }
}

/// An external PDF document.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct PdfDocument(Arc<Prehashed<PdfDocumentRepr>>);

impl PdfDocument {
    /// Load a new PDF document.
    pub fn new(pdf: Arc<Pdf>) -> PdfDocument {
        Self(Arc::new(Prehashed::new(PdfDocumentRepr(pdf))))
    }

    pub(crate) fn pdf(&self) -> &Pdf {
        &self.0.deref().0
    }

    pub(crate) fn pages(&self) -> &[Page<'_>] {
        self.0.deref().0.pages()
    }
}

#[derive(Default, Debug)]
pub(crate) struct PdfDocumentInfo {
    counter: u64,
    query_refs: Vec<Ref>,
    cached_xobjects: HashMap<usize, Ref>,
    queries: Vec<ExtractionQuery>,
    locations: Vec<Option<Location>>,
}

impl PdfDocumentInfo {
    pub(crate) fn new(counter: u64) -> Self {
        Self {
            counter,
            ..Self::default()
        }
    }
}

#[derive(Default, Debug)]
pub(crate) struct PdfSerializerContext {
    infos: HashMap<PdfDocument, PdfDocumentInfo>,
    counter: u64,
}

impl PdfSerializerContext {
    pub(crate) fn add_page(
        &mut self,
        document: &PdfDocument,
        page_index: usize,
        ref_: Ref,
        location: Option<Location>,
    ) {
        // Unlike XObject's, we cannot cache page refs since Acrobat doesn't like duplicate page
        // refs in the page tree.
        let info = self.get_info(document);

        info.query_refs.push(ref_);
        info.queries.push(ExtractionQuery::new_page(page_index));
        info.locations.push(location);
    }

    fn get_info(&mut self, document: &PdfDocument) -> &mut PdfDocumentInfo {
        self.infos.entry(document.clone()).or_insert_with(|| {
            let info = PdfDocumentInfo::new(self.counter);
            self.counter += 1;

            info
        })
    }

    #[must_use = "This method might not use the original ref and could return a different one that \
    was previously cached."]
    pub(crate) fn add_xobject(
        &mut self,
        document: &PdfDocument,
        page_index: usize,
        ref_: Ref,
        location: Option<Location>,
    ) -> Ref {
        let info = self.get_info(document);

        if let Some(cached_ref) = info.cached_xobjects.get(&page_index) {
            return *cached_ref;
        }

        info.query_refs.push(ref_);
        info.queries.push(ExtractionQuery::new_xobject(page_index));
        info.locations.push(location);

        info.cached_xobjects.insert(page_index, ref_);

        ref_
    }

    pub(crate) fn serialize(
        self,
        sc: &mut SerializeContext,
        chunk_container: &mut ChunkContainer,
    ) -> KrillaResult<()> {
        let page_tree_parent_ref = sc.page_tree_ref();
        let krilla_version = sc.serialize_settings().configuration.version();

        let mut entries = self.infos.into_iter().collect::<Vec<_>>();
        // Make sure we always process them in the same order.
        entries.sort_by(|d1, d2| d1.1.counter.cmp(&d2.1.counter));

        for (doc, info) in entries {
            for location in info.locations.iter() {
                sc.register_validation_error(ValidationError::EmbeddedPDF(*location))
            }

            // In theory `hayro-write` only requires this for PDFs that are embedded
            // as XObject's to write the XObject group color space (embedded pages don't need it),
            // but we just always allocate it if we have at least one PDF,
            // regardless of how the files are embedded. We can change this in the
            // future, but it keeps the code simpler.
            let xobject_group_color_space = sc.register_colorspace(
                chunk_container,
                rgb::color_space(sc.serialize_settings().no_device_cs).into(),
            );
            let chunk_settings = sc.chunk_settings();

            let deferred_chunk = Deferred::new(move || {
                // We can't share the serializer context between threads, so each PDF has it's own
                // reference, and we remap it later in `ChunkContainer`.
                let mut new_ref = Ref::new(1);
                // `local_ref` refers to the Ref that will be assigned to it locally
                // in the extracted PDF chunk. Note that the colorspace itself
                // doesn't actually exist in the extracted PDF, i.e. the Ref
                // points to the null object. It just represents a placeholder Ref.
                // Later on when actually merging the PDF chunk into the final
                // krilla PDF, we replace `local_ref` with `cs_ref` such that it
                // actually points to the colorspace that is embedded in the full PDF.
                let (assigned_cs_ref, should_cs_ref) = match &xobject_group_color_space {
                    MaybeDeviceColorSpace::DeviceRgb => (None, None),
                    MaybeDeviceColorSpace::ColorSpace(cs) => {
                        (Some(new_ref.bump()), Some(cs.get_ref()))
                    }
                    MaybeDeviceColorSpace::DeviceGray | MaybeDeviceColorSpace::DeviceCMYK => {
                        unreachable!()
                    }
                };

                let first_location = info.locations.iter().flatten().next().cloned();
                let pdf = doc.pdf();

                let pdf_version = convert_pdf_version(pdf.version());

                if krilla_version < pdf_version {
                    return Err(KrillaError::Pdf(
                        doc.clone(),
                        PdfError::VersionMismatch(pdf_version),
                        first_location,
                    ));
                }

                let extracted = hayro_write::extract(
                    pdf,
                    Box::new(|| new_ref.bump()),
                    chunk_settings,
                    |group| {
                        if let Some(local_group_color_space_ref) = assigned_cs_ref {
                            group
                                .insert(Name(b"CS"))
                                .primitive(local_group_color_space_ref);
                        } else {
                            // If we aren't using an ICC color space, just use
                            // the device RGB color space.
                            group.color_space().device_rgb();
                        }
                    },
                    &info.queries,
                );
                let result = convert_extraction_result(extracted, &doc, first_location.as_ref())?;

                debug_assert_eq!(info.query_refs.len(), result.root_refs.len());

                let mut root_ref_mappings = HashMap::new();

                root_ref_mappings.insert(result.page_tree_parent_ref, page_tree_parent_ref);

                if let (Some(local_group_color_space_ref), Some(group_color_space_ref)) =
                    (assigned_cs_ref, should_cs_ref)
                {
                    root_ref_mappings.insert(local_group_color_space_ref, group_color_space_ref);
                }

                for ((should_ref, extraction_result), location) in info
                    .query_refs
                    .iter()
                    .zip(result.root_refs)
                    .zip(&info.locations)
                {
                    let assigned_ref = convert_extraction_result(
                        extraction_result,
                        &doc,
                        location.clone().as_ref(),
                    )?;

                    root_ref_mappings.insert(assigned_ref, *should_ref);
                }

                Ok(EmbeddedPdfChunk {
                    root_ref_mappings,
                    original_chunk: result.chunk,
                    new_chunk: OnceLock::new(),
                })
            });

            chunk_container.mixed.embedded_pdfs.push(deferred_chunk);
        }

        Ok(())
    }
}

fn convert_pdf_version(version: HayroPdfVersion) -> PdfVersion {
    match version {
        // Those are obviously not right, but we don't support versions lower than 1.4 in krilla.
        // Since we only need this conversion to detect version mismatches (in which case the
        // version of the embedded PDF has to be higher than 1.4), this hack is sufficient for our
        // purposes.
        HayroPdfVersion::Pdf10 => PdfVersion::Pdf14,
        HayroPdfVersion::Pdf11 => PdfVersion::Pdf14,
        HayroPdfVersion::Pdf12 => PdfVersion::Pdf14,
        HayroPdfVersion::Pdf13 => PdfVersion::Pdf14,

        HayroPdfVersion::Pdf14 => PdfVersion::Pdf14,
        HayroPdfVersion::Pdf15 => PdfVersion::Pdf15,
        HayroPdfVersion::Pdf16 => PdfVersion::Pdf16,
        HayroPdfVersion::Pdf17 => PdfVersion::Pdf17,
        HayroPdfVersion::Pdf20 => PdfVersion::Pdf20,
    }
}

fn convert_extraction_result<T>(
    result: Result<T, ExtractionError>,
    doc: &PdfDocument,
    location: Option<&Location>,
) -> KrillaResult<T> {
    result.map_err(|e| {
        let pdf_error = match e {
            ExtractionError::InvalidPageIndex(i) => PdfError::InvalidPage(i),
        };

        KrillaError::Pdf(doc.clone(), pdf_error, location.cloned())
    })
}
