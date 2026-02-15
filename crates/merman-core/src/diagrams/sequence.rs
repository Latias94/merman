use crate::{Error, ParseMetadata, Result};
use rustc_hash::FxHashMap;
use serde_json::{Value, json};
use std::collections::VecDeque;

lalrpop_util::lalrpop_mod!(
    #[allow(clippy::type_complexity, clippy::result_large_err)]
    sequence_grammar,
    "/diagrams/sequence_grammar.rs"
);

// Mermaid 11.12.x sequence diagram constants (SequenceDB.LINETYPE / PLACEMENT).
const LINETYPE_NOTE: i32 = 2;
const LINETYPE_LOOP_START: i32 = 10;
const LINETYPE_LOOP_END: i32 = 11;
const LINETYPE_ALT_START: i32 = 12;
const LINETYPE_ALT_ELSE: i32 = 13;
const LINETYPE_ALT_END: i32 = 14;
const LINETYPE_OPT_START: i32 = 15;
const LINETYPE_OPT_END: i32 = 16;
const LINETYPE_ACTIVE_START: i32 = 17;
const LINETYPE_ACTIVE_END: i32 = 18;
const LINETYPE_PAR_START: i32 = 19;
const LINETYPE_PAR_AND: i32 = 20;
const LINETYPE_PAR_END: i32 = 21;
const LINETYPE_RECT_START: i32 = 22;
const LINETYPE_RECT_END: i32 = 23;
const LINETYPE_AUTONUMBER: i32 = 26;
const LINETYPE_CRITICAL_START: i32 = 27;
const LINETYPE_CRITICAL_OPTION: i32 = 28;
const LINETYPE_CRITICAL_END: i32 = 29;
const LINETYPE_BREAK_START: i32 = 30;
const LINETYPE_BREAK_END: i32 = 31;
const LINETYPE_PAR_OVER_START: i32 = 32;

const PLACEMENT_LEFT_OF: i32 = 0;
const PLACEMENT_RIGHT_OF: i32 = 1;
const PLACEMENT_OVER: i32 = 2;

#[derive(Debug, Clone)]
pub(crate) enum Tok {
    Newline,

    SequenceDiagram,
    Participant,
    ActorKw,
    Create,
    Destroy,
    As,

    Box,
    End,

    Loop,
    Rect,
    Opt,
    Alt,
    Else,
    Par,
    ParOver,
    And,
    Critical,
    Option,
    Break,

    Note,
    LeftOf,
    RightOf,
    Over,

    Links,
    Link,
    Properties,
    Details,

    Autonumber,
    Off,

    Activate,
    Deactivate,

    Comma,
    Plus,
    Minus,

    Num(i64),
    Actor(String),
    Text(String),
    RestOfLine(String),
    SignalType(i32),
    Config(String),

    Title(String),
    CompatTitle(String),
    AccTitle(String),
    AccDescr(String),
    AccDescrMultiline(String),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub(crate) struct LexError {
    pub message: String,
}

#[derive(Debug, Clone)]
pub(crate) enum Action {
    SetTitle(String),
    SetAccTitle(String),
    SetAccDescr(String),

    EnsureParticipant {
        id: String,
    },
    AddParticipant {
        id: String,
        description: Option<String>,
        draw: String,
        config: Option<String>,
    },

    CreateParticipant {
        id: String,
        description: Option<String>,
        draw: String,
        config: Option<String>,
    },
    DestroyParticipant {
        id: String,
    },

    ControlSignal {
        signal_type: i32,
        text: Option<String>,
    },

    BoxStart {
        header: String,
    },
    BoxEnd,

    AddLinks {
        actor: String,
        text: String,
    },
    AddLink {
        actor: String,
        text: String,
    },
    AddProperties {
        actor: String,
        text: String,
    },
    AddDetails {
        actor: String,
        text: String,
    },

    AddMessage {
        from: String,
        to: String,
        signal_type: i32,
        text: String,
        activate: bool,
    },
    ActiveStart {
        actor: String,
    },
    ActiveEnd {
        actor: String,
    },

    AddNote {
        actors: Vec<String>,
        placement: i32,
        text: String,
    },

    Autonumber {
        start: Option<i64>,
        step: Option<i64>,
        visible: bool,
    },
}

#[derive(Debug, Clone)]
struct ParsedText {
    text: String,
    wrap: Option<bool>,
}

#[derive(Debug, Clone)]
struct Actor {
    name: String,
    description: String,
    wrap: bool,
    actor_type: String,
    box_index: Option<usize>,
    links: serde_json::Map<String, Value>,
    properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone)]
struct Message {
    id: String,
    from: Option<String>,
    to: Option<String>,
    message: Value,
    wrap: bool,
    message_type: i32,
    activate: bool,
    placement: Option<i32>,
}

#[derive(Debug, Clone)]
struct Note {
    actor: Value,
    placement: i32,
    message: String,
    wrap: bool,
}

#[derive(Debug, Clone)]
struct SeqBox {
    name: Option<String>,
    fill: String,
    wrap: bool,
    actor_keys: Vec<String>,
}

#[derive(Debug, Default)]
struct SequenceDb {
    actors: FxHashMap<String, Actor>,
    actor_order: Vec<String>,
    messages: Vec<Message>,
    notes: Vec<Note>,
    boxes: Vec<SeqBox>,
    current_box: Option<usize>,
    wrap_enabled: Option<bool>,

    created_actors: FxHashMap<String, usize>,
    destroyed_actors: FxHashMap<String, usize>,
    last_created: Option<String>,
    last_destroyed: Option<String>,

    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
}

impl SequenceDb {
    fn new(wrap_enabled: Option<bool>) -> Self {
        Self {
            wrap_enabled,
            ..Default::default()
        }
    }

    fn auto_wrap(&self) -> bool {
        self.wrap_enabled.unwrap_or(false)
    }

