//! HTML5 recovery and target serialization share one flat, non-recursive tree.

use super::{DompurifyEffectiveConfig, SanitizeFailure, SanitizeOutputSink};
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{BufferQueue, TokenSink, TokenSinkResult, Tokenizer};
use html5ever::tree_builder::{
    ElementFlags, NodeOrText, QuirksMode, Tracer, TreeBuilder, TreeSink,
};
use html5ever::{Attribute, LocalName, Namespace, QualName, TokenizerResult, ns};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

const NONE: usize = usize::MAX;
const CHUNK_BYTES: usize = 256;
const PARSE_WORK_BATCH: usize = 64;
const WORK_BYTES_PER_UNIT: usize = 256;

#[derive(Clone, Debug)]
struct Handle {
    index: usize,
    name: QualName,
    integration: bool,
    attribute_bytes: usize,
}

impl html5ever::tree_builder::ElemName for &Handle {
    fn ns(&self) -> &Namespace {
        &self.name.ns
    }
    fn local_name(&self) -> &LocalName {
        &self.name.local
    }
}

enum Kind {
    Container,
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
    },
    Text(StrTendril),
    Ignored,
}

struct Node {
    kind: Kind,
    parent: usize,
    first: usize,
    last: usize,
    prev: usize,
    next: usize,
    template: usize,
}

impl Node {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            parent: NONE,
            first: NONE,
            last: NONE,
            prev: NONE,
            next: NONE,
            template: NONE,
        }
    }
}

struct State<E> {
    nodes: Vec<Node>,
    retained: usize,
    error: Option<SanitizeFailure<E>>,
    merged_attribute_names: HashMap<usize, HashSet<QualName>>,
    parse_work: usize,
    total_parse_work: usize,
    parse_work_limit: Option<usize>,
}

struct Builder<'a, S: SanitizeOutputSink> {
    state: RefCell<State<S::Error>>,
    sink: &'a S,
    input_bytes: usize,
}

/// Stops forwarding tokens to the HTML5 tree builder after a sink rejects work.
///
/// `TreeSink` cannot return a `Result`, so a failed callback otherwise leaves the
/// tokenizer running through every remaining token in the chunk. This adapter uses
/// html5ever's public `TokenSink` boundary. Resource admission observes token kinds
/// but does not implement HTML parsing, recovery, or scope rules of its own.
struct StoppingTreeBuilder<'a, S: SanitizeOutputSink> {
    inner: TreeBuilder<Handle, Builder<'a, S>>,
    stopped: std::cell::Cell<bool>,
    complete_token: std::cell::Cell<bool>,
}

struct FormattingWork<'a, 'b, S: SanitizeOutputSink> {
    builder: &'a Builder<'b, S>,
    tag: &'a html5ever::tokenizer::Tag,
    attribute_bytes: usize,
}

impl<S: SanitizeOutputSink> Tracer for FormattingWork<'_, '_, S> {
    type Handle = Handle;

    fn trace_handle(&self, handle: &Handle) {
        if !self.builder.charge_parse_work(1) {
            return;
        }
        let attrs = {
            let state = self.builder.state.borrow();
            state
                .nodes
                .get(handle.index)
                .and_then(|node| match &node.kind {
                    Kind::Element { name, attrs }
                        if name.ns == ns!(html) && name.local == self.tag.name =>
                    {
                        Some(attrs.len())
                    }
                    _ => None,
                })
        };
        if let Some(attrs) = attrs {
            // equiv_modulo_attr_order clones and sorts both attribute vectors.
            // A squared count bounds that comparison work without duplicating its
            // sorting or the parser's active-formatting state. Live handles can be
            // traced twice (open elements and formatting list), which is conservative.
            let existing = attrs.saturating_add(1);
            let incoming = self.tag.attrs.len().saturating_add(1);
            let comparison_bytes = handle
                .attribute_bytes
                .saturating_add(self.attribute_bytes)
                .div_ceil(WORK_BYTES_PER_UNIT)
                .max(1);
            self.builder.charge_parse_work(
                existing
                    .saturating_mul(existing)
                    .saturating_add(incoming.saturating_mul(incoming))
                    .saturating_mul(comparison_bytes),
            );
        }
    }
}

