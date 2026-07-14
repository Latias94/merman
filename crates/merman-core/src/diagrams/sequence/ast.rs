pub(super) enum Action {
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
        central_connection: i32,
    },
    ActiveStart {
        actor: String,
    },
    ActiveEnd {
        actor: String,
    },
    CentralConnection {
        actor: String,
    },
    CentralConnectionReverse {
        actor: String,
    },

    AddNote {
        actors: Vec<String>,
        placement: i32,
        text: String,
    },

    Autonumber {
        start: Option<f64>,
        step: Option<f64>,
        visible: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceParticipantKind {
    Participant,
    Actor,
}

impl SequenceParticipantKind {
    const fn draw(self) -> &'static str {
        match self {
            Self::Participant => "participant",
            Self::Actor => "actor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceMessageKind {
    Solid,
    Dotted,
}

impl SequenceMessageKind {
    const fn signal_type(self) -> i32 {
        match self {
            Self::Solid => super::LINETYPE_SOLID,
            Self::Dotted => super::LINETYPE_DOTTED,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequenceControlKind {
    LoopStart,
    LoopEnd,
    AltStart,
    AltElse,
    AltEnd,
    OptStart,
    OptEnd,
    ParStart,
    ParAnd,
    ParEnd,
}

impl SequenceControlKind {
    const fn signal_type(self) -> i32 {
        match self {
            Self::LoopStart => super::LINETYPE_LOOP_START,
            Self::LoopEnd => super::LINETYPE_LOOP_END,
            Self::AltStart => super::LINETYPE_ALT_START,
            Self::AltElse => super::LINETYPE_ALT_ELSE,
            Self::AltEnd => super::LINETYPE_ALT_END,
            Self::OptStart => super::LINETYPE_OPT_START,
            Self::OptEnd => super::LINETYPE_OPT_END,
            Self::ParStart => super::LINETYPE_PAR_START,
            Self::ParAnd => super::LINETYPE_PAR_AND,
            Self::ParEnd => super::LINETYPE_PAR_END,
        }
    }
}

#[derive(Default)]
pub(crate) struct SequenceActionBuilder {
    actions: Vec<Action>,
}

impl SequenceActionBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set_title(&mut self, title: String) {
        self.actions.push(Action::SetTitle(title));
    }

    pub(crate) fn set_acc_title(&mut self, title: String) {
        self.actions.push(Action::SetAccTitle(title));
    }

    pub(crate) fn set_acc_descr(&mut self, description: String) {
        self.actions.push(Action::SetAccDescr(description));
    }

    pub(crate) fn participant(
        &mut self,
        id: String,
        description: Option<String>,
        kind: SequenceParticipantKind,
    ) {
        self.actions.push(Action::AddParticipant {
            id,
            description,
            draw: kind.draw().to_string(),
            config: None,
        });
    }

    pub(crate) fn create_participant(
        &mut self,
        id: String,
        description: Option<String>,
        kind: SequenceParticipantKind,
    ) {
        self.actions.push(Action::CreateParticipant {
            id,
            description,
            draw: kind.draw().to_string(),
            config: None,
        });
    }

    pub(crate) fn control(&mut self, kind: SequenceControlKind, text: Option<String>) {
        self.actions.push(Action::ControlSignal {
            signal_type: kind.signal_type(),
            text,
        });
    }

    pub(crate) fn message(
        &mut self,
        from: String,
        to: String,
        kind: SequenceMessageKind,
        text: String,
    ) {
        self.actions
            .push(Action::EnsureParticipant { id: from.clone() });
        self.actions
            .push(Action::EnsureParticipant { id: to.clone() });
        self.actions.push(Action::AddMessage {
            from,
            to,
            signal_type: kind.signal_type(),
            text,
            activate: false,
            central_connection: 0,
        });
    }

    pub(crate) fn note_over(&mut self, from: String, to: String, text: String) {
        self.actions
            .push(Action::EnsureParticipant { id: from.clone() });
        self.actions
            .push(Action::EnsureParticipant { id: to.clone() });
        self.actions.push(Action::AddNote {
            actors: vec![from, to],
            placement: super::PLACEMENT_OVER,
            text,
        });
    }

    pub(crate) fn activate(&mut self, actor: String) {
        self.actions.push(Action::ActiveStart { actor });
    }

    pub(crate) fn deactivate(&mut self, actor: String) {
        self.actions.push(Action::ActiveEnd { actor });
    }

    pub(super) fn into_actions(self) -> Vec<Action> {
        self.actions
    }
}
