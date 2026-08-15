use std::collections::BTreeSet;

use lopdf::{
    Dictionary, Document, LoadOptions, Object, ObjectId,
    content::{Content, Operation},
};

use super::{C6ProofError, C6ProofResult};

const MAX_PDF_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
const MAX_PDF_LOAD_STREAM_BYTES: usize = 32 * 1024 * 1024;
const MAX_PAGE_CONTENT_BYTES: usize = 1024 * 1024;
const MAX_PAGE_FONT_RESOURCES: usize = 256;
const MAX_DESCENDANT_FONTS: usize = 64;
const MAX_FONT_STREAM_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn prove_brutalist_state_pdf(bytes: &[u8]) -> C6ProofResult<()> {
    let document = load_pdf_artifact(bytes)?;
    let pages = document.get_pages();
    c6_ensure!(
        "pdf-page",
        pages.len() == 1,
        "the bounded PDF smoke requires exactly one page, found {}",
        pages.len()
    );
    let page_id = pages
        .values()
        .next()
        .copied()
        .ok_or_else(|| C6ProofError::new("pdf-page", "the PDF page is missing"))?;

    prove_finite_positive_media_box(&document, page_id)?;
    let page_content = document
        .get_page_content_with_limit(page_id, MAX_PAGE_CONTENT_BYTES)
        .map_err(|error| {
            C6ProofError::new(
                "pdf-page-content",
                format!("decode the bounded PDF page content: {error}"),
            )
        })?;
    c6_ensure!(
        "pdf-page-content",
        !page_content.is_empty() && page_content.len() <= MAX_PAGE_CONTENT_BYTES,
        "decoded PDF page content must be non-empty and at most {MAX_PAGE_CONTENT_BYTES} bytes; found {}",
        page_content.len()
    );
    let content = Content::decode_strict(&page_content).map_err(|error| {
        C6ProofError::new(
            "pdf-page-content",
            format!("strictly decode the bounded PDF page operators: {error}"),
        )
    })?;
    prove_basic_page_operations(&content.operations)?;
    prove_embedded_font_stream(&document, page_id)?;

    Ok(())
}

fn load_pdf_artifact(bytes: &[u8]) -> C6ProofResult<Document> {
    c6_ensure!(
        "pdf-artifact",
        !bytes.is_empty() && bytes.len() <= MAX_PDF_ARTIFACT_BYTES,
        "PDF artifact must be non-empty and at most {MAX_PDF_ARTIFACT_BYTES} bytes; found {}",
        bytes.len()
    );
    load_pdf_artifact_with_limit(bytes, MAX_PDF_LOAD_STREAM_BYTES)
}

fn load_pdf_artifact_with_limit(
    bytes: &[u8],
    max_decompressed_size: usize,
) -> C6ProofResult<Document> {
    Document::load_mem_with_options(
        bytes,
        LoadOptions::with_max_decompressed_size(max_decompressed_size),
    )
    .map_err(|error| {
        C6ProofError::new(
            "pdf-artifact",
            format!("parse the bounded PDF artifact: {error}"),
        )
    })
}

fn prove_finite_positive_media_box(document: &Document, page_id: ObjectId) -> C6ProofResult<()> {
    let media_box = inherited_media_box(document, page_id)?;
    let [left, bottom, right, top] = media_box;
    c6_ensure!(
        "pdf-page-box",
        media_box.into_iter().all(f64::is_finite),
        "PDF MediaBox coordinates must be finite"
    );
    c6_ensure!(
        "pdf-page-box",
        right > left && top > bottom,
        "PDF MediaBox dimensions must be positive; left={left}, bottom={bottom}, right={right}, top={top}"
    );
    Ok(())
}