impl<S: SanitizeOutputSink> TokenSink for StoppingTreeBuilder<'_, S> {
    type Handle = Handle;

    fn process_token(
        &self,
        token: html5ever::tokenizer::Token,
        line_number: u64,
    ) -> TokenSinkResult<Self::Handle> {
        if self.stopped.get() || self.inner.sink.failed() {
            self.stopped.set(true);
            return TokenSinkResult::Continue;
        }
        if !matches!(token, html5ever::tokenizer::Token::ParseError(_)) {
            self.complete_token.set(true);
        }
        if let html5ever::tokenizer::Token::TagToken(tag) = &token
            && tag.kind == html5ever::tokenizer::StartTag
            // Pinned html5ever rules.rs: these are the only start tags that call
            // create_formatting_element_for, whose candidate scan has no callbacks.
            && matches!(tag.name.as_ref(), "a" | "b" | "big" | "code" | "em" | "font"
                | "i" | "s" | "small" | "strike" | "strong" | "tt" | "u" | "nobr")
        {
            self.inner.trace_handles(&FormattingWork {
                builder: &self.inner.sink,
                tag,
                attribute_bytes: tag.attrs.iter().fold(0usize, |bytes, attr| {
                    bytes
                        .saturating_add(attr.name.local.len())
                        .saturating_add(attr.value.len())
                }),
            });
            if !self.inner.sink.flush_parse_work() {
                self.stopped.set(true);
                return TokenSinkResult::Continue;
            }
        }
        let result = self.inner.process_token(token, line_number);
        if self.inner.sink.failed() {
            self.stopped.set(true);
        }
        result
    }

    fn end(&self) {
        if !self.stopped.get() && !self.inner.sink.failed() {
            self.inner.end();
            if self.inner.sink.failed() {
                self.stopped.set(true);
            }
        }
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        !self.stopped.get()
            && !self.inner.sink.failed()
            && self
                .inner
                .adjusted_current_node_present_but_not_in_html_namespace()
    }
}

impl<S: SanitizeOutputSink> Builder<'_, S> {
    fn failed(&self) -> bool {
        self.state.borrow().error.is_some()
    }

    /// Charge TreeSink callbacks in bounded batches. html5ever's tree builder
    /// performs ancestor and active-formatting scans through these callbacks; the
    /// callbacks are the parser's stable observation point, so this accounting does
    /// not duplicate any HTML parsing logic here.
    fn charge_parse_work(&self, additional: usize) -> bool {
        if additional == 0 || self.failed() {
            return !self.failed();
        }

        let charge = {
            let mut state = self.state.borrow_mut();
            let Some(total) = state.total_parse_work.checked_add(additional) else {
                state.error = Some(SanitizeFailure::RejectedInput);
                return false;
            };
            if state.parse_work_limit.is_some_and(|limit| total > limit) {
                state.error = Some(SanitizeFailure::RejectedInput);
                return false;
            }
            state.total_parse_work = total;
            state.parse_work = state.parse_work.saturating_add(additional);
            if state.parse_work >= PARSE_WORK_BATCH {
                let charge = state.parse_work;
                state.parse_work = 0;
                charge
            } else {
                0
            }
        };

        if charge != 0
            && let Err(error) = self.sink.charge_parse_work(charge)
        {
            self.state.borrow_mut().error = Some(SanitizeFailure::Output(error));
            return false;
        }
        true
    }

    fn flush_parse_work(&self) -> bool {
        if self.failed() {
            return false;
        }
        let charge = {
            let mut state = self.state.borrow_mut();
            let charge = state.parse_work;
            state.parse_work = 0;
            charge
        };
        if charge != 0
            && let Err(error) = self.sink.charge_parse_work(charge)
        {
            self.state.borrow_mut().error = Some(SanitizeFailure::Output(error));
            return false;
        }
        true
    }

    fn admit(&self, nodes: usize, bytes: usize) -> bool {
        if self.failed() {
            return false;
        }
        let mut state = self.state.borrow_mut();
        let retained = state.retained.saturating_add(bytes);
        // Input admission bounds tokenizer scratch. This additional bound covers implicit
        // formatting clones and tree bookkeeping before they can amplify retained storage.
        // The tree builder may synthesize table containers and formatting clones. Each
        // retained node costs at least one source byte; allow two complete node/attribute
        // records per input byte plus the document/context/root bookkeeping.
        let record_bytes = 2 * (std::mem::size_of::<Node>() + std::mem::size_of::<Attribute>());
        let ceiling = self
            .input_bytes
            .saturating_add(3)
            .saturating_mul(record_bytes);
        let result = self.sink.checkpoint().and_then(|()| {
            self.sink
                .check_parse_storage(self.input_bytes, nodes, retained)
        });
        if let Err(error) = result {
            state.error = Some(SanitizeFailure::Output(error));
            return false;
        }
        if retained > ceiling {
            state.error = Some(SanitizeFailure::RejectedInput);
            return false;
        }
        state.retained = retained;
        true
    }

    fn allocate(&self, kind: Kind, bytes: usize) -> usize {
        let count = self.state.borrow().nodes.len().saturating_add(1);
        if !self.admit(count, std::mem::size_of::<Node>().saturating_add(bytes)) {
            return NONE;
        }
        let mut state = self.state.borrow_mut();
        if state.nodes.try_reserve(1).is_err() {
            state.error = Some(SanitizeFailure::RejectedInput);
            return NONE;
        }
        let index = state.nodes.len();
        state.nodes.push(Node::new(kind));
        index
    }

    fn insert(&self, parent: usize, before: usize, child: NodeOrText<Handle>) {
        if parent == NONE || !self.charge_parse_work(1) {
            return;
        }
        let child = match child {
            NodeOrText::AppendNode(child) => child.index,
            NodeOrText::AppendText(text) => {
                let previous = {
                    let state = self.state.borrow();
                    if before == NONE {
                        state.nodes[parent].last
                    } else {
                        state.nodes[before].prev
                    }
                };
                if previous != NONE {
                    let is_text = matches!(self.state.borrow().nodes[previous].kind, Kind::Text(_));
                    if is_text {
                        let count = self.state.borrow().nodes.len();
                        if !self.admit(count, text.len()) {
                            return;
                        }
                        if let Kind::Text(existing) =
                            &mut self.state.borrow_mut().nodes[previous].kind
                        {
                            existing.push_tendril(&text);
                        }
                        return;
                    }
                }
                let bytes = text.len();
                self.allocate(Kind::Text(text), bytes)
            }
        };
        if child == NONE || self.failed() {
            return;
        }
        let mut state = self.state.borrow_mut();
        detach(&mut state.nodes, child);
        let previous = if before == NONE {
            state.nodes[parent].last
        } else {
            state.nodes[before].prev
        };
        state.nodes[child].parent = parent;
        state.nodes[child].prev = previous;
        state.nodes[child].next = before;
        if previous == NONE {
            state.nodes[parent].first = child;
        } else {
            state.nodes[previous].next = child;
        }
        if before == NONE {
            state.nodes[parent].last = child;
        } else {
            state.nodes[before].prev = child;
        }
    }
}

