use crate::error::{Error, Result};
use crate::html::diagram_html;
use crate::options::{FailMode, Options};
use crate::render::{
    HeadlessMermaidRenderer, IncludeResolver, ManifestIncludeResolver, MermaidRenderer,
    source_preview,
};
use crate::svg::validate_svg;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Fence {
    ch: char,
    len: usize,
}

pub(crate) fn rewrite_doc_lines(
    lines: &[String],
    next_diagram: &mut usize,
    options: Options,
) -> Result<Vec<String>> {
    let mut render = HeadlessMermaidRenderer;
    let mut include = ManifestIncludeResolver;
    rewrite_doc_lines_with(lines, next_diagram, options, &mut render, &mut include)
}

fn rewrite_doc_lines_with<R, I>(
    lines: &[String],
    next_diagram: &mut usize,
    options: Options,
    render: &mut R,
    include: &mut I,
) -> Result<Vec<String>>
where
    R: MermaidRenderer,
    I: IncludeResolver,
{
    let mut out = Vec::with_capacity(lines.len());
    let mut i = 0;
    let mut non_mermaid_fence = None;

    while i < lines.len() {
        let markdown = markdown_line(&lines[i]);

        if let Some(fence) = non_mermaid_fence {
            out.push(lines[i].clone());
            if is_fence_end(markdown, fence) {
                non_mermaid_fence = None;
            }
            i += 1;
            continue;
        }

        if let Some(fence) = mermaid_fence_start(markdown) {
            let start = i;
            i += 1;
            let mut body = Vec::new();
            while i < lines.len() {
                let line = markdown_line(&lines[i]);
                if is_fence_end(line, fence) {
                    break;
                }
                body.push(line.to_string());
                i += 1;
            }

            if i == lines.len() {
                if options.fail == FailMode::KeepSource {
                    out.extend(lines[start..].iter().cloned());
                    break;
                }
                return Err(Error::new(format!(
                    "unclosed Mermaid fence in rustdoc comment starting at doc line {}",
                    start + 1
                )));
            }

            let source = body.join("\n");
            let origin = format!("Mermaid fence starting at doc line {}", start + 1);
            match render_diagram_block(&source, next_diagram, options, &origin, render) {
                Ok(block) => out.push(block),
                Err(_) if options.fail == FailMode::KeepSource => {
                    out.extend(lines[start..=i].iter().cloned());
                }
                Err(err) => return Err(err),
            }
            i += 1;
            continue;
        }

        match parse_include_mmd(markdown) {
            Ok(Some(path)) => {
                let block = include.read_include_mmd(&path).and_then(|source| {
                    let origin = format!("include_mmd!(\"{path}\") at doc line {}", i + 1);
                    render_diagram_block(&source, next_diagram, options, &origin, render)
                });
                match block {
                    Ok(block) => out.push(block),
                    Err(_) if options.fail == FailMode::KeepSource => out.push(lines[i].clone()),
                    Err(err) => return Err(err),
                }
                i += 1;
                continue;
            }
            Ok(None) => {}
            Err(_) if options.fail == FailMode::KeepSource => {
                out.push(lines[i].clone());
                i += 1;
                continue;
            }
            Err(err) => return Err(Error::new(format!("doc line {}: {err}", i + 1))),
        }

        if let Some(fence) = any_fence_start(markdown) {
            non_mermaid_fence = Some(fence);
        }

        out.push(lines[i].clone());
        i += 1;
    }

    Ok(out)
}

fn render_diagram_block<R>(
    source: &str,
    next_diagram: &mut usize,
    options: Options,
    origin: &str,
    render: &mut R,
) -> Result<String>
where
    R: MermaidRenderer,
{
    let index = *next_diagram;
    let svg = render
        .render_mermaid_svg(source, index, options.pipeline)
        .map_err(|err| Error::new(format!("{origin} near `{}`: {err}", source_preview(source))))?;
    validate_svg(&svg, options.sanitize)
        .map_err(|err| Error::new(format!("{origin} near `{}`: {err}", source_preview(source))))?;
    *next_diagram += 1;
    Ok(diagram_html(source, &svg, options.source))
}

fn markdown_line(line: &str) -> &str {
    line.strip_prefix(' ').unwrap_or(line)
}

fn any_fence_start(line: &str) -> Option<Fence> {
    let trimmed = line.trim_start();
    let mut chars = trimmed.chars();
    let ch = chars.next()?;
    if ch != '`' && ch != '~' {
        return None;
    }

    let len = trimmed.chars().take_while(|current| *current == ch).count();
    (len >= 3).then_some(Fence { ch, len })
}

fn mermaid_fence_start(line: &str) -> Option<Fence> {
    let fence = any_fence_start(line)?;
    let trimmed = line.trim_start();
    let rest = trimmed
        .char_indices()
        .nth(fence.len)
        .map(|(idx, _)| &trimmed[idx..])
        .unwrap_or("")
        .trim();
    let info = rest
        .trim_start_matches('{')
        .split(|ch: char| ch.is_whitespace() || ch == '}' || ch == ',')
        .next()
        .unwrap_or("");

    info.eq_ignore_ascii_case("mermaid").then_some(fence)
}

