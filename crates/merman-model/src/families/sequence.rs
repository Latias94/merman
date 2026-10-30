//! Canonical Sequence data, message semantics, and borrowed compatibility serialization.

use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;

pub mod compat;
mod complexity;
#[cfg(test)]
mod tests;

pub use compat::SequenceCompatibilityProjection;
pub use complexity::SequenceComplexity;

// Mermaid 11.16.1 sequence diagram constants (SequenceDB.LINETYPE / PLACEMENT).
pub const LINETYPE_SOLID: i32 = 0;
pub const LINETYPE_DOTTED: i32 = 1;
pub const LINETYPE_NOTE: i32 = 2;
pub const LINETYPE_SOLID_CROSS: i32 = 3;
pub const LINETYPE_DOTTED_CROSS: i32 = 4;
pub const LINETYPE_SOLID_OPEN: i32 = 5;
pub const LINETYPE_DOTTED_OPEN: i32 = 6;
pub const LINETYPE_LOOP_START: i32 = 10;
pub const LINETYPE_LOOP_END: i32 = 11;
pub const LINETYPE_ALT_START: i32 = 12;
pub const LINETYPE_ALT_ELSE: i32 = 13;
pub const LINETYPE_ALT_END: i32 = 14;
pub const LINETYPE_OPT_START: i32 = 15;
pub const LINETYPE_OPT_END: i32 = 16;
pub const LINETYPE_ACTIVE_START: i32 = 17;
pub const LINETYPE_ACTIVE_END: i32 = 18;
pub const LINETYPE_PAR_START: i32 = 19;
pub const LINETYPE_PAR_AND: i32 = 20;
pub const LINETYPE_PAR_END: i32 = 21;
pub const LINETYPE_RECT_START: i32 = 22;
pub const LINETYPE_RECT_END: i32 = 23;
pub const LINETYPE_SOLID_POINT: i32 = 24;
pub const LINETYPE_DOTTED_POINT: i32 = 25;
pub const LINETYPE_AUTONUMBER: i32 = 26;
pub const LINETYPE_CRITICAL_START: i32 = 27;
pub const LINETYPE_CRITICAL_OPTION: i32 = 28;
pub const LINETYPE_CRITICAL_END: i32 = 29;
pub const LINETYPE_BREAK_START: i32 = 30;
pub const LINETYPE_BREAK_END: i32 = 31;
pub const LINETYPE_PAR_OVER_START: i32 = 32;
pub const LINETYPE_BIDIRECTIONAL_SOLID: i32 = 33;
pub const LINETYPE_BIDIRECTIONAL_DOTTED: i32 = 34;
pub const LINETYPE_SOLID_TOP: i32 = 41;
pub const LINETYPE_SOLID_BOTTOM: i32 = 42;
pub const LINETYPE_STICK_TOP: i32 = 43;
pub const LINETYPE_STICK_BOTTOM: i32 = 44;
pub const LINETYPE_SOLID_ARROW_TOP_REVERSE: i32 = 45;
pub const LINETYPE_SOLID_ARROW_BOTTOM_REVERSE: i32 = 46;
pub const LINETYPE_STICK_ARROW_TOP_REVERSE: i32 = 47;
pub const LINETYPE_STICK_ARROW_BOTTOM_REVERSE: i32 = 48;
pub const LINETYPE_SOLID_TOP_DOTTED: i32 = 51;
pub const LINETYPE_SOLID_BOTTOM_DOTTED: i32 = 52;
pub const LINETYPE_STICK_TOP_DOTTED: i32 = 53;
pub const LINETYPE_STICK_BOTTOM_DOTTED: i32 = 54;
pub const LINETYPE_SOLID_ARROW_TOP_REVERSE_DOTTED: i32 = 55;
pub const LINETYPE_SOLID_ARROW_BOTTOM_REVERSE_DOTTED: i32 = 56;
pub const LINETYPE_STICK_ARROW_TOP_REVERSE_DOTTED: i32 = 57;
pub const LINETYPE_STICK_ARROW_BOTTOM_REVERSE_DOTTED: i32 = 58;
pub const LINETYPE_CENTRAL_CONNECTION: i32 = 59;
pub const LINETYPE_CENTRAL_CONNECTION_REVERSE: i32 = 60;
pub const LINETYPE_CENTRAL_CONNECTION_DUAL: i32 = 61;

pub const PLACEMENT_LEFT_OF: i32 = 0;
pub const PLACEMENT_RIGHT_OF: i32 = 1;
pub const PLACEMENT_OVER: i32 = 2;