    fn ensure_actor(&mut self, id: &str) {
        if self.actors.contains_key(id) {
            return;
        }
        let id_owned = id.to_string();
        self.actor_order.push(id_owned.clone());
        self.actors.insert(
            id_owned.clone(),
            Actor {
                name: id_owned.clone(),
                description: id_owned,
                wrap: self.auto_wrap(),
                actor_type: "participant".to_string(),
                box_index: None,
                links: serde_json::Map::new(),
                properties: serde_json::Map::new(),
            },
        );
    }

    fn add_actor(
        &mut self,
        id: &str,
        description: Option<String>,
        actor_type: &str,
        config: Option<String>,
    ) -> std::result::Result<(), String> {
        let mut actor_type = actor_type.to_string();
        if let Some(config) = config.as_deref() {
            let meta = parse_participant_meta_yaml(config)?;
            if let serde_yaml::Value::Mapping(m) = meta {
                let key = serde_yaml::Value::String("type".to_string());
                if let Some(v) = m.get(&key) {
                    if let Some(t) = yaml_to_string(v) {
                        actor_type = t;
                    }
                }
            }
        }

        if let Some(current_box) = self.current_box {
            if let Some(existing) = self.actors.get(id) {
                if let Some(old_box) = existing.box_index {
                    if old_box != current_box {
                        let old_name = self.boxes[old_box]
                            .name
                            .clone()
                            .unwrap_or_else(|| "undefined".to_string());
                        let new_name = self.boxes[current_box]
                            .name
                            .clone()
                            .unwrap_or_else(|| "undefined".to_string());
                        return Err(format!(
                            "A same participant should only be defined in one Box: {} can't be in '{}' and in '{}' at the same time.",
                            existing.name, old_name, new_name
                        ));
                    }
                }
            }
        }

        let description = description
            .map(|s| self.parse_message(&s))
            .unwrap_or_else(|| ParsedText {
                text: id.to_string(),
                wrap: None,
            });

        let wrap = description.wrap.unwrap_or(self.auto_wrap());

        if let Some(existing) = self.actors.get_mut(id) {
            existing.description = description.text;
            existing.wrap = wrap;
            existing.actor_type = actor_type;
            if let Some(current_box) = self.current_box {
                if existing.box_index.is_none() {
                    existing.box_index = Some(current_box);
                }
                self.boxes[current_box].actor_keys.push(id.to_string());
            }
            return Ok(());
        }

        self.actor_order.push(id.to_string());
        self.actors.insert(
            id.to_string(),
            Actor {
                name: id.to_string(),
                description: description.text,
                wrap,
                actor_type,
                box_index: self.current_box,
                links: serde_json::Map::new(),
                properties: serde_json::Map::new(),
            },
        );

        if let Some(current_box) = self.current_box {
            self.boxes[current_box].actor_keys.push(id.to_string());
        }

        Ok(())
    }