fn detach(nodes: &mut [Node], index: usize) {
    let parent = nodes[index].parent;
    if parent == NONE {
        return;
    }
    let prev = nodes[index].prev;
    let next = nodes[index].next;
    if prev == NONE {
        nodes[parent].first = next;
    } else {
        nodes[prev].next = next;
    }
    if next == NONE {
        nodes[parent].last = prev;
    } else {
        nodes[next].prev = prev;
    }
    nodes[index].parent = NONE;
    nodes[index].prev = NONE;
    nodes[index].next = NONE;
}

fn handle(index: usize, name: QualName) -> Handle {
    Handle {
        index,
        name,
        integration: false,
        attribute_bytes: 0,
    }
}
fn container_name() -> QualName {
    QualName::new(None, ns!(html), LocalName::from("html"))
}

impl<S: SanitizeOutputSink> TreeSink for Builder<'_, S> {
    type Handle = Handle;
    type Output = Self;
    type ElemName<'a>
        = &'a Handle
    where
        Self: 'a;

    fn finish(self) -> Self {
        self
    }
    fn parse_error(&self, _: Cow<'static, str>) {}
    fn get_document(&self) -> Handle {
        handle(0, container_name())
    }
    fn elem_name<'a>(&'a self, node: &'a Handle) -> &'a Handle {
        self.charge_parse_work(1);
        node
    }
    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        let attribute_bytes = attrs.iter().fold(0usize, |bytes, attr| {
            bytes
                .saturating_add(attr.name.local.len())
                .saturating_add(attr.value.len())
        });
        let bytes = attrs.iter().fold(name.local.len(), |total, attr| {
            total
                .saturating_add(std::mem::size_of::<Attribute>())
                .saturating_add(attr.name.local.len())
                .saturating_add(attr.value.len())
        });
        let index = self.allocate(
            Kind::Element {
                name: name.clone(),
                attrs,
            },
            bytes,
        );
        if index != NONE && flags.template {
            let template = self.allocate(Kind::Container, 0);
            self.state.borrow_mut().nodes[index].template = template;
        }
        Handle {
            index,
            name,
            integration: flags.mathml_annotation_xml_integration_point,
            attribute_bytes,
        }
    }
    fn create_comment(&self, _: StrTendril) -> Handle {
        handle(self.allocate(Kind::Ignored, 0), container_name())
    }
    fn create_pi(&self, _: StrTendril, _: StrTendril) -> Handle {
        handle(self.allocate(Kind::Ignored, 0), container_name())
    }
    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        self.insert(parent.index, NONE, child);
    }
    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        previous: &Handle,
        child: NodeOrText<Handle>,
    ) {
        if self.failed() || element.index == NONE {
            return;
        }
        let parent = self.state.borrow().nodes[element.index].parent;
        if parent == NONE {
            self.insert(previous.index, NONE, child);
        } else {
            self.insert(parent, element.index, child);
        }
    }
    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}
    fn get_template_contents(&self, node: &Handle) -> Handle {
        let index = if self.failed() || node.index == NONE {
            NONE
        } else {
            self.state.borrow().nodes[node.index].template
        };
        handle(index, container_name())
    }
    fn same_node(&self, a: &Handle, b: &Handle) -> bool {
        self.charge_parse_work(1);
        a.index == b.index
    }
    fn set_quirks_mode(&self, _: QuirksMode) {}
    fn append_before_sibling(&self, sibling: &Handle, node: NodeOrText<Handle>) {
        if self.failed() || sibling.index == NONE {
            return;
        }
        let parent = self.state.borrow().nodes[sibling.index].parent;
        self.insert(parent, sibling.index, node);
    }
    fn add_attrs_if_missing(&self, node: &Handle, attrs: Vec<Attribute>) {
        if self.failed() || node.index == NONE {
            return;
        }
        let bytes = attrs
            .iter()
            .map(|attr| {
                std::mem::size_of::<Attribute>()
                    + std::mem::size_of::<QualName>()
                    + attr.name.local.len()
                    + attr.value.len()
            })
            .sum();
        let count = self.state.borrow().nodes.len();
        if !self.admit(count, bytes) {
            return;
        }
        let mut state = self.state.borrow_mut();
        let State {
            nodes,
            merged_attribute_names,
            ..
        } = &mut *state;
        if let Kind::Element {
            attrs: existing, ..
        } = &mut nodes[node.index].kind
        {
            let names = merged_attribute_names
                .entry(node.index)
                .or_insert_with(|| existing.iter().map(|attr| attr.name.clone()).collect());
            for attr in attrs {
                if names.insert(attr.name.clone()) {
                    existing.push(attr);
                }
            }
        }
    }
    fn remove_from_parent(&self, node: &Handle) {
        if !self.failed() && node.index != NONE {
            detach(&mut self.state.borrow_mut().nodes, node.index);
        }
    }
    fn reparent_children(&self, node: &Handle, parent: &Handle) {
        if self.failed() || node.index == NONE || parent.index == NONE {
            return;
        }
        loop {
            let child = self.state.borrow().nodes[node.index].first;
            if child == NONE {
                break;
            }
            self.insert(
                parent.index,
                NONE,
                NodeOrText::AppendNode(handle(child, container_name())),
            );
            if self.failed() {
                break;
            }
        }
    }
    fn is_mathml_annotation_xml_integration_point(&self, node: &Handle) -> bool {
        self.charge_parse_work(1);
        node.integration
    }
}

