use dioxus::prelude::*;
use lp_ui::command_palette::{is_command_palette_shortcut, CommandItem, CommandPalette};
use lp_ui::icons::{IconCheck, IconClassroom, IconDigest, IconGraph, IconSparkles};
use lp_ui::primitives::{Button, ButtonSize, ButtonVariant};

use crate::state::{is_accounts_shortcut, use_app_state, QuickAction, View};
use crate::views::accounts::Accounts;
use crate::views::classroom::ClassroomView;
use crate::views::digest_fsrs::DigestFsrsView;
use crate::views::graph_canvas::GraphCanvasView;

#[component]
pub fn Shell() -> Element {
    let mut state = use_app_state();
    let view = (state.current_view)();
    let commands = View::ALL
        .into_iter()
        .map(|view| CommandItem::new(view.id(), view.label()).with_category("Navigation"))
        .chain([CommandItem::new("accounts", "CLI accounts").with_shortcut("⌘A")])
        .collect();
    rsx! {
        div {
            class: "h-screen w-screen overflow-hidden select-none flex flex-col bg-background text-foreground",
            "data-testid": "app-shell",
            tabindex: "0",
            "aria-label": "LearnPuppy",
            onmounted: move |event| async move { let _ = event.data().set_focus(true).await; },
            onkeydown: move |event| {
                if is_accounts_shortcut(&event.key(), event.modifiers()) {
                    event.prevent_default();
                    state.command_palette_open.set(false);
                    let open = (state.accounts_modal_open)();
                    state.accounts_modal_open.set(!open);
                } else if is_command_palette_shortcut(&event.key(), event.modifiers()) {
                    event.prevent_default();
                    state.accounts_modal_open.set(false);
                    let open = (state.command_palette_open)();
                    state.command_palette_open.set(!open);
                }
            },
            header { class: "flex items-center justify-between", style: "padding: 0.75rem 1rem; border-bottom: 1px solid var(--border); gap: 1rem;",
                strong { "LearnPuppy" }
                div { role: "toolbar", "aria-label": "Quick actions", class: "flex items-center", style: "gap: 0.5rem;",
                    for action in QuickAction::ALL {
                        Button {
                            key: "{action.id()}",
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            aria_label: action.label(),
                            test_id: format!("action-{}", action.id()),
                            onclick: move |_| state.select_action(action),
                            "{action.label()}"
                        }
                    }
                }
                div { class: "flex items-center", style: "gap: 0.5rem;",
                    Button {
                        variant: ButtonVariant::Ghost,
                        aria_label: "Open command palette",
                        test_id: "open-command-palette",
                        "aria-keyshortcuts": "Meta+K Control+K",
                        onclick: move |_| {
                            state.accounts_modal_open.set(false);
                            state.command_palette_open.set(true);
                        },
                        "Commands ⌘K"
                    }
                    Button {
                        variant: ButtonVariant::Ghost,
                        aria_label: "Open CLI accounts",
                        test_id: "open-accounts",
                        "aria-keyshortcuts": "Meta+A Control+A",
                        onclick: move |_| {
                            state.command_palette_open.set(false);
                            state.accounts_modal_open.set(true);
                        },
                        "Accounts ⌘A"
                    }
                }
            }
            div { class: "flex overflow-hidden", style: "flex: 1; min-height: 0;",
                nav { "aria-label": "Main navigation", "data-testid": "navigation-rail",
                    class: "flex flex-col", style: "gap: 0.5rem; padding: 0.75rem 0.5rem; border-right: 1px solid var(--border);",
                    for target in View::ALL {
                        Button {
                            key: "{target.id()}",
                            variant: if target == view { ButtonVariant::Secondary } else { ButtonVariant::Ghost },
                            size: ButtonSize::Icon,
                            aria_label: target.label(),
                            test_id: format!("nav-{}", target.id()),
                            "aria-current": if target == view { Some("page") } else { None },
                            title: target.label(),
                            onclick: move |_| state.current_view.set(target),
                            match target {
                                View::Graph => rsx! { IconGraph {} },
                                View::Classroom => rsx! { IconClassroom {} },
                                View::Plan => rsx! { IconSparkles {} },
                                View::Review => rsx! { IconCheck {} },
                                View::Digest => rsx! { IconDigest {} },
                            }
                        }
                    }
                }
                main { "aria-label": view.label(), "data-testid": "main-view",
                    style: "flex: 1; min-width: 0; min-height: 0; overflow: auto; padding: 1.5rem;",
                    match view {
                        View::Graph => rsx! { GraphCanvasView {} },
                        View::Classroom | View::Plan => rsx! { ClassroomView {} },
                        View::Review | View::Digest => rsx! { DigestFsrsView {} },
                    }
                }
            }
            if (state.accounts_modal_open)() { Accounts {} }
            CommandPalette {
                open_signal: state.command_palette_open,
                commands,
                test_id: "command-palette",
                on_select: move |id: String| {
                    if id == "accounts" {
                        state.accounts_modal_open.set(true);
                    } else if let Some(view) = View::ALL.into_iter().find(|view| view.id() == id) {
                        state.current_view.set(view);
                    }
                    state.command_palette_open.set(false);
                },
            }
        }
    }
}