    fn parse_message(&self, raw: &str) -> ParsedText {
        let trimmed = raw.trim();
        fn strip_prefix_ci<'a>(s: &'a str, prefix: &[u8]) -> Option<&'a str> {
            let bytes = s.as_bytes();
            if bytes.len() < prefix.len() {
                return None;
            }
            for i in 0..prefix.len() {
                if !bytes[i].eq_ignore_ascii_case(&prefix[i]) {
                    return None;
                }
            }
            Some(&s[prefix.len()..])
        }

        let (wrap, cleaned) = if trimmed.len() >= 5
            && matches!(
                trimmed.as_bytes().first().copied(),
                Some(b':' | b'w' | b'W' | b'n' | b'N')
            ) {
            if let Some(rest) = strip_prefix_ci(trimmed, b":wrap:") {
                (Some(true), rest.trim())
            } else if let Some(rest) = strip_prefix_ci(trimmed, b"wrap:") {
                (Some(true), rest.trim())
            } else if let Some(rest) = strip_prefix_ci(trimmed, b":nowrap:") {
                (Some(false), rest.trim())
            } else if let Some(rest) = strip_prefix_ci(trimmed, b"nowrap:") {
                (Some(false), rest.trim())
            } else {
                (None, trimmed)
            }
        } else {
            (None, trimmed)
        };

        ParsedText {
            text: cleaned.to_string(),
            wrap,
        }
    }

    fn add_signal(
        &mut self,
        from: Option<String>,
        to: Option<String>,
        message: Option<ParsedText>,
        message_type: i32,
        activate: bool,
        placement: Option<i32>,
    ) {
        let msg_text = message.unwrap_or(ParsedText {
            text: String::new(),
            wrap: None,
        });
        let wrap = msg_text.wrap.unwrap_or(self.auto_wrap());

        self.messages.push(Message {
            id: self.messages.len().to_string(),
            from,
            to,
            message: Value::String(msg_text.text),
            wrap,
            message_type,
            activate,
            placement,
        });
    }

    fn activation_count(&self, actor: &str) -> i32 {
        if actor.is_empty() {
            return 0;
        }
        let mut count = 0;
        for msg in &self.messages {
            if msg.message_type == LINETYPE_ACTIVE_START
                && msg.from.as_deref().is_some_and(|a| a == actor)
            {
                count += 1;
            }
            if msg.message_type == LINETYPE_ACTIVE_END
                && msg.from.as_deref().is_some_and(|a| a == actor)
            {
                count -= 1;
            }
        }
        count
    }

    fn add_autonumber(&mut self, start: Option<i64>, step: Option<i64>, visible: bool) {
        let mut msg = serde_json::Map::new();
        if let Some(s) = start {
            msg.insert("start".to_string(), Value::Number(s.into()));
        }
        if let Some(s) = step {
            msg.insert("step".to_string(), Value::Number(s.into()));
        }
        msg.insert("visible".to_string(), Value::Bool(visible));

        self.messages.push(Message {
            id: self.messages.len().to_string(),
            from: None,
            to: None,
            message: Value::Object(msg),
            wrap: false,
            message_type: LINETYPE_AUTONUMBER,
            activate: false,
            placement: None,
        });
    }

    fn add_note(&mut self, actors: Vec<String>, placement: i32, raw_text: String) {
        let parsed = self.parse_message(&raw_text);
        let wrap = parsed.wrap.unwrap_or(self.auto_wrap());

        let actor_value = match actors.as_slice() {
            [a] => Value::String(a.clone()),
            [a, b] => json!([a, b]),
            _ => json!(actors),
        };

        self.notes.push(Note {
            actor: actor_value.clone(),
            placement,
            message: parsed.text.clone(),
            wrap,
        });

        let (from, to) = match actors.as_slice() {
            [a] => (Some(a.clone()), Some(a.clone())),
            [a, b] => (Some(a.clone()), Some(b.clone())),
            _ => (
                actors.first().cloned(),
                actors.get(1).cloned().or_else(|| actors.first().cloned()),
            ),
        };

        self.messages.push(Message {
            id: self.messages.len().to_string(),
            from,
            to,
            message: Value::String(parsed.text),
            wrap,
            message_type: LINETYPE_NOTE,
            activate: false,
            placement: Some(placement),
        });
    }

    fn apply(&mut self, action: Action) -> std::result::Result<(), String> {
        match action {
            Action::SetTitle(t) => {
                self.title = Some(t.trim().to_string());
                Ok(())
            }
            Action::SetAccTitle(t) => {
                self.acc_title = Some(t.trim().to_string());
                Ok(())
            }
            Action::SetAccDescr(t) => {
                self.acc_descr = Some(t.trim().to_string());
                Ok(())
            }

            Action::EnsureParticipant { id } => {
                self.ensure_actor(&id);
                Ok(())
            }
            Action::AddParticipant {
                id,
                description,
                draw,
                config,
            } => self.add_actor(&id, description, &draw, config),

            Action::CreateParticipant {
                id,
                description,
                draw,
                config,
            } => {
                if self.actors.contains_key(&id) {
                    return Err("It is not possible to have actors with the same id, even if one is destroyed before the next is created. Use 'AS' aliases to simulate the behavior".to_string());
                }
                self.last_created = Some(id.clone());
                self.add_actor(&id, description, &draw, config)?;
                self.created_actors.insert(id, self.messages.len());
                Ok(())
            }
            Action::DestroyParticipant { id } => {
                self.last_destroyed = Some(id.clone());
                self.destroyed_actors.insert(id, self.messages.len());
                Ok(())
            }

            Action::ControlSignal { signal_type, text } => {
                let msg = text.as_deref().map(|t| self.parse_message(t));
                self.add_signal(None, None, msg, signal_type, false, None);
                Ok(())
            }

            Action::BoxStart { header } => {
                self.add_box(&header);
                Ok(())
            }
            Action::BoxEnd => {
                self.current_box = None;
                Ok(())
            }

            Action::AddLinks { actor, text } => {
                self.add_links(&actor, &text);
                Ok(())
            }
            Action::AddLink { actor, text } => {
                self.add_link(&actor, &text);
                Ok(())
            }
            Action::AddProperties { actor, text } => {
                self.add_properties(&actor, &text);
                Ok(())
            }
            Action::AddDetails { actor, text } => {
                let _ = (actor, text);
                Ok(())
            }

            Action::AddMessage {
                from,
                to,
                signal_type,
                text,
                activate,
            } => {
                if let Some(last_created) = self.last_created.clone() {
                    if to != last_created {
                        return Err(format!(
                            "The created participant {last_created} does not have an associated creating message after its declaration. Please check the sequence diagram."
                        ));
                    }
                    self.last_created = None;
                } else if let Some(last_destroyed) = self.last_destroyed.clone() {
                    if from != last_destroyed && to != last_destroyed {
                        return Err(format!(
                            "The destroyed participant {last_destroyed} does not have an associated destroying message after its declaration. Please check the sequence diagram."
                        ));
                    }
                    self.last_destroyed = None;
                }

                let msg = self.parse_message(&text);
                self.add_signal(Some(from), Some(to), Some(msg), signal_type, activate, None);
                Ok(())
            }

            Action::ActiveStart { actor } => {
                self.add_signal(Some(actor), None, None, LINETYPE_ACTIVE_START, false, None);
                Ok(())
            }
            Action::ActiveEnd { actor } => {
                if self.activation_count(&actor) < 1 {
                    return Err(format!(
                        "Trying to inactivate an inactive participant ({actor})"
                    ));
                }
                self.add_signal(Some(actor), None, None, LINETYPE_ACTIVE_END, false, None);
                Ok(())
            }

            Action::AddNote {
                actors,
                placement,
                text,
            } => {
                self.add_note(actors, placement, text);
                Ok(())
            }

            Action::Autonumber {
                start,
                step,
                visible,
            } => {
                self.add_autonumber(start, step, visible);
                Ok(())
            }
        }
    }

    fn add_box(&mut self, raw: &str) {
        let data = self.parse_box_data(raw);
        let wrap = data.wrap.unwrap_or(self.auto_wrap());
        self.boxes.push(SeqBox {
            name: data.text,
            fill: data.color,
            wrap,
            actor_keys: Vec::new(),
        });
        self.current_box = Some(self.boxes.len() - 1);
    }

    fn parse_box_data(&self, raw: &str) -> BoxData {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return BoxData {
                text: None,
                color: "transparent".to_string(),
                wrap: None,
            };
        }

        let (color_candidate, title_candidate) = split_color_and_title(trimmed);
        let mut color = if color_candidate.trim().is_empty() {
            "transparent".to_string()
        } else {
            color_candidate.trim().to_string()
        };
        let mut title = title_candidate.trim().to_string();

        if !is_css_color_value(&color) {
            color = "transparent".to_string();
            title = trimmed.to_string();
        }

        let parsed_title = self.parse_message(&title);
        let text = if parsed_title.text.is_empty() {
            None
        } else {
            Some(parsed_title.text)
        };

        BoxData {
            text,
            color,
            wrap: parsed_title.wrap,
        }
    }

    fn add_links(&mut self, actor: &str, raw_text: &str) {
        let s = unescape_entities(raw_text);
        let Ok(v) = serde_json::from_str::<Value>(&s) else {
            return;
        };
        let Some(obj) = v.as_object() else {
            return;
        };
        let Some(a) = self.actors.get_mut(actor) else {
            return;
        };
        for (k, v) in obj {
            if let Some(url) = v.as_str() {
                a.links.insert(k.clone(), Value::String(url.to_string()));
            }
        }
    }

    fn add_link(&mut self, actor: &str, raw_text: &str) {
        let s = unescape_entities(raw_text);
        let Some(idx) = s.find('@') else {
            return;
        };
        let (left, right) = s.split_at(idx);
        let label = left.strip_suffix(' ').unwrap_or(left).trim();
        let url = right.trim_start_matches('@').trim();
        if label.is_empty() || url.is_empty() {
            return;
        }
        let Some(a) = self.actors.get_mut(actor) else {
            return;
        };
        a.links
            .insert(label.to_string(), Value::String(url.to_string()));
    }

    fn add_properties(&mut self, actor: &str, raw_text: &str) {
        let s = unescape_entities(raw_text);
        let Ok(v) = serde_json::from_str::<Value>(&s) else {
            return;
        };
        let Some(obj) = v.as_object() else {
            return;
        };
        let Some(a) = self.actors.get_mut(actor) else {
            return;
        };
        for (k, v) in obj {
            a.properties.insert(k.clone(), v.clone());
        }
    }

    fn into_model(mut self, meta: &ParseMetadata) -> Value {
        fn opt_string(v: Option<String>) -> Value {
            v.map(Value::String).unwrap_or(Value::Null)
        }

        let mut actors = std::mem::take(&mut self.actors);
        let mut actors_json = serde_json::Map::new();
        let mut actor_order_json: Vec<Value> = Vec::with_capacity(self.actor_order.len());
        for id in std::mem::take(&mut self.actor_order) {
            actor_order_json.push(Value::String(id.clone()));
            if let Some(a) = actors.remove(&id) {
                let mut obj = serde_json::Map::with_capacity(6);
                obj.insert("name".to_string(), Value::String(a.name));
                obj.insert("description".to_string(), Value::String(a.description));
                obj.insert("wrap".to_string(), Value::Bool(a.wrap));
                obj.insert("type".to_string(), Value::String(a.actor_type));
                obj.insert("links".to_string(), Value::Object(a.links));
                obj.insert("properties".to_string(), Value::Object(a.properties));
                actors_json.insert(id, Value::Object(obj));
            }
        }

        let messages_json: Vec<Value> = std::mem::take(&mut self.messages)
            .into_iter()
            .map(|m| {
                let mut obj = serde_json::Map::new();
                obj.insert("id".to_string(), Value::String(m.id));
                obj.insert(
                    "from".to_string(),
                    m.from.map(Value::String).unwrap_or(Value::Null),
                );
                obj.insert(
                    "to".to_string(),
                    m.to.map(Value::String).unwrap_or(Value::Null),
                );
                obj.insert("message".to_string(), m.message);
                obj.insert("wrap".to_string(), Value::Bool(m.wrap));
                obj.insert("type".to_string(), Value::Number(m.message_type.into()));
                obj.insert("activate".to_string(), Value::Bool(m.activate));
                if let Some(p) = m.placement {
                    obj.insert("placement".to_string(), Value::Number(p.into()));
                }
                Value::Object(obj)
            })
            .collect();

        let notes_json: Vec<Value> = std::mem::take(&mut self.notes)
            .into_iter()
            .map(|n| {
                let mut obj = serde_json::Map::with_capacity(4);
                obj.insert("actor".to_string(), n.actor);
                obj.insert("placement".to_string(), Value::Number(n.placement.into()));
                obj.insert("message".to_string(), Value::String(n.message));
                obj.insert("wrap".to_string(), Value::Bool(n.wrap));
                Value::Object(obj)
            })
            .collect();

        let boxes_json: Vec<Value> = std::mem::take(&mut self.boxes)
            .into_iter()
            .map(|b| {
                let mut obj = serde_json::Map::with_capacity(4);
                obj.insert("name".to_string(), opt_string(b.name));
                obj.insert("wrap".to_string(), Value::Bool(b.wrap));
                obj.insert("fill".to_string(), Value::String(b.fill));
                obj.insert(
                    "actorKeys".to_string(),
                    Value::Array(b.actor_keys.into_iter().map(Value::String).collect()),
                );
                Value::Object(obj)
            })
            .collect();

        let created_json: serde_json::Map<String, Value> = std::mem::take(&mut self.created_actors)
            .into_iter()
            .map(|(k, v)| (k, Value::Number((v as u64).into())))
            .collect();

        let destroyed_json: serde_json::Map<String, Value> =
            std::mem::take(&mut self.destroyed_actors)
                .into_iter()
                .map(|(k, v)| (k, Value::Number((v as u64).into())))
                .collect();

        let mut placement = serde_json::Map::with_capacity(3);
        placement.insert(
            "leftOf".to_string(),
            Value::Number(PLACEMENT_LEFT_OF.into()),
        );
        placement.insert(
            "rightOf".to_string(),
            Value::Number(PLACEMENT_RIGHT_OF.into()),
        );
        placement.insert("over".to_string(), Value::Number(PLACEMENT_OVER.into()));
        let mut constants = serde_json::Map::with_capacity(1);
        constants.insert("placement".to_string(), Value::Object(placement));

        let mut root = serde_json::Map::with_capacity(11);
        root.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
        root.insert("title".to_string(), opt_string(self.title));
        root.insert("accTitle".to_string(), opt_string(self.acc_title));
        root.insert("accDescr".to_string(), opt_string(self.acc_descr));
        root.insert("actorOrder".to_string(), Value::Array(actor_order_json));
        root.insert("actors".to_string(), Value::Object(actors_json));
        root.insert("messages".to_string(), Value::Array(messages_json));
        root.insert("notes".to_string(), Value::Array(notes_json));
        root.insert("boxes".to_string(), Value::Array(boxes_json));
        root.insert("createdActors".to_string(), Value::Object(created_json));
        root.insert("destroyedActors".to_string(), Value::Object(destroyed_json));
        root.insert("constants".to_string(), Value::Object(constants));

        Value::Object(root)
    }
}