#[derive(Clone, Copy)]
pub(super) enum Output {
    Html,
    Xhtml,
}

pub(super) fn sanitize<S: SanitizeOutputSink>(
    text: &str,
    cfg: &DompurifyEffectiveConfig,
    sink: &S,
    output: Output,
) -> Result<String, SanitizeFailure<S::Error>> {
    parse_and_serialize(text, Some(cfg), sink, output, ns!(html))
}

pub(super) fn xhtml_from_sanitized_svg_fragment<S: SanitizeOutputSink>(
    text: &str,
    sink: &S,
) -> Result<String, SanitizeFailure<S::Error>> {
    // Architecture assigns already sanitized HTML to an SVG div's innerHTML. The
    // browser then constructs a foreign-content fragment without sanitizing it again.
    parse_and_serialize(text, None, sink, Output::Xhtml, ns!(svg))
}

fn parse_and_serialize<S: SanitizeOutputSink>(
    text: &str,
    cfg: Option<&DompurifyEffectiveConfig>,
    sink: &S,
    output: Output,
    context: Namespace,
) -> Result<String, SanitizeFailure<S::Error>> {
    sink.checkpoint().map_err(SanitizeFailure::Output)?;
    sink.check_parse_storage(text.len(), 0, 0)
        .map_err(SanitizeFailure::Output)?;
    let builder = Builder {
        state: RefCell::new(State {
            nodes: vec![Node::new(Kind::Container)],
            retained: std::mem::size_of::<Node>(),
            error: None,
            merged_attribute_names: HashMap::new(),
            parse_work: 0,
            total_parse_work: 0,
            parse_work_limit: sink.parse_work_limit(text.len()),
        }),
        sink,
        input_bytes: text.len(),
    };
    let mut options = html5ever::ParseOpts::default();
    let tree_builder = if cfg.is_some() {
        // DOMPurify's default DOMParser uses an inert HTML document, then sanitizes
        // and returns its body. Head/template placement is not body-fragment parsing.
        options.tree_builder.scripting_enabled = false;
        TreeBuilder::new(builder, options.tree_builder)
    } else {
        let context_name = QualName::new(None, context.clone(), LocalName::from("svg"));
        let context_element =
            html5ever::tree_builder::create_element(&builder, context_name, Vec::new());
        let tree_builder =
            TreeBuilder::new_for_fragment(builder, context_element, None, options.tree_builder);
        options.tokenizer.initial_state =
            Some(tree_builder.tokenizer_state_for_context_elem(false));
        tree_builder
    };
    let parser = Tokenizer::new(
        StoppingTreeBuilder {
            inner: tree_builder,
            stopped: std::cell::Cell::new(false),
            complete_token: std::cell::Cell::new(false),
        },
        options.tokenizer,
    );
    let input_queue = BufferQueue::default();
    let mut cursor = 0;
    let mut pending_token_bytes: usize = 0;
    while cursor < text.len() {
        sink.checkpoint().map_err(SanitizeFailure::Output)?;
        let mut end = (cursor + CHUNK_BYTES).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        // The tokenizer checks each new attribute against the attributes already
        // present on its unfinished tag, before it emits any token or TreeSink call.
        // Prepay a conservative quadratic byte-work envelope for the current token;
        // completed tokens reset it, so ordinary sequences of short tags stay linear.
        let next_pending = pending_token_bytes.saturating_add(end - cursor);
        let token_work = next_pending
            .saturating_mul(next_pending)
            .div_ceil(WORK_BYTES_PER_UNIT)
            .saturating_sub(
                pending_token_bytes
                    .saturating_mul(pending_token_bytes)
                    .div_ceil(WORK_BYTES_PER_UNIT),
            );
        parser.sink.inner.sink.charge_parse_work(token_work);
        parser.sink.inner.sink.flush_parse_work();
        if let Some(error) = parser.sink.inner.sink.state.borrow_mut().error.take() {
            return Err(error);
        }
        parser.sink.complete_token.set(false);
        input_queue.push_back(StrTendril::from_slice(&text[cursor..end]));
        while !matches!(parser.feed(&input_queue), TokenizerResult::Done) {}
        pending_token_bytes = if parser.sink.complete_token.get() {
            // A complete token may be followed by an unfinished tag in this same
            // chunk. Retaining one entire chunk bounds that unknown trailing prefix.
            CHUNK_BYTES
        } else {
            next_pending
        };
        parser.sink.inner.sink.flush_parse_work();
        // TreeSink cannot return errors. A rejected callback preserves real element
        // names/identity until the current HTML5 token completes; subsequent tokens
        // are not forwarded to the tree builder. The tokenizer only drains this
        // bounded chunk before the parser is discarded without EOF or unwinding.
        if let Some(error) = parser.sink.inner.sink.state.borrow_mut().error.take() {
            return Err(error);
        }
        cursor = end;
    }
    parser.end();
    let builder = parser.sink.inner.sink.finish();
    builder.flush_parse_work();
    let root = if cfg.is_some() {
        builder.state.borrow().nodes.iter().position(|node| {
            matches!(&node.kind, Kind::Element { name, .. } if name.ns == ns!(html) && &*name.local == "body")
        }).unwrap_or(NONE)
    } else {
        builder.state.borrow().nodes[0].first
    };
    if cfg.is_some() && root != NONE && !builder.failed() {
        // DOMPurify restores precisely this prefix after DOMParser moves leading
        // whitespace out of the body (pinned _initDocument implementation).
        let prefix = text
            .bytes()
            .take_while(|byte| matches!(byte, b'\r' | b'\n' | b'\t' | b' '))
            .count();
        if prefix != 0 {
            let first = builder.state.borrow().nodes[root].first;
            builder.insert(
                root,
                first,
                NodeOrText::AppendText(StrTendril::from_slice(&text[..prefix])),
            );
        }
    }
    let mut state = builder.state.into_inner();
    if let Some(error) = state.error {
        return Err(error);
    }
    if root == NONE {
        return Ok(String::new());
    }
    serialize(&mut state.nodes, root, cfg, sink, output)
}

