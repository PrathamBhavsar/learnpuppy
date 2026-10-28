use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Graph,
    Classroom,
    Plan,
    Review,
    Digest,
}

impl View {
    pub const ALL: [Self; 5] = [
        Self::Graph,
        Self::Classroom,
        Self::Plan,
        Self::Review,
        Self::Digest,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Graph => "Graph",
            Self::Classroom => "Classroom",
            Self::Plan => "Plan",
            Self::Review => "Review",
            Self::Digest => "Digest",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Graph => "graph",
            Self::Classroom => "classroom",
            Self::Plan => "plan",
            Self::Review => "review",
            Self::Digest => "digest",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum QuickAction {
    #[default]
    Teach,
    Quiz,
    Practice,
    Review,
}

impl QuickAction {
    pub const ALL: [Self; 4] = [Self::Teach, Self::Quiz, Self::Practice, Self::Review];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Teach => "Teach",
            Self::Quiz => "Quiz",
            Self::Practice => "Practice",
            Self::Review => "Review",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Teach => "teach",
            Self::Quiz => "quiz",
            Self::Practice => "practice",
            Self::Review => "review",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct AppState {
    pub current_view: Signal<View>,
    pub command_palette_open: Signal<bool>,
    pub accounts_modal_open: Signal<bool>,
    pub quick_action: Signal<QuickAction>,
}

impl AppState {
    pub fn use_provider(initial_view: View, accounts_open: bool) -> Self {
        let current_view = use_signal(|| initial_view);
        let command_palette_open = use_signal(|| false);
        let accounts_modal_open = use_signal(|| accounts_open);
        let quick_action = use_signal(QuickAction::default);
        use_context_provider(|| Self {
            current_view,
            command_palette_open,
            accounts_modal_open,
            quick_action,
        })
    }

    pub fn select_action(&mut self, action: QuickAction) {
        self.quick_action.set(action);
        self.current_view.set(match action {
            QuickAction::Review => View::Review,
            _ => View::Classroom,
        });
    }
}

pub fn use_app_state() -> AppState {
    use_context::<AppState>()
}

pub fn is_accounts_shortcut(key: &Key, modifiers: Modifiers) -> bool {
    (modifiers.meta() || modifiers.ctrl())
        && !modifiers.alt()
        && matches!(key, Key::Character(value) if value.eq_ignore_ascii_case("a"))
}
