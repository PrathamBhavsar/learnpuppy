use dioxus::prelude::*;

use crate::state::{AppState, View};

#[component]
pub fn ClassroomView() -> Element {
    let state = use_context::<AppState>();
    let view = (state.current_view)();
    rsx! {
        section { "aria-label": view.label(), "data-testid": format!("view-{}", view.id()),
            h1 { "{view.label()}" }
            if view == View::Classroom {
                p { role: "status", "data-testid": "classroom-action", "{(state.quick_action)().label()} session" }
            }
        }
    }
}