fn qualified_name(name: &QualName) -> Cow<'_, str> {
    match &name.prefix {
        Some(prefix) => Cow::Owned(format!("{prefix}:{}", name.local)),
        None => Cow::Borrowed(&name.local),
    }
}

fn is_void(name: &QualName) -> bool {
    name.ns == ns!(html)
        && matches!(
            name.local.as_ref(),
            "area"
                | "base"
                | "br"
                | "col"
                | "embed"
                | "hr"
                | "img"
                | "input"
                | "link"
                | "meta"
                | "param"
                | "source"
                | "track"
                | "wbr"
        )
}

fn is_xml_local_name(name: &str) -> bool {
    // XML 1.0 (Fifth Edition) NameStartChar/NameChar, excluding namespace colons.
    fn start(ch: char) -> bool {
        matches!(ch, 'A'..='Z' | '_' | 'a'..='z' | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}'
            | '\u{f8}'..='\u{2ff}' | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}' | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
    }
    let mut chars = name.chars();
    chars.next().is_some_and(start) && chars.all(|ch| start(ch) || matches!(ch, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'))
}

fn valid_namespace(name: &QualName, parent: Option<&QualName>, context: &Namespace) -> bool {
    use crate::generated::dompurify_defaults as defaults;
    let tag = name.local.to_ascii_lowercase();
    let tag = &*tag;
    let parent_ns = parent.map_or(context, |parent| &parent.ns);
    let parent_tag = parent.map_or("template", |parent| &*parent.local);
    if name.ns == ns!(svg) {
        if *parent_ns == ns!(html) {
            return tag == "svg";
        }
        if *parent_ns == ns!(mathml) {
            return tag == "svg"
                && (parent_tag == "annotation-xml"
                    || defaults::DEFAULT_MATHML_TEXT_INTEGRATION_POINTS.contains(&parent_tag));
        }
        return defaults::ALL_SVG_TAGS.contains(&tag);
    }
    if name.ns == ns!(mathml) {
        if *parent_ns == ns!(html) {
            return tag == "math";
        }
        if *parent_ns == ns!(svg) {
            return tag == "math"
                && defaults::DEFAULT_HTML_INTEGRATION_POINTS.contains(&parent_tag);
        }
        return defaults::ALL_MATHML_TAGS.contains(&tag);
    }
    if name.ns == ns!(html) {
        if *parent_ns == ns!(svg)
            && !defaults::DEFAULT_HTML_INTEGRATION_POINTS.contains(&parent_tag)
        {
            return false;
        }
        if *parent_ns == ns!(mathml)
            && !defaults::DEFAULT_MATHML_TEXT_INTEGRATION_POINTS.contains(&parent_tag)
        {
            return false;
        }
        return !defaults::ALL_MATHML_TAGS.contains(&tag)
            && (defaults::COMMON_SVG_AND_HTML_ELEMENTS.contains(&tag)
                || !defaults::ALL_SVG_TAGS.contains(&tag));
    }
    false
}

fn is_literal_html_tag(tag: &str) -> bool {
    matches!(
        tag,
        "style" | "script" | "xmp" | "iframe" | "noembed" | "noframes" | "plaintext" | "noscript"
    )
}

fn unsafe_literal_html<S: SanitizeOutputSink>(
    nodes: &[Node],
    index: usize,
    sink: &S,
) -> Result<bool, SanitizeFailure<S::Error>> {
    // DOMPurify SAFE_FOR_XML rejects element children or markup-like literal text,
    // including noscript. Match across text nodes without concatenation or recursion.
    let mut child = nodes[index].first;
    let mut markup_start = false;
    while child != NONE {
        sink.checkpoint().map_err(SanitizeFailure::Output)?;
        match &nodes[child].kind {
            Kind::Element { .. } => return Ok(true),
            Kind::Text(text) => {
                for (offset, byte) in text.bytes().enumerate() {
                    if offset % CHUNK_BYTES == 0 {
                        sink.checkpoint().map_err(SanitizeFailure::Output)?;
                    }
                    if markup_start
                        && (byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'/' | b'!'))
                    {
                        return Ok(true);
                    }
                    markup_start = byte == b'<';
                }
            }
            _ => {}
        }
        child = nodes[child].next;
    }
    Ok(false)
}