pub fn parse_sequence(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let wrap_enabled = meta
        .effective_config
        .as_value()
        .get("wrap")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            meta.effective_config
                .as_value()
                .get("sequence")
                .and_then(|v| v.get("wrap"))
                .and_then(|v| v.as_bool())
        });

    if let Some(v) = fast_parse_sequence_signals_only(code, wrap_enabled, meta) {
        return Ok(v);
    }

    let actions = sequence_grammar::ActionsParser::new()
        .parse(Lexer::new(code))
        .map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("{e:?}"),
        })?;

    let mut db = SequenceDb::new(wrap_enabled);
    for a in actions {
        db.apply(a).map_err(|e| Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: e,
        })?;
    }

    Ok(db.into_model(meta))
}

fn fast_parse_sequence_signals_only(
    code: &str,
    wrap_enabled: Option<bool>,
    meta: &ParseMetadata,
) -> Option<Value> {
    // Fast-path for very small sequence diagrams that only contain signal statements.
    //
    // This avoids the LALRPOP parser + token stream overhead for tiny inputs, while preserving
    // correctness by falling back to the full parser on anything unrecognized.
    //
    // Current target fixture: benches `sequence_tiny`:
    //
    //   sequenceDiagram
    //     Alice->>Bob: Hi
    //
    // Keep the fast-path conservative to avoid surprising behavior differences.
    if code.len() > 256 {
        return None;
    }

    fn eq_ascii_ci(a: &str, b: &str) -> bool {
        a.eq_ignore_ascii_case(b)
    }

    #[derive(Clone, Copy)]
    enum Activation {
        None,
        Plus,
        Minus,
    }

    struct Signal<'a> {
        from: &'a str,
        to: &'a str,
        ty: i32,
        text: &'a str,
        activation: Activation,
    }

    fn parse_signal_line(line: &str) -> Option<Signal<'_>> {
        let s = line.trim();
        if s.is_empty() {
            return None;
        }
        // Keep the fast-path strict: semicolons are handled by the full lexer/comment rules.
        if s.contains(';') {
            return None;
        }

        let bytes = s.as_bytes();
        let mut sig_start: Option<usize> = None;
        let mut i = 0usize;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'<' {
                sig_start = Some(i);
                break;
            }
            if b == b'-' {
                let next = bytes.get(i + 1).copied();
                if matches!(next, Some(b'-' | b'>' | b'x' | b')')) {
                    sig_start = Some(i);
                    break;
                }
            }
            i += 1;
        }
        let sig_start = sig_start?;
        let from = s[..sig_start].trim();
        if from.is_empty() {
            return None;
        }

        let rest = &s[sig_start..];
        let (sig_len, ty) = if rest.starts_with("<<-->>") {
            (6, 34)
        } else if rest.starts_with("<<->>") {
            (5, 33)
        } else if rest.starts_with("-->>") {
            (4, 1)
        } else if rest.starts_with("->>") {
            (3, 0)
        } else if rest.starts_with("-->") {
            (3, 6)
        } else if rest.starts_with("->") {
            (2, 5)
        } else if rest.starts_with("--x") {
            (3, 4)
        } else if rest.starts_with("-x") {
            (2, 3)
        } else if rest.starts_with("--)") {
            (3, 25)
        } else if rest.starts_with("-)") {
            (2, 24)
        } else {
            return None;
        };

        let mut p = sig_start + sig_len;
        while p < bytes.len() && bytes[p].is_ascii_whitespace() {
            p += 1;
        }

        let activation = match bytes.get(p).copied() {
            Some(b'+') => {
                p += 1;
                Activation::Plus
            }
            Some(b'-') => {
                p += 1;
                Activation::Minus
            }
            _ => Activation::None,
        };

        while p < bytes.len() && bytes[p].is_ascii_whitespace() {
            p += 1;
        }

        let to_start = p;
        while p < bytes.len() {
            let b = bytes[p];
            if b.is_ascii_whitespace() || b == b':' {
                break;
            }
            p += 1;
        }
        let to = s[to_start..p].trim();
        if to.is_empty() {
            return None;
        }

        while p < bytes.len() && bytes[p].is_ascii_whitespace() {
            p += 1;
        }
        if bytes.get(p).copied()? != b':' {
            return None;
        }
        p += 1;
        let text = s[p..].trim();

        Some(Signal {
            from,
            to,
            ty,
            text,
            activation,
        })
    }

    let mut header_seen = false;
    let mut non_empty_lines = 0usize;
    let mut signals: Vec<Signal<'_>> = Vec::with_capacity(2);
    for raw in code.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("%%") {
            continue;
        }
        non_empty_lines += 1;
        if non_empty_lines > 8 {
            return None;
        }
        if !header_seen {
            if !eq_ascii_ci(t, "sequenceDiagram") {
                return None;
            }
            header_seen = true;
            continue;
        }
        let sig = parse_signal_line(t)?;
        signals.push(sig);
        if signals.len() > 4 {
            return None;
        }
    }

    if signals.is_empty() {
        return None;
    }

    let mut db = SequenceDb::new(wrap_enabled);
    for sig in signals {
        db.ensure_actor(sig.from);
        db.ensure_actor(sig.to);

        let activate = matches!(sig.activation, Activation::Plus);
        let msg = db.parse_message(sig.text);
        db.add_signal(
            Some(sig.from.to_string()),
            Some(sig.to.to_string()),
            Some(msg),
            sig.ty,
            activate,
            None,
        );

        match sig.activation {
            Activation::Plus => {
                db.add_signal(
                    Some(sig.to.to_string()),
                    None,
                    None,
                    LINETYPE_ACTIVE_START,
                    false,
                    None,
                );
            }
            Activation::Minus => {
                if db.activation_count(sig.from) < 1 {
                    return None;
                }
                db.add_signal(
                    Some(sig.from.to_string()),
                    None,
                    None,
                    LINETYPE_ACTIVE_END,
                    false,
                    None,
                );
            }
            Activation::None => {}
        }
    }

    Some(db.into_model(meta))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Default,
    Line,
    AccDescrMultiline,
}

