use dioxus::prelude::*;

use crate::state::AppState;

#[component]
pub fn GraphCanvasView() -> Element {
    let state = use_context::<AppState>();
    let view = (state.current_view)();
    rsx! {
        section { "aria-label": view.label(), "data-testid": "view-graph",
            h1 { "{view.label()}" }
        }
    }
}