struct Writer<'a, S> {
    value: Vec<u8>,
    sink: &'a S,
}
impl<S: SanitizeOutputSink> Writer<'_, S> {
    fn push(&mut self, value: &str) -> Result<(), SanitizeFailure<S::Error>> {
        self.sink
            .push_output_chunk(&mut self.value, value.as_bytes())
            .map_err(SanitizeFailure::Output)
    }
    fn name(&mut self, name: &QualName) -> Result<(), SanitizeFailure<S::Error>> {
        if let Some(prefix) = &name.prefix {
            self.push(prefix)?;
            self.push(":")?;
        }
        self.push(&name.local)
    }
    fn escaped(
        &mut self,
        text: &str,
        attr: bool,
        escape_gt: bool,
        xhtml: bool,
    ) -> Result<(), SanitizeFailure<S::Error>> {
        let mut start = 0;
        for (i, (offset, ch)) in text.char_indices().enumerate() {
            if i % 64 == 0 {
                self.sink.checkpoint().map_err(SanitizeFailure::Output)?;
            }
            let replacement = match ch {
                '&' => Some("&amp;"),
                '<' => Some("&lt;"),
                '>' if escape_gt || xhtml => Some("&gt;"),
                '"' if attr => Some("&quot;"),
                '\n' if attr && xhtml => Some("&#10;"),
                '\r' if attr && xhtml => Some("&#13;"),
                '\t' if attr && xhtml => Some("&#9;"),
                '\u{a0}' if !xhtml => Some("&nbsp;"),
                '\u{0}'..='\u{8}'
                | '\u{b}'
                | '\u{c}'
                | '\u{e}'..='\u{1f}'
                | '\u{fffe}'
                | '\u{ffff}'
                    if xhtml =>
                {
                    Some("")
                }
                _ => None,
            };
            if let Some(replacement) = replacement {
                self.push(&text[start..offset])?;
                self.push(replacement)?;
                start = offset + ch.len_utf8();
            }
        }
        self.push(&text[start..])
    }
}