struct Lexer<'input> {
    input: &'input str,
    pos: usize,
    pending: VecDeque<(usize, Tok, usize)>,
    mode: Mode,
}

impl<'input> Lexer<'input> {
    fn new(input: &'input str) -> Self {
        Self {
            input,
            pos: 0,
            pending: VecDeque::new(),
            mode: Mode::Default,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn peek2(&self) -> Option<[u8; 2]> {
        if self.pos + 1 >= self.input.len() {
            return None;
        }
        Some([
            self.input.as_bytes()[self.pos],
            self.input.as_bytes()[self.pos + 1],
        ])
    }

    fn bump(&mut self) -> Option<u8> {
        if self.pos >= self.input.len() {
            return None;
        }
        let bytes = self.input.as_bytes();
        let b = bytes[self.pos];

        // Keep `self.pos` on a UTF-8 char boundary. Mermaid input can contain arbitrary Unicode
        // (including `encodeEntities(...)` placeholders), and this lexer is otherwise byte-based.
        if b.is_ascii() {
            self.pos += 1;
        } else {
            // If we're already in the middle of a codepoint (continuation byte), resync by
            // skipping continuation bytes.
            if (b & 0b1100_0000) == 0b1000_0000 {
                self.pos += 1;
                while self.pos < bytes.len() && (bytes[self.pos] & 0b1100_0000) == 0b1000_0000 {
                    self.pos += 1;
                }
            } else {
                let len = if (b & 0b1110_0000) == 0b1100_0000 {
                    2
                } else if (b & 0b1111_0000) == 0b1110_0000 {
                    3
                } else if (b & 0b1111_1000) == 0b1111_0000 {
                    4
                } else {
                    1
                };
                self.pos = (self.pos + len).min(bytes.len());
                while self.pos < bytes.len() && (bytes[self.pos] & 0b1100_0000) == 0b1000_0000 {
                    self.pos += 1;
                }
            }
        }
        Some(b)
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b == b' ' || b == b'\t' || b == b'\r' {
                self.pos += 1;
                continue;
            }
            break;
        }
    }

    fn starts_with_ci(&self, kw: &str) -> bool {
        let rest = self.input.as_bytes().get(self.pos..).unwrap_or_default();
        let kwb = kw.as_bytes();
        if rest.len() < kwb.len() {
            return false;
        }
        for i in 0..kwb.len() {
            let a = rest[i];
            let b = kwb[i];
            if !a.eq_ignore_ascii_case(&b) {
                return false;
            }
        }
        true
    }

    fn starts_with_ci_word(&self, kw: &str) -> bool {
        if !self.starts_with_ci(kw) {
            return false;
        }
        let after = self.pos + kw.len();
        if after >= self.input.len() {
            return true;
        }
        let b = self.input.as_bytes()[after];
        !b.is_ascii_alphanumeric() && b != b'_'
    }

    fn lex_newline(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        match self.peek()? {
            b'\n' | b';' => {
                self.pos += 1;
                self.mode = Mode::Default;
                Some((start, Tok::Newline, self.pos))
            }
            _ => None,
        }
    }

    fn lex_comment(&mut self) -> bool {
        let Some(b) = self.peek() else {
            return false;
        };
        if b == b'#' {
            while let Some(b2) = self.peek() {
                if b2 == b'\n' {
                    break;
                }
                self.pos += 1;
            }
            return true;
        }
        let Some([b'%', b'%']) = self.peek2() else {
            return false;
        };
        // Mermaid directives are removed earlier in preprocess, so `%%` is always a comment here.
        while let Some(b2) = self.peek() {
            if b2 == b'\n' {
                break;
            }
            self.pos += 1;
        }
        true
    }

    fn lex_multiline_acc_descr(&mut self) -> Option<(usize, Tok, usize)> {
        if self.mode != Mode::AccDescrMultiline {
            return None;
        }
        let start = self.pos;
        let Some(rel_end) = self.input[self.pos..].find('}') else {
            let s = self.input[self.pos..].to_string();
            self.pos = self.input.len();
            return Some((start, Tok::AccDescrMultiline(s), self.pos));
        };
        let end = self.pos + rel_end;
        let s = self.input[self.pos..end].to_string();
        self.pos = end + 1;
        self.mode = Mode::Default;
        Some((start, Tok::AccDescrMultiline(s), self.pos))
    }

    fn lex_keyword_lines(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;

        if self.starts_with_ci_word("title:") {
            self.pos += "title:".len();
            self.skip_ws();
            let s = self.read_to_line_end();
            return Some((start, Tok::CompatTitle(s.trim().to_string()), self.pos));
        }

        if self.starts_with_ci_word("title") {
            let after = self.pos + "title".len();
            if after < self.input.len() && self.input.as_bytes()[after].is_ascii_whitespace() {
                self.pos = after;
                self.skip_ws();
                let s = self.read_to_line_end();
                return Some((start, Tok::Title(s.trim().to_string()), self.pos));
            }
        }

        if self.starts_with_ci_word("accTitle") {
            let after = self.pos + "accTitle".len();
            let rest = &self.input[after..];
            let colon_pos = rest.find(':')?;
            if rest[..colon_pos].chars().any(|c| c == '\n' || c == ';') {
                return None;
            }
            self.pos = after + colon_pos + 1;
            self.skip_ws();
            let s = self.read_to_line_end();
            return Some((start, Tok::AccTitle(s.trim().to_string()), self.pos));
        }

        if self.starts_with_ci_word("accDescr") {
            let after = self.pos + "accDescr".len();
            let rest = &self.input[after..];
            let non_ws = rest.find(|c: char| !c.is_whitespace())?;
            match rest[non_ws..].chars().next() {
                Some(':') => {
                    self.pos = after + non_ws + 1;
                    self.skip_ws();
                    let s = self.read_to_line_end();
                    return Some((start, Tok::AccDescr(s.trim().to_string()), self.pos));
                }
                Some('{') => {
                    self.pos = after + non_ws + 1;
                    self.mode = Mode::AccDescrMultiline;
                    return self.lex_multiline_acc_descr();
                }
                _ => {}
            }
        }

        None
    }

    fn read_to_line_end(&mut self) -> String {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if b == b'\n' || b == b';' {
                break;
            }
            if b == b'#' {
                break;
            }
            if let Some([b'%', b'%']) = self.peek2() {
                if b == b'%' {
                    break;
                }
            }
            self.pos += 1;
        }
        self.input[start..self.pos].to_string()
    }

