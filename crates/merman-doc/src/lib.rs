#![doc = include_str!("../README.md")]

mod html;
mod markdown;

pub use html::{SvgVariants, diagram_html_len, write_diagram_html};
pub use markdown::{Block, BlockKind, DocError, Embedding, Location, scan, visit_blocks};