/// Canonical Sequence actors, messages, and parser-resolved lifecycle ownership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceDiagramModel {
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "actorOrder")]
    pub actor_order: Vec<String>,
    pub actors: BTreeMap<String, SequenceActor>,
    #[serde(default)]
    pub boxes: Vec<SequenceBox>,
    pub messages: Vec<SequenceMessage>,
    #[serde(default)]
    pub notes: Vec<SequenceNote>,
    #[serde(rename = "createdActors", default)]
    pub created_actors: BTreeMap<String, usize>,
    #[serde(rename = "destroyedActors", default)]
    pub destroyed_actors: BTreeMap<String, usize>,
    #[serde(
        rename = "actorLifecycles",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    /// Parser-resolved lifecycle message ownership.
    ///
    /// Mermaid's compatibility maps retain declaration anchors, while its
    /// `AddMessage` state machine decides which later signal actually consumes
    /// each pending create or destroy request. Parser-backed models store that
    /// resolved truth in actor-order slots here. `None` is reserved for
    /// legacy/direct typed models that only provide the compatibility maps.
    pub actor_lifecycles: Option<Vec<SequenceActorLifecycle>>,
}

impl SequenceDiagramModel {
    /// Returns the lifecycle message ownership for one actor.
    ///
    /// Parser-backed models use the resolved `AddMessage` projection. Legacy
    /// direct models fall back to Mermaid's compatibility anchor maps.
    pub fn actor_lifecycle(&self, actor_id: &str) -> Option<SequenceActorLifecycle> {
        match &self.actor_lifecycles {
            Some(_) => self
                .actor_order
                .iter()
                .position(|id| id == actor_id)
                .and_then(|index| self.actor_lifecycle_at(index)),
            None => {
                let lifecycle = SequenceActorLifecycle {
                    created_at: self.created_actors.get(actor_id).copied(),
                    destroyed_at: self.destroyed_actors.get(actor_id).copied(),
                };
                (lifecycle != SequenceActorLifecycle::default()).then_some(lifecycle)
            }
        }
    }

    /// Returns one actor's parser-resolved lifecycle by normalized actor order.
    pub fn actor_lifecycle_at(&self, actor_index: usize) -> Option<SequenceActorLifecycle> {
        match &self.actor_lifecycles {
            Some(lifecycles) => lifecycles
                .get(actor_index)
                .copied()
                .filter(|lifecycle| *lifecycle != SequenceActorLifecycle::default()),
            None => self
                .actor_order
                .get(actor_index)
                .and_then(|actor_id| self.actor_lifecycle(actor_id)),
        }
    }

    /// Returns the effective creation index by normalized actor order.
    pub fn created_actor_message_index_at(&self, actor_index: usize) -> Option<usize> {
        self.actor_lifecycle_at(actor_index)?.created_at
    }

    /// Returns the effective destruction index by normalized actor order.
    pub fn destroyed_actor_message_index_at(&self, actor_index: usize) -> Option<usize> {
        self.actor_lifecycle_at(actor_index)?.destroyed_at
    }

    /// Returns the effective creation index.
    ///
    /// Parser-backed models return the consuming signal index. Legacy direct
    /// models may return the compatibility declaration anchor instead.
    pub fn created_actor_message_index(&self, actor_id: &str) -> Option<usize> {
        self.actor_lifecycle(actor_id)?.created_at
    }