fn serialize<S: SanitizeOutputSink>(
    nodes: &mut [Node],
    root: usize,
    cfg: Option<&DompurifyEffectiveConfig>,
    sink: &S,
    mode: Output,
) -> Result<String, SanitizeFailure<S::Error>> {
    let xhtml = matches!(mode, Output::Xhtml);
    // All XHTML fragments are embedded beneath an XHTML parent, independently of
    // the HTML5 parsing context that produced their expanded element names.
    let context = ns!(html);
    let mut out = Writer {
        value: sink.output_buffer(0).map_err(SanitizeFailure::Output)?,
        sink,
    };
    // The HTML5 fragment builder's synthetic html root is not part of the fragment.
    let mut stack = Vec::new();
    let mut child = nodes[root].last;
    while child != NONE {
        stack.push((child, false, None));
        child = nodes[child].prev;
    }
    while let Some((index, close, parent_name)) = stack.pop() {
        sink.checkpoint().map_err(SanitizeFailure::Output)?;
        if !close
            && cfg.is_some_and(|cfg| cfg.safe_for_xml)
            && let Kind::Element { name, .. } = &nodes[index].kind
            && name.ns == ns!(html)
            && is_literal_html_tag(&name.local)
            && unsafe_literal_html(nodes, index, sink)?
        {
            continue;
        }
        let parent_ns = parent_name
            .as_ref()
            .map_or(&context, |name: &QualName| &name.ns);
        let mut child_parent = parent_name.clone();
        let mut descend = true;
        let raw_html_text = !xhtml
            && parent_name.as_ref().is_some_and(|name| {
                name.ns == ns!(html)
                    && matches!(
                        &*name.local,
                        "style"
                            | "script"
                            | "xmp"
                            | "iframe"
                            | "noembed"
                            | "noframes"
                            | "plaintext"
                    )
            });
        match &mut nodes[index].kind {
            Kind::Text(text) if raw_html_text => out.push(text)?,
            Kind::Text(text) => out.escaped(text, false, true, xhtml)?,
            Kind::Element { name, attrs } => {
                let local = name.local.to_ascii_lowercase();
                if close {
                    out.push("</")?;
                    out.name(name)?;
                    out.push(">")?;
                    continue;
                }
                let allowed = cfg.is_none_or(|cfg| {
                    cfg.allowed_tags.contains(local.as_str())
                        && !cfg.forbid_tags.contains(local.as_str())
                });
                if cfg.is_some_and(|cfg| {
                    matches!(local.as_str(), "script" | "iframe" | "style")
                        || (!allowed
                            && (!cfg.keep_content || cfg.forbid_contents.contains(local.as_str())))
                }) {
                    descend = false;
                } else if allowed {
                    if cfg.is_some() && !valid_namespace(name, parent_name.as_ref(), &context) {
                        continue;
                    }
                    if xhtml && !is_xml_local_name(&name.local) {
                        return Err(SanitizeFailure::RejectedInput);
                    }
                    if xhtml {
                        attrs.retain(|attr| {
                            is_xml_local_name(&attr.name.local)
                                && attr.name.prefix.as_ref().is_none_or(|prefix| {
                                    matches!(&**prefix, "xml" | "xlink" | "xmlns")
                                })
                        });
                    }
                    if let Some(cfg) = cfg {
                        let target = (&*local == "a")
                            .then(|| {
                                attrs
                                    .iter()
                                    .find(|a| &*a.name.local == "target")
                                    .map(|a| a.value.clone())
                            })
                            .flatten();
                        if let Some(target) = target {
                            if let Some(existing) = attrs
                                .iter_mut()
                                .find(|attr| &*attr.name.local == "data-temp-href-target")
                            {
                                existing.value = target;
                            } else {
                                attrs.push(Attribute {
                                    name: QualName::new(
                                        None,
                                        ns!(),
                                        LocalName::from("data-temp-href-target"),
                                    ),
                                    value: target,
                                });
                            }
                        }
                        attrs.retain_mut(|attr| {
                            let key = qualified_name(&attr.name);
                            let key = if key.bytes().any(|byte| byte.is_ascii_uppercase()) {
                                Cow::Owned(key.to_ascii_lowercase())
                            } else {
                                key
                            };
                            let value =
                                super::dompurify_normalize_dom_attribute_value(&key, &attr.value);
                            if !super::dompurify_is_valid_attribute(cfg, &local, &key, value) {
                                return false;
                            }
                            if value != attr.value.as_ref() {
                                attr.value = StrTendril::from_slice(value);
                            }
                            true
                        });
                        if &*local == "a"
                            && let Some(i) = attrs
                                .iter()
                                .position(|a| &*a.name.local == "data-temp-href-target")
                        {
                            let value = attrs.remove(i).value;
                            attrs.retain(|a| {
                                &*a.name.local != "target"
                                    && (value.as_ref() != "_blank" || &*a.name.local != "rel")
                            });
                            attrs.push(Attribute {
                                name: QualName::new(None, ns!(), LocalName::from("target")),
                                value: value.clone(),
                            });
                            if value.as_ref() == "_blank" {
                                attrs.push(Attribute {
                                    name: QualName::new(None, ns!(), LocalName::from("rel")),
                                    value: StrTendril::from_slice("noopener"),
                                });
                            }
                        }
                    }
                    out.push("<")?;
                    out.name(name)?;
                    if xhtml
                        && (name.ns != *parent_ns
                            || attrs.iter().any(|attr| &*attr.name.local == "xmlns"))
                    {
                        out.push(" xmlns=\"")?;
                        out.escaped(name.ns.as_ref(), true, false, true)?;
                        out.push("\"")?;
                    }
                    let mut wrote_xlink = false;
                    for attr in attrs.iter() {
                        if xhtml && (&*attr.name.local == "xmlns" || attr.name.ns == ns!(xmlns)) {
                            continue;
                        }
                        if xhtml && attr.name.ns == ns!(xlink) && !wrote_xlink {
                            out.push(" xmlns:xlink=\"http://www.w3.org/1999/xlink\"")?;
                            wrote_xlink = true;
                        }
                        out.push(" ")?;
                        out.name(&attr.name)?;
                        out.push("=\"")?;
                        out.escaped(&attr.value, true, false, xhtml)?;
                        out.push("\"")?;
                    }
                    if is_void(name) {
                        out.push(if xhtml { " />" } else { ">" })?;
                    } else {
                        out.push(">")?;
                        stack.push((index, true, parent_name.clone()));
                    }
                    child_parent = Some(name.clone());
                }
            }
            Kind::Ignored => descend = false,
            Kind::Container => {}
        }
        if descend {
            let contents = nodes[index].template;
            let mut child = if contents == NONE {
                nodes[index].last
            } else {
                nodes[contents].last
            };
            while child != NONE {
                stack.push((child, false, child_parent.clone()));
                child = nodes[child].prev;
            }
        }
    }
    String::from_utf8(out.value).map_err(|_| SanitizeFailure::InvalidUtf8Output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_tree_does_not_process_subsequent_tokens() {
        let output_sink = super::super::StringOutputSink;
        let builder = Builder {
            state: RefCell::new(State {
                nodes: vec![Node::new(Kind::Container)],
                retained: std::mem::size_of::<Node>(),
                error: None,
                merged_attribute_names: HashMap::new(),
                parse_work: 0,
                total_parse_work: 0,
                parse_work_limit: None,
            }),
            sink: &output_sink,
            input_bytes: 64,
        };
        let stopping = StoppingTreeBuilder {
            inner: TreeBuilder::new(builder, Default::default()),
            stopped: std::cell::Cell::new(false),
            complete_token: std::cell::Cell::new(false),
        };
        let token = || {
            html5ever::tokenizer::Token::TagToken(html5ever::tokenizer::Tag {
                kind: html5ever::tokenizer::StartTag,
                name: LocalName::from("div"),
                self_closing: false,
                attrs: Vec::new(),
                had_duplicate_attributes: false,
            })
        };
        let _ = stopping.process_token(token(), 1);
        let before = {
            let state = stopping.inner.sink.state.borrow();
            (state.nodes.len(), state.total_parse_work)
        };
        stopping.inner.sink.state.borrow_mut().error = Some(SanitizeFailure::RejectedInput);
        for _ in 0..8 {
            assert!(matches!(
                stopping.process_token(token(), 1),
                TokenSinkResult::Continue
            ));
        }
        stopping.end();
        let state = stopping.inner.sink.state.borrow();
        assert!(stopping.stopped.get());
        assert_eq!((state.nodes.len(), state.total_parse_work), before);
        assert!(matches!(state.error, Some(SanitizeFailure::RejectedInput)));
    }
}
