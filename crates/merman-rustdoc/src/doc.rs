use crate::error::{Error, Result};
use crate::options::{FailMode, Options, SourceMode};
use crate::render::{RenderedDiagram, read_include_mmd, render_mermaid_diagram, source_preview};
use merman_doc::{BlockKind, SvgVariants};

/// Prepare the lexical comment sugar before rustdoc's common indentation pass.
/// Raw doc attributes are Markdown and must not lose literal list markers.
pub(crate) fn prepare_literal(literal: &syn::LitStr) -> String {
    let text = literal.value();
    let source = literal.span().source_text().unwrap_or_default();
    let block = source.starts_with("/**") || source.starts_with("/*!");
    let sugared = block || source.starts_with("///") || source.starts_with("//!");
    if !sugared {
        return text;
    }
    let mut lines = text.lines().collect::<Vec<_>>();
    if block {
        let first = lines
            .iter()
            .position(|line| !line.trim().is_empty())
            .unwrap_or(lines.len());
        let end = lines
            .iter()
            .rposition(|line| !line.trim().is_empty())
            .map_or(first, |index| index + 1);
        lines = lines[first..end].to_vec();
        let decorated_start = usize::from(
            lines
                .first()
                .is_some_and(|line| !line.trim_start().starts_with('*')),
        );
        if !lines[decorated_start..].is_empty()
            && lines[decorated_start..]
                .iter()
                .all(|line| line.trim().is_empty() || line.trim_start().starts_with('*'))
        {
            for line in &mut lines[decorated_start..] {
                *line = line.trim_start().strip_prefix('*').unwrap_or("");
            }
        }
    }
    lines
        .into_iter()
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn common_indentation(fragments: &[syn::LitStr]) -> usize {
    fragments
        .iter()
        .filter_map(|fragment| {
            fragment
                .value()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.bytes().take_while(|b| *b == b' ').count())
                .min()
        })
        .min()
        .unwrap_or(0)
}

pub(crate) fn normalize_document(fragments: &[String], indentation: usize) -> String {
    let combined = fragments.join("\n");
    combined
        .lines()
        .map(|line| {
            &line[line
                .bytes()
                .take_while(|b| *b == b' ')
                .count()
                .min(indentation)..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn rewrite_document(source: &str, options: &Options, namespace: &str) -> Result<String> {
    let mut output = String::with_capacity(source.len());
    let mut copied = 0;
    for (index, block) in merman_doc::scan(source).into_iter().enumerate() {
        output.push_str(&source[copied..block.span.start]);
        let rendered = (|| {
            let diagram_source = match block.kind {
                BlockKind::Mermaid(source) => source,
                BlockKind::Include(path) => read_include_mmd(&path)?,
                BlockKind::Invalid(error) => return Err(Error::new(error.to_string())),
            };
            let diagram = render_mermaid_diagram(&diagram_source, index, options, namespace)
                .map_err(|err| {
                    Error::new(format!("near `{}`: {err}", source_preview(&diagram_source)))
                })?;
            let variants = match &diagram {
                RenderedDiagram::Single(svg) => SvgVariants::Single(svg),
                RenderedDiagram::RustdocTheme { light, dark } => {
                    SvgVariants::RustdocTheme { light, dark }
                }
            };
            let mut html = String::new();
            merman_doc::write_diagram_html(
                &mut html,
                None,
                &diagram_source,
                variants,
                options.source == SourceMode::Details,
            )
            .map_err(|_| Error::new("failed to embed rendered SVG in Markdown"))?;
            Ok(html)
        })();
        match rendered {
            Ok(html) => block
                .embedding
                .with_trailing_boundary()
                .write_html(&mut output, &html)
                .map_err(|_| Error::new("failed to embed diagram in its Markdown container"))?,
            Err(_) if options.fail == FailMode::KeepSource => {
                output.push_str(&source[block.span.clone()])
            }
            Err(err) => {
                return Err(Error::new(format!(
                    "Mermaid block at doc line {}, column {}: {err}",
                    block.location.line, block.location.column
                )));
            }
        }
        copied = block.span.end;
    }
    output.push_str(&source[copied..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_lines_and_multiline_fragments_without_flattening_code() {
        assert_eq!(
            normalize_document(&[" Intro".into(), "".into(), "     example".into()], 1),
            "Intro\n\n    example"
        );
        assert_eq!(
            normalize_document(&["Intro\n\n    example".into()], 0),
            "Intro\n\n    example"
        );
    }
    #[test]
    fn tolerant_mode_keeps_a_failed_block_and_renders_the_next() {
        let source =
            "include_mmd!(\"missing-test.mmd\")\n\n```mermaid\nflowchart TD\nA-->B\n```\n**After**";
        let options = Options {
            fail: FailMode::KeepSource,
            ..Options::default()
        };
        let output = rewrite_document(source, &options, "test").unwrap();
        assert!(output.contains("include_mmd!(\"missing-test.mmd\")"));
        assert!(output.contains("<svg"));
        assert!(output.contains("\n\n**After**"));
    }
}