    /// Returns the effective destruction index.
    ///
    /// Parser-backed models return the consuming signal index. Legacy direct
    /// models may return the compatibility declaration anchor instead.
    pub fn destroyed_actor_message_index(&self, actor_id: &str) -> Option<usize> {
        self.actor_lifecycle(actor_id)?.destroyed_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceActor {
    #[serde(default)]
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub actor_type: String,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub links: serde_json::Map<String, Value>,
    #[serde(default)]
    pub properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Signal indices that consumed one actor's pending Mermaid lifecycle requests.
pub struct SequenceActorLifecycle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destroyed_at: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Stroke pattern of a drawable Mermaid sequence signal.
pub enum SequenceMessageStroke {
    #[default]
    Solid,
    Dotted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Marker owned by one authored endpoint of a Mermaid sequence signal.
pub enum SequenceMessageMarker {
    #[default]
    None,
    Filled,
    Cross,
    Point,
    FilledHalfTop,
    FilledHalfBottom,
    OpenHalfTop,
    OpenHalfBottom,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Semantic direction of the signal relative to its authored `from` and `to` actors.
pub enum SequenceMessageDirection {
    #[default]
    Forward,
    Reverse,
    Bidirectional,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Optional central-connection decoration attached to authored signal endpoints.
pub enum SequenceCentralDecoration {
    #[default]
    None,
    Source,
    Target,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Semantic family of a Mermaid sequence control record.
pub enum SequenceControlKind {
    Loop,
    Opt,
    Break,
    Alt,
    Par,
    Critical,
    Rect,
    ParOver,
}

impl SequenceControlKind {
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Opt => "opt",
            Self::Break => "break",
            Self::Alt => "alt",
            Self::Par => "par",
            Self::Critical => "critical",
            Self::Rect => "rect",
            Self::ParOver => "par_over",
        }
    }

    pub fn separator_keyword(self) -> Option<&'static str> {
        match self {
            Self::Alt => Some("else"),
            Self::Par => Some("and"),
            Self::Critical => Some("option"),
            Self::Loop | Self::Opt | Self::Break | Self::Rect | Self::ParOver => None,
        }
    }

    pub fn accepts_end(self, end: Self) -> bool {
        self == end || matches!((self, end), (Self::ParOver, Self::Par))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Structural role of one Mermaid sequence control record.
pub enum SequenceControlRole {
    Start,
    Separator,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Typed projection of Mermaid's numeric control-record protocol.
pub struct SequenceControlSemantics {
    pub kind: SequenceControlKind,
    pub role: SequenceControlRole,
}

impl SequenceControlSemantics {
    pub fn consumes_text(self) -> bool {
        self.role != SequenceControlRole::End
    }

    fn from_message_type(message_type: i32) -> Option<Self> {
        use SequenceControlKind as Kind;
        use SequenceControlRole as Role;

        let (kind, role) = match message_type {
            LINETYPE_LOOP_START => (Kind::Loop, Role::Start),
            LINETYPE_LOOP_END => (Kind::Loop, Role::End),
            LINETYPE_ALT_START => (Kind::Alt, Role::Start),
            LINETYPE_ALT_ELSE => (Kind::Alt, Role::Separator),
            LINETYPE_ALT_END => (Kind::Alt, Role::End),
            LINETYPE_OPT_START => (Kind::Opt, Role::Start),
            LINETYPE_OPT_END => (Kind::Opt, Role::End),
            LINETYPE_PAR_START => (Kind::Par, Role::Start),
            LINETYPE_PAR_AND => (Kind::Par, Role::Separator),
            LINETYPE_PAR_END => (Kind::Par, Role::End),
            LINETYPE_RECT_START => (Kind::Rect, Role::Start),
            LINETYPE_RECT_END => (Kind::Rect, Role::End),
            LINETYPE_CRITICAL_START => (Kind::Critical, Role::Start),
            LINETYPE_CRITICAL_OPTION => (Kind::Critical, Role::Separator),
            LINETYPE_CRITICAL_END => (Kind::Critical, Role::End),
            LINETYPE_BREAK_START => (Kind::Break, Role::Start),
            LINETYPE_BREAK_END => (Kind::Break, Role::End),
            LINETYPE_PAR_OVER_START => (Kind::ParOver, Role::Start),
            _ => return None,
        };
        Some(Self { kind, role })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Semantic placement of an authored sequence note.
pub enum SequenceNotePlacement {
    LeftOf,
    RightOf,
    Over,
}

impl SequenceNotePlacement {
    fn from_raw(placement: i32) -> Option<Self> {
        match placement {
            PLACEMENT_LEFT_OF => Some(Self::LeftOf),
            PLACEMENT_RIGHT_OF => Some(Self::RightOf),
            PLACEMENT_OVER => Some(Self::Over),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Typed role of one Mermaid compatibility `messages` record.
pub enum SequenceMessageKind {
    Signal,
    Note,
    ActivationStart,
    ActivationEnd,
    Autonumber,
    Control,
    CentralDecorationRecord,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Lossless terminal-facing projection of Mermaid's numeric signal line types.
pub struct SequenceSignalSemantics {
    pub stroke: SequenceMessageStroke,
    pub source_marker: SequenceMessageMarker,
    pub target_marker: SequenceMessageMarker,
    pub direction: SequenceMessageDirection,
}

impl SequenceSignalSemantics {
    const fn forward(stroke: SequenceMessageStroke, target_marker: SequenceMessageMarker) -> Self {
        Self {
            stroke,
            source_marker: SequenceMessageMarker::None,
            target_marker,
            direction: SequenceMessageDirection::Forward,
        }
    }

    const fn reverse(stroke: SequenceMessageStroke, source_marker: SequenceMessageMarker) -> Self {
        Self {
            stroke,
            source_marker,
            target_marker: SequenceMessageMarker::None,
            direction: SequenceMessageDirection::Reverse,
        }
    }

    const fn bidirectional(stroke: SequenceMessageStroke) -> Self {
        Self {
            stroke,
            source_marker: SequenceMessageMarker::Filled,
            target_marker: SequenceMessageMarker::Filled,
            direction: SequenceMessageDirection::Bidirectional,
        }
    }

    fn from_message_type(message_type: i32) -> Option<Self> {
        use SequenceMessageMarker as Marker;
        use SequenceMessageStroke as Stroke;

        match message_type {
            LINETYPE_SOLID => Some(Self::forward(Stroke::Solid, Marker::Filled)),
            LINETYPE_DOTTED => Some(Self::forward(Stroke::Dotted, Marker::Filled)),
            LINETYPE_SOLID_CROSS => Some(Self::forward(Stroke::Solid, Marker::Cross)),
            LINETYPE_DOTTED_CROSS => Some(Self::forward(Stroke::Dotted, Marker::Cross)),
            // Mermaid calls these numeric line types "open", but 11.16.1 renders them as
            // headless signals: the name distinguishes the grammar form, not an endpoint marker.
            LINETYPE_SOLID_OPEN => Some(Self::forward(Stroke::Solid, Marker::None)),
            LINETYPE_DOTTED_OPEN => Some(Self::forward(Stroke::Dotted, Marker::None)),
            LINETYPE_SOLID_POINT => Some(Self::forward(Stroke::Solid, Marker::Point)),
            LINETYPE_DOTTED_POINT => Some(Self::forward(Stroke::Dotted, Marker::Point)),
            LINETYPE_BIDIRECTIONAL_SOLID => Some(Self::bidirectional(Stroke::Solid)),
            LINETYPE_BIDIRECTIONAL_DOTTED => Some(Self::bidirectional(Stroke::Dotted)),
            LINETYPE_SOLID_TOP => Some(Self::forward(Stroke::Solid, Marker::FilledHalfTop)),
            LINETYPE_SOLID_BOTTOM => Some(Self::forward(Stroke::Solid, Marker::FilledHalfBottom)),
            LINETYPE_STICK_TOP => Some(Self::forward(Stroke::Solid, Marker::OpenHalfTop)),
            LINETYPE_STICK_BOTTOM => Some(Self::forward(Stroke::Solid, Marker::OpenHalfBottom)),
            LINETYPE_SOLID_TOP_DOTTED => Some(Self::forward(Stroke::Dotted, Marker::FilledHalfTop)),
            LINETYPE_SOLID_BOTTOM_DOTTED => {
                Some(Self::forward(Stroke::Dotted, Marker::FilledHalfBottom))
            }
            LINETYPE_STICK_TOP_DOTTED => Some(Self::forward(Stroke::Dotted, Marker::OpenHalfTop)),
            LINETYPE_STICK_BOTTOM_DOTTED => {
                Some(Self::forward(Stroke::Dotted, Marker::OpenHalfBottom))
            }
            LINETYPE_SOLID_ARROW_TOP_REVERSE => {
                Some(Self::reverse(Stroke::Solid, Marker::FilledHalfTop))
            }
            LINETYPE_SOLID_ARROW_BOTTOM_REVERSE => {
                Some(Self::reverse(Stroke::Solid, Marker::FilledHalfBottom))
            }
            LINETYPE_STICK_ARROW_TOP_REVERSE => {
                Some(Self::reverse(Stroke::Solid, Marker::OpenHalfTop))
            }
            LINETYPE_STICK_ARROW_BOTTOM_REVERSE => {
                Some(Self::reverse(Stroke::Solid, Marker::OpenHalfBottom))
            }
            LINETYPE_SOLID_ARROW_TOP_REVERSE_DOTTED => {
                Some(Self::reverse(Stroke::Dotted, Marker::FilledHalfTop))
            }
            LINETYPE_SOLID_ARROW_BOTTOM_REVERSE_DOTTED => {
                Some(Self::reverse(Stroke::Dotted, Marker::FilledHalfBottom))
            }
            LINETYPE_STICK_ARROW_TOP_REVERSE_DOTTED => {
                Some(Self::reverse(Stroke::Dotted, Marker::OpenHalfTop))
            }
            LINETYPE_STICK_ARROW_BOTTOM_REVERSE_DOTTED => {
                Some(Self::reverse(Stroke::Dotted, Marker::OpenHalfBottom))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceMessage {
    pub id: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(rename = "type")]
    pub message_type: i32,
    pub message: SequenceMessagePayload,
    #[serde(default)]
    pub wrap: bool,
    #[serde(default)]
    pub activate: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<i32>,
    #[serde(
        rename = "centralConnection",
        default,
        skip_serializing_if = "is_zero_i32"
    )]
    pub central_connection: i32,
}

impl SequenceMessage {
    pub fn message_text(&self) -> &str {
        self.message.as_text()
    }

    pub fn semantic_kind(&self) -> SequenceMessageKind {
        if self.signal_semantics().is_some() {
            return SequenceMessageKind::Signal;
        }

        match self.message_type {
            LINETYPE_NOTE => SequenceMessageKind::Note,
            LINETYPE_ACTIVE_START => SequenceMessageKind::ActivationStart,
            LINETYPE_ACTIVE_END => SequenceMessageKind::ActivationEnd,
            LINETYPE_AUTONUMBER => SequenceMessageKind::Autonumber,
            _ if self.central_record_decoration().is_some() => {
                SequenceMessageKind::CentralDecorationRecord
            }
            _ if self.control_semantics().is_some() => SequenceMessageKind::Control,
            _ => SequenceMessageKind::Unknown,
        }
    }

    pub fn signal_semantics(&self) -> Option<SequenceSignalSemantics> {
        SequenceSignalSemantics::from_message_type(self.message_type)
    }

    pub fn central_decoration(&self) -> Option<SequenceCentralDecoration> {
        match self.central_connection {
            0 => Some(SequenceCentralDecoration::None),
            LINETYPE_CENTRAL_CONNECTION => Some(SequenceCentralDecoration::Target),
            LINETYPE_CENTRAL_CONNECTION_REVERSE => Some(SequenceCentralDecoration::Source),
            LINETYPE_CENTRAL_CONNECTION_DUAL => Some(SequenceCentralDecoration::Both),
            _ => None,
        }
    }

    pub fn central_record_decoration(&self) -> Option<SequenceCentralDecoration> {
        match self.message_type {
            LINETYPE_CENTRAL_CONNECTION => Some(SequenceCentralDecoration::Target),
            LINETYPE_CENTRAL_CONNECTION_REVERSE => Some(SequenceCentralDecoration::Source),
            LINETYPE_CENTRAL_CONNECTION_DUAL => Some(SequenceCentralDecoration::Both),
            _ => None,
        }
    }

    pub fn control_semantics(&self) -> Option<SequenceControlSemantics> {
        SequenceControlSemantics::from_message_type(self.message_type)
    }

    pub fn note_placement(&self) -> Option<SequenceNotePlacement> {
        SequenceNotePlacement::from_raw(self.placement.unwrap_or(PLACEMENT_OVER))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SequenceMessagePayload {
    Text(String),
    Autonumber(SequenceAutonumber),
}

impl SequenceMessagePayload {
    pub fn as_text(&self) -> &str {
        match self {
            Self::Text(text) => text,
            Self::Autonumber(_) => "",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceAutonumber {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_sequence_number"
    )]
    pub start: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_sequence_number"
    )]
    pub step: Option<f64>,
    #[serde(default = "default_true")]
    pub visible: bool,
}

fn serialize_optional_sequence_number<S>(
    value: &Option<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serialize_sequence_number(*value, serializer),
        None => serializer.serialize_none(),
    }
}

fn serialize_sequence_number<S>(value: f64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if value.is_finite()
        && value.fract() == 0.0
        && value >= i64::MIN as f64
        && value <= i64::MAX as f64
    {
        serializer.serialize_i64(value as i64)
    } else {
        serializer.serialize_f64(value)
    }
}

fn default_true() -> bool {
    true
}

fn is_zero_i32(value: &i32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceBox {
    #[serde(rename = "actorKeys")]
    pub actor_keys: Vec<String>,
    pub fill: String,
    pub name: Option<String>,
    #[serde(default)]
    pub wrap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceNote {
    pub actor: Value,
    pub message: String,
    pub placement: i32,
    #[serde(default)]
    pub wrap: bool,
}

impl SequenceNote {
    pub fn note_placement(&self) -> Option<SequenceNotePlacement> {
        SequenceNotePlacement::from_raw(self.placement)
    }
}