    fn lex_word_keywords(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.starts_with_ci_word("sequenceDiagram") {
            self.pos += "sequenceDiagram".len();
            return Some((start, Tok::SequenceDiagram, self.pos));
        }
        if self.starts_with_ci_word("participant") {
            self.pos += "participant".len();
            return Some((start, Tok::Participant, self.pos));
        }
        if self.starts_with_ci_word("actor") {
            self.pos += "actor".len();
            return Some((start, Tok::ActorKw, self.pos));
        }
        if self.starts_with_ci_word("box") {
            self.pos += "box".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Box, self.pos));
        }
        if self.starts_with_ci_word("end") {
            self.pos += "end".len();
            return Some((start, Tok::End, self.pos));
        }
        if self.starts_with_ci_word("loop") {
            self.pos += "loop".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Loop, self.pos));
        }
        if self.starts_with_ci_word("rect") {
            self.pos += "rect".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Rect, self.pos));
        }
        if self.starts_with_ci_word("opt") {
            self.pos += "opt".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Opt, self.pos));
        }
        if self.starts_with_ci_word("alt") {
            self.pos += "alt".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Alt, self.pos));
        }
        if self.starts_with_ci_word("else") {
            self.pos += "else".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Else, self.pos));
        }
        if self.starts_with_ci_word("par_over") {
            self.pos += "par_over".len();
            self.mode = Mode::Line;
            return Some((start, Tok::ParOver, self.pos));
        }
        if self.starts_with_ci_word("par") {
            self.pos += "par".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Par, self.pos));
        }
        if self.starts_with_ci_word("and") {
            self.pos += "and".len();
            self.mode = Mode::Line;
            return Some((start, Tok::And, self.pos));
        }
        if self.starts_with_ci_word("critical") {
            self.pos += "critical".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Critical, self.pos));
        }
        if self.starts_with_ci_word("option") {
            self.pos += "option".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Option, self.pos));
        }
        if self.starts_with_ci_word("break") {
            self.pos += "break".len();
            self.mode = Mode::Line;
            return Some((start, Tok::Break, self.pos));
        }
        if self.starts_with_ci_word("create") {
            self.pos += "create".len();
            return Some((start, Tok::Create, self.pos));
        }
        if self.starts_with_ci_word("destroy") {
            self.pos += "destroy".len();
            return Some((start, Tok::Destroy, self.pos));
        }
        if self.starts_with_ci_word("as") {
            self.pos += "as".len();
            self.mode = Mode::Line;
            return Some((start, Tok::As, self.pos));
        }
        if self.starts_with_ci_word("note") {
            self.pos += "note".len();
            return Some((start, Tok::Note, self.pos));
        }

        if self.starts_with_ci_word("links") {
            self.pos += "links".len();
            return Some((start, Tok::Links, self.pos));
        }
        if self.starts_with_ci_word("link") {
            self.pos += "link".len();
            return Some((start, Tok::Link, self.pos));
        }
        if self.starts_with_ci_word("properties") {
            self.pos += "properties".len();
            return Some((start, Tok::Properties, self.pos));
        }
        if self.starts_with_ci_word("details") {
            self.pos += "details".len();
            return Some((start, Tok::Details, self.pos));
        }

        if self.starts_with_ci("left of") {
            let after = self.pos + "left of".len();
            if after >= self.input.len() || self.input.as_bytes()[after].is_ascii_whitespace() {
                self.pos = after;
                return Some((start, Tok::LeftOf, self.pos));
            }
        }
        if self.starts_with_ci("right of") {
            let after = self.pos + "right of".len();
            if after >= self.input.len() || self.input.as_bytes()[after].is_ascii_whitespace() {
                self.pos = after;
                return Some((start, Tok::RightOf, self.pos));
            }
        }
        if self.starts_with_ci_word("over") {
            self.pos += "over".len();
            return Some((start, Tok::Over, self.pos));
        }

        if self.starts_with_ci_word("autonumber") {
            self.pos += "autonumber".len();
            return Some((start, Tok::Autonumber, self.pos));
        }
        if self.starts_with_ci_word("off") {
            self.pos += "off".len();
            return Some((start, Tok::Off, self.pos));
        }
        if self.starts_with_ci_word("activate") {
            self.pos += "activate".len();
            return Some((start, Tok::Activate, self.pos));
        }
        if self.starts_with_ci_word("deactivate") {
            self.pos += "deactivate".len();
            return Some((start, Tok::Deactivate, self.pos));
        }

        None
    }

    fn lex_punct(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        match self.peek()? {
            b',' => {
                self.pos += 1;
                Some((start, Tok::Comma, self.pos))
            }
            b'+' => {
                self.pos += 1;
                Some((start, Tok::Plus, self.pos))
            }
            _ => None,
        }
    }

    fn lex_signal_type(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let rest = &self.input[self.pos..];

        let (len, ty) = if rest.starts_with("<<-->>") {
            (6, 34)
        } else if rest.starts_with("<<->>") {
            (5, 33)
        } else if rest.starts_with("-->>") {
            (4, 1)
        } else if rest.starts_with("->>") {
            (3, 0)
        } else if rest.starts_with("-->") {
            (3, 6)
        } else if rest.starts_with("->") {
            (2, 5)
        } else if rest.starts_with("--x") {
            (3, 4)
        } else if rest.starts_with("-x") {
            (2, 3)
        } else if rest.starts_with("--)") {
            (3, 25)
        } else if rest.starts_with("-)") {
            (2, 24)
        } else {
            return None;
        };

        self.pos += len;
        Some((start, Tok::SignalType(ty), self.pos))
    }

    fn lex_minus(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.peek()? != b'-' {
            return None;
        }
        self.pos += 1;
        Some((start, Tok::Minus, self.pos))
    }

    fn lex_num(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let mut end = self.pos;
        while let Some(b) = self.input.as_bytes().get(end) {
            if b.is_ascii_digit() {
                end += 1;
                continue;
            }
            break;
        }
        if end == start {
            return None;
        }
        let n: i64 = self.input[start..end].parse().ok()?;
        self.pos = end;
        Some((start, Tok::Num(n), self.pos))
    }

    fn lex_rest_of_line(&mut self) -> Option<(usize, Tok, usize)> {
        if self.mode != Mode::Line {
            return None;
        }
        let start = self.pos;
        let s = self.read_to_line_end();
        self.mode = Mode::Default;
        Some((start, Tok::RestOfLine(s.trim().to_string()), self.pos))
    }

    fn lex_config(&mut self) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        let start = self.pos;
        if !self.input[self.pos..].starts_with("@{") {
            return None;
        }
        if start > 0 && self.input.as_bytes()[start - 1].is_ascii_whitespace() {
            return Some(Err(LexError {
                message: "Config objects must be attached to the actor id without whitespace"
                    .to_string(),
            }));
        }
        self.pos += 2;
        let Some(rel_end) = self.input[self.pos..].find('}') else {
            return Some(Err(LexError {
                message: "Unterminated config object; missing '}'".to_string(),
            }));
        };
        let end = self.pos + rel_end;
        let s = self.input[self.pos..end].trim().to_string();
        self.pos = end + 1;
        Some(Ok((start, Tok::Config(s), self.pos)))
    }

    fn lex_text(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        if self.peek()? != b':' {
            return None;
        }
        self.pos += 1;
        let s = self.read_to_line_end();
        Some((start, Tok::Text(s.trim().to_string()), self.pos))
    }

    fn lex_actor(&mut self) -> Option<(usize, Tok, usize)> {
        let start = self.pos;
        let mut end = self.pos;
        let bytes = self.input.as_bytes();

        while end < self.input.len() {
            let b = bytes[end];
            if b.is_ascii_whitespace()
                || b == b'\n'
                || b == b';'
                || b == b','
                || b == b':'
                || b == b'+'
            {
                break;
            }
            if b == b'@' && end + 1 < bytes.len() && bytes[end + 1] == b'{' {
                break;
            }
            if b == b'-' {
                let next = bytes.get(end + 1).copied();
                if matches!(next, Some(b'-' | b'>' | b'x' | b')')) {
                    break;
                }
            }
            if b == b'<' {
                break;
            }
            end += 1;
        }

        if end == start {
            return None;
        }
        let s = self.input[start..end].trim().to_string();
        self.pos = end;
        Some((start, Tok::Actor(s), self.pos))
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = std::result::Result<(usize, Tok, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tok) = self.pending.pop_front() {
            return Some(Ok(tok));
        }

        loop {
            let start = self.pos;
            self.skip_ws();

            if self.pos >= self.input.len() {
                return None;
            }

            if self.lex_comment() {
                continue;
            }

            if let Some(tok) = self.lex_multiline_acc_descr() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_rest_of_line() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_newline() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_keyword_lines() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_word_keywords() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_signal_type() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_config() {
                return Some(tok);
            }

            if let Some(tok) = self.lex_text() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_num() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_punct() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_minus() {
                return Some(Ok(tok));
            }

            if let Some(tok) = self.lex_actor() {
                return Some(Ok(tok));
            }

            let _ = self.bump();
            return Some(Err(LexError {
                message: format!("Unexpected character at {start}"),
            }));
        }
    }
}

