use dioxus::prelude::*;

use crate::state::AppState;

#[component]
pub fn DigestFsrsView() -> Element {
    let state = use_context::<AppState>();
    let view = (state.current_view)();
    rsx! {
        section { "aria-label": view.label(), "data-testid": format!("view-{}", view.id()),
            h1 { "{view.label()}" }
        }
    }
}