fn is_fence_end(line: &str, fence: Fence) -> bool {
    let trimmed = line.trim_start();
    let len = trimmed
        .chars()
        .take_while(|current| *current == fence.ch)
        .count();
    if len < fence.len {
        return false;
    }

    let rest = trimmed
        .char_indices()
        .nth(len)
        .map(|(idx, _)| &trimmed[idx..])
        .unwrap_or("");
    rest.trim().is_empty()
}

fn parse_include_mmd(line: &str) -> Result<Option<String>> {
    let trimmed = line.trim();
    let Some(rest) = trimmed.strip_prefix("include_mmd!") else {
        return Ok(None);
    };
    let rest = rest.trim();
    let Some(inner) = rest.strip_prefix('(').and_then(|s| s.strip_suffix(')')) else {
        return Err(Error::new(
            "invalid include_mmd! syntax in rustdoc comment; expected include_mmd!(\"path.mmd\")",
        ));
    };
    let lit = syn::parse_str::<syn::LitStr>(inner.trim()).map_err(|err| {
        Error::new(format!(
            "invalid include_mmd! path literal in rustdoc comment: {err}"
        ))
    })?;
    Ok(Some(lit.value()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{PipelineMode, SanitizeMode, SourceMode};

    fn rewrite_with_fake_renderer(lines: &[&str]) -> Result<(Vec<String>, Vec<String>)> {
        rewrite_with_fake_renderer_options(lines, Options::default())
    }

    fn rewrite_with_fake_renderer_options(
        lines: &[&str],
        options: Options,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let lines = lines
            .iter()
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>();
        let mut rendered_sources = Vec::new();
        let mut render = |source: &str, index: usize, pipeline: PipelineMode| {
            assert_eq!(pipeline, options.pipeline);
            rendered_sources.push(source.to_string());
            Ok(format!(r#"<svg id="diagram-{index}"></svg>"#))
        };
        let mut include = |path: &str| Ok(format!("flowchart TD\nA[{path}] --> B[Done]"));
        let mut next = 0;
        let out = rewrite_doc_lines_with(&lines, &mut next, options, &mut render, &mut include)?;
        Ok((out, rendered_sources))
    }

    #[test]
    fn rewrites_mermaid_fence_to_inline_svg_block() {
        let (out, rendered_sources) = rewrite_with_fake_renderer(&[
            " Intro",
            " ```mermaid",
            " flowchart TD",
            "   A --> B",
            " ```",
            " Outro",
        ])
        .unwrap();

        assert_eq!(rendered_sources, vec!["flowchart TD\n  A --> B"]);
        assert_eq!(out[0], " Intro");
        assert!(out[1].contains(r#"class="merman-rustdoc-diagram""#));
        assert!(out[1].contains(r#"<svg id="diagram-0"></svg>"#));
        assert_eq!(out[2], " Outro");
    }

    #[test]
    fn supports_tilde_mermaid_fences() {
        let (out, rendered_sources) = rewrite_with_fake_renderer(&[
            " Before",
            " ~~~ mermaid",
            " sequenceDiagram",
            "   A->>B: hi",
            " ~~~",
        ])
        .unwrap();

        assert_eq!(rendered_sources, vec!["sequenceDiagram\n  A->>B: hi"]);
        assert!(out[1].contains(r#"<svg id="diagram-0"></svg>"#));
    }

    #[test]
    fn include_mmd_uses_resolver_and_renders_result() {
        let (out, rendered_sources) =
            rewrite_with_fake_renderer(&[" Intro", " include_mmd!(\"docs/diagram.mmd\")"]).unwrap();

        assert_eq!(
            rendered_sources,
            vec!["flowchart TD\nA[docs/diagram.mmd] --> B[Done]"]
        );
        assert!(out[1].contains(r#"<svg id="diagram-0"></svg>"#));
    }

    #[test]
    fn include_mmd_inside_non_mermaid_fence_is_preserved() {
        let (out, rendered_sources) = rewrite_with_fake_renderer(&[
            " ```rust",
            " include_mmd!(\"docs/diagram.mmd\")",
            " ```",
        ])
        .unwrap();

        assert!(rendered_sources.is_empty());
        assert_eq!(
            out,
            vec![
                " ```rust".to_string(),
                " include_mmd!(\"docs/diagram.mmd\")".to_string(),
                " ```".to_string()
            ]
        );
    }

    #[test]
    fn source_details_adds_escaped_mermaid_source() {
        let options = Options {
            source: SourceMode::Details,
            ..Options::default()
        };
        let (out, _rendered_sources) = rewrite_with_fake_renderer_options(
            &[
                " ```mermaid",
                " flowchart TD",
                "   A[<Start & Go>] --> B[Done]",
                " ```",
            ],
            options,
        )
        .unwrap();

        assert!(out[0].contains(r#"class="merman-rustdoc-source""#));
        assert!(out[0].contains("Mermaid source"));
        assert!(out[0].contains("A[&lt;Start &amp; Go&gt;]"));
    }

    #[test]
    fn keep_source_preserves_fence_when_render_fails() {
        let lines = [
            " Intro",
            " ```mermaid",
            " flowchart TD",
            "   A --> B",
            " ```",
            " Outro",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let options = Options {
            fail: FailMode::KeepSource,
            ..Options::default()
        };
        let mut render =
            |_source: &str, _index: usize, _pipeline: PipelineMode| Err(Error::new("boom"));
        let mut include = |_path: &str| Ok(String::new());
        let mut next = 0;

        let out =
            rewrite_doc_lines_with(&lines, &mut next, options, &mut render, &mut include).unwrap();

        assert_eq!(out, lines);
        assert_eq!(next, 0);
    }

    #[test]
    fn keep_source_preserves_include_when_file_read_fails() {
        let lines = [" include_mmd!(\"missing.mmd\")"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let options = Options {
            fail: FailMode::KeepSource,
            ..Options::default()
        };
        let mut render = |_source: &str, _index: usize, _pipeline: PipelineMode| Ok(String::new());
        let mut include = |_path: &str| Err(Error::new("missing"));
        let mut next = 0;

        let out =
            rewrite_doc_lines_with(&lines, &mut next, options, &mut render, &mut include).unwrap();

        assert_eq!(out, lines);
    }

    #[test]
    fn render_error_mentions_doc_line_and_source_preview() {
        let lines = [
            " Intro",
            " ```mermaid",
            " flowchart TD",
            "   A --> B",
            " ```",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let mut render = |_source: &str, _index: usize, _pipeline: PipelineMode| {
            Err(Error::new("render failed"))
        };
        let mut include = |_path: &str| Ok(String::new());
        let mut next = 0;

        let err = rewrite_doc_lines_with(
            &lines,
            &mut next,
            Options::default(),
            &mut render,
            &mut include,
        )
        .unwrap_err();
        let err = err.to_string();

        assert!(err.contains("Mermaid fence starting at doc line 2"));
        assert!(err.contains("flowchart TD"));
        assert!(err.contains("render failed"));
    }

    #[test]
    fn invalid_include_syntax_mentions_doc_line() {
        let lines = [" Intro", " include_mmd!(docs/diagram.mmd)"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut render = |_source: &str, _index: usize, _pipeline: PipelineMode| Ok(String::new());
        let mut include = |_path: &str| Ok(String::new());
        let mut next = 0;

        let err = rewrite_doc_lines_with(
            &lines,
            &mut next,
            Options::default(),
            &mut render,
            &mut include,
        )
        .unwrap_err();

        assert!(err.to_string().contains("doc line 2"));
    }

    #[test]
    fn unclosed_mermaid_fence_is_an_error() {
        let lines = [" ```mermaid", " flowchart TD"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut next = 0;
        let mut render = |_source: &str, _index: usize, _pipeline: PipelineMode| Ok(String::new());
        let mut include = |_path: &str| Ok(String::new());

        let err = rewrite_doc_lines_with(
            &lines,
            &mut next,
            Options::default(),
            &mut render,
            &mut include,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unclosed Mermaid fence"));
    }

    #[test]
    fn invalid_include_syntax_is_an_error() {
        let err = parse_include_mmd("include_mmd!(docs/diagram.mmd)").unwrap_err();

        assert!(
            err.to_string()
                .contains("invalid include_mmd! path literal")
        );
    }

    #[test]
    fn strict_sanitize_rejects_dangerous_rendered_svg() {
        let lines = [" ```mermaid", " flowchart TD", "   A --> B", " ```"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let mut render = |_source: &str, _index: usize, _pipeline: PipelineMode| {
            Ok(r#"<svg><script>alert(1)</script></svg>"#.to_string())
        };
        let mut include = |_path: &str| Ok(String::new());
        let mut next = 0;

        let err = rewrite_doc_lines_with(
            &lines,
            &mut next,
            Options::default(),
            &mut render,
            &mut include,
        )
        .unwrap_err();

        assert!(err.to_string().contains("strict SVG sanitization"));
        assert_eq!(next, 0);
    }

    #[test]
    fn sanitize_off_allows_dangerous_rendered_svg() {
        let lines = [" ```mermaid", " flowchart TD", "   A --> B", " ```"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let options = Options {
            sanitize: SanitizeMode::Off,
            ..Options::default()
        };
        let mut render = |_source: &str, _index: usize, _pipeline: PipelineMode| {
            Ok(r#"<svg><script>alert(1)</script></svg>"#.to_string())
        };
        let mut include = |_path: &str| Ok(String::new());
        let mut next = 0;

        let out =
            rewrite_doc_lines_with(&lines, &mut next, options, &mut render, &mut include).unwrap();

        assert!(out[0].contains("<script>"));
        assert_eq!(next, 1);
    }
}