fn inherited_media_box(document: &Document, page_id: ObjectId) -> C6ProofResult<[f64; 4]> {
    let mut current = page_id;
    let mut visited = BTreeSet::new();
    loop {
        c6_ensure!(
            "pdf-page-box",
            visited.insert(current),
            "PDF page parent chain contains a cycle"
        );
        let dictionary = document.get_dictionary(current).map_err(|error| {
            C6ProofError::new(
                "pdf-page-box",
                format!("read PDF page ancestor {current:?}: {error}"),
            )
        })?;
        if let Ok(media_box) = dictionary.get_deref(b"MediaBox", document) {
            let values = media_box.as_array().map_err(|error| {
                C6ProofError::new(
                    "pdf-page-box",
                    format!("PDF MediaBox must be an array: {error}"),
                )
            })?;
            let [left, bottom, right, top] = values.as_slice() else {
                return Err(C6ProofError::new(
                    "pdf-page-box",
                    format!(
                        "PDF MediaBox must contain four numbers, found {}",
                        values.len()
                    ),
                ));
            };
            return Ok([
                pdf_number(left)?,
                pdf_number(bottom)?,
                pdf_number(right)?,
                pdf_number(top)?,
            ]);
        }
        current = dictionary
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(|error| {
                C6ProofError::new(
                    "pdf-page-box",
                    format!("PDF page tree lacks an inherited MediaBox: {error}"),
                )
            })?;
    }
}

fn pdf_number(object: &Object) -> C6ProofResult<f64> {
    object.as_float().map(f64::from).map_err(|error| {
        C6ProofError::new(
            "pdf-page-box",
            format!("PDF MediaBox coordinate must be numeric: {error}"),
        )
    })
}

fn prove_basic_page_operations(operations: &[Operation]) -> C6ProofResult<()> {
    c6_ensure!(
        "pdf-drawing",
        operations.iter().any(is_basic_drawing_operation),
        "PDF page content must contain at least one basic drawing operation"
    );
    c6_ensure!(
        "pdf-text",
        operations.iter().any(is_text_show_operation),
        "PDF page content must contain at least one text-show operation"
    );
    Ok(())
}

fn is_basic_drawing_operation(operation: &Operation) -> bool {
    matches!(
        operation.operator.as_str(),
        "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "Do"
    )
}

fn is_text_show_operation(operation: &Operation) -> bool {
    matches!(operation.operator.as_str(), "Tj" | "TJ" | "'" | "\"")
}

fn prove_embedded_font_stream(document: &Document, page_id: ObjectId) -> C6ProofResult<()> {
    let fonts = document.get_page_fonts(page_id).map_err(|error| {
        C6ProofError::new(
            "pdf-font",
            format!("read the bounded PDF page fonts: {error}"),
        )
    })?;
    c6_ensure!(
        "pdf-font",
        !fonts.is_empty() && fonts.len() <= MAX_PAGE_FONT_RESOURCES,
        "PDF page must expose between 1 and {MAX_PAGE_FONT_RESOURCES} font resources; found {}",
        fonts.len()
    );

    let mut embedded_stream_found = false;
    for font in fonts.values() {
        embedded_stream_found |= font_has_embedded_stream(document, font)?;
    }
    c6_ensure!(
        "pdf-font",
        embedded_stream_found,
        "PDF page must reference at least one non-empty embedded font stream"
    );
    Ok(())
}

