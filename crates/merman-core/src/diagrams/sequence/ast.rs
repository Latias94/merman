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