#[derive(Debug, Clone)]
struct BoxData {
    text: Option<String>,
    color: String,
    wrap: Option<bool>,
}

fn unescape_entities(input: &str) -> String {
    input.replace("&equals;", "=").replace("&amp;", "&")
}

fn split_color_and_title(input: &str) -> (&str, &str) {
    let lower = input.to_ascii_lowercase();
    for prefix in ["rgba", "rgb", "hsla", "hsl"] {
        if lower.starts_with(prefix) {
            if let Some(end) = input.find(')') {
                let color = &input[..=end];
                let rest = &input[end + 1..];
                return (color.trim(), rest);
            }
        }
    }

    let mut end = 0usize;
    for (idx, c) in input.char_indices() {
        if c.is_ascii_alphanumeric() || c == '_' {
            end = idx + c.len_utf8();
            continue;
        }
        break;
    }
    (&input[..end], &input[end..])
}

fn parse_participant_meta_yaml(yaml_body: &str) -> std::result::Result<serde_yaml::Value, String> {
    let yaml_data = if yaml_body.contains('\n') {
        format!("{yaml_body}\n")
    } else {
        format!("{{\n{yaml_body}\n}}")
    };
    serde_yaml::from_str(&yaml_data).map_err(|e| format!("{e}"))
}

fn yaml_to_string(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn is_css_color_value(input: &str) -> bool {
    let t = input.trim();
    if t.is_empty() {
        return false;
    }
    let lower = t.to_ascii_lowercase();
    if lower == "transparent" {
        return true;
    }
    if (lower.starts_with("rgb(")
        || lower.starts_with("rgba(")
        || lower.starts_with("hsl(")
        || lower.starts_with("hsla("))
        && lower.ends_with(')')
    {
        return true;
    }
    if !lower.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    CSS_COLOR_KEYWORDS.binary_search(&lower.as_str()).is_ok()
}

static CSS_COLOR_KEYWORDS: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];