fn font_has_embedded_stream(document: &Document, font: &Dictionary) -> C6ProofResult<bool> {
    if descriptor_has_embedded_stream(document, font)? {
        return Ok(true);
    }
    let Ok(descendant_fonts) = font.get(b"DescendantFonts") else {
        return Ok(false);
    };
    let (_, descendant_fonts) = document.dereference(descendant_fonts).map_err(|error| {
        C6ProofError::new(
            "pdf-font",
            format!("dereference PDF descendant font array: {error}"),
        )
    })?;
    let descendants = descendant_fonts.as_array().map_err(|error| {
        C6ProofError::new(
            "pdf-font",
            format!("PDF DescendantFonts must be an array: {error}"),
        )
    })?;
    c6_ensure!(
        "pdf-font",
        descendants.len() <= MAX_DESCENDANT_FONTS,
        "PDF font contains too many descendants; found {}, max={MAX_DESCENDANT_FONTS}",
        descendants.len()
    );
    for descendant in descendants {
        let (_, descendant) = document.dereference(descendant).map_err(|error| {
            C6ProofError::new(
                "pdf-font",
                format!("dereference PDF descendant font: {error}"),
            )
        })?;
        let descendant = descendant.as_dict().map_err(|error| {
            C6ProofError::new(
                "pdf-font",
                format!("PDF descendant font must be a dictionary: {error}"),
            )
        })?;
        if descriptor_has_embedded_stream(document, descendant)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn descriptor_has_embedded_stream(document: &Document, font: &Dictionary) -> C6ProofResult<bool> {
    let Ok(descriptor) = font.get(b"FontDescriptor") else {
        return Ok(false);
    };
    let (_, descriptor) = document.dereference(descriptor).map_err(|error| {
        C6ProofError::new(
            "pdf-font",
            format!("dereference PDF font descriptor: {error}"),
        )
    })?;
    let descriptor = descriptor.as_dict().map_err(|error| {
        C6ProofError::new(
            "pdf-font",
            format!("PDF font descriptor must be a dictionary: {error}"),
        )
    })?;

    for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
        let Ok(font_stream) = descriptor.get(key) else {
            continue;
        };
        let (_, font_stream) = document.dereference(font_stream).map_err(|error| {
            C6ProofError::new(
                "pdf-font",
                format!("dereference embedded PDF font stream: {error}"),
            )
        })?;
        let font_stream = font_stream.as_stream().map_err(|error| {
            C6ProofError::new(
                "pdf-font",
                format!("embedded PDF font object must be a stream: {error}"),
            )
        })?;
        let decoded = font_stream
            .decompressed_content_with_limit(MAX_FONT_STREAM_BYTES)
            .map_err(|error| {
                C6ProofError::new(
                    "pdf-font",
                    format!("decode the bounded embedded PDF font stream: {error}"),
                )
            })?;
        c6_ensure!(
            "pdf-font",
            !decoded.is_empty(),
            "embedded PDF font stream must be non-empty; found {} bytes",
            decoded.len()
        );
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;

    fn flate_bomb(target: usize) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        let zeros = [0u8; 4 * 1024];
        let mut remaining = target;
        while remaining > 0 {
            let chunk = remaining.min(zeros.len());
            encoder
                .write_all(&zeros[..chunk])
                .expect("compress bounded test payload");
            remaining -= chunk;
        }
        encoder.finish().expect("finish bounded test payload")
    }

    fn xref_stream_bomb_pdf(bomb: &[u8]) -> Vec<u8> {
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.5\n");
        let object_offset = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n");
        pdf.extend_from_slice(
            format!(
                "<< /Type /XRef /Size 1 /W [1 1 1] /Root 1 0 R /Filter /FlateDecode /Length {} >>\n",
                bomb.len()
            )
            .as_bytes(),
        );
        pdf.extend_from_slice(b"stream\n");
        pdf.extend_from_slice(bomb);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
        pdf.extend_from_slice(format!("startxref\n{object_offset}\n%%EOF").as_bytes());
        pdf
    }

    #[test]
    fn pdf_loader_rejects_load_time_decompression_bombs() {
        let pdf = xref_stream_bomb_pdf(&flate_bomb(16 * 1024));

        let error = load_pdf_artifact_with_limit(&pdf, 1024)
            .expect_err("load-time decompression must honor the configured bound");

        assert_eq!(error.stage, "pdf-artifact");
        assert!(
            error.detail.contains("decompressed output exceeded")
                && error.detail.contains("1024-byte limit"),
            "unexpected decompression error: {error}"
        );
    }

    #[test]
    fn malformed_pdf_artifact_returns_structured_parse_error() {
        let mut pdf = b"%PDF-1.7\n".to_vec();
        pdf.resize(1030, b'x');
        pdf.extend_from_slice(b"\nstartxref\n0\n%%EOF");

        let error = load_pdf_artifact(&pdf).expect_err("a malformed PDF must fail closed");

        assert_eq!(error.stage, "pdf-artifact");
        assert!(!error.detail.is_empty());
    }

    #[test]
    fn page_operator_smoke_requires_drawing_and_text() {
        let drawing = Operation::new("f", vec![]);
        let text = Operation::new("Tj", vec![Object::string_literal("smoke")]);

        prove_basic_page_operations(&[drawing.clone(), text.clone()])
            .expect("drawing plus text satisfies the bounded page smoke");
        assert_eq!(
            prove_basic_page_operations(&[text])
                .expect_err("text alone must not prove page drawing")
                .stage,
            "pdf-drawing"
        );
        assert_eq!(
            prove_basic_page_operations(&[drawing])
                .expect_err("drawing alone must not prove page text")
                .stage,
            "pdf-text"
        );
    }
}
