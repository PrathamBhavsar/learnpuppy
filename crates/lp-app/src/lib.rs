use dioxus::prelude::*;

pub mod backend;
pub mod shell;
pub mod state;
pub mod views;

pub fn app() -> Element {
    rsx! {
        div { class: "h-screen w-screen overflow-hidden select-none flex flex-col bg-background text-foreground" }
    }
}
