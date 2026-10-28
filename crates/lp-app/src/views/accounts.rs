use dioxus::prelude::*;
use lp_ui::dialog::Dialog;
use lp_ui::primitives::{Badge, BadgeVariant, Button, ButtonVariant};

use crate::backend::{AccountSlot, BackendHandle};
use crate::state::use_app_state;

#[component]
pub fn Accounts() -> Element {
    let state = use_app_state();
    let backend = use_context::<BackendHandle>();
    let mut slots = use_resource(move || {
        let backend = backend.clone();
        async move { backend.accounts().await }
    });
    let result = slots.read_unchecked();
    rsx! {
        Dialog {
            open_signal: state.accounts_modal_open,
            title: "CLI accounts",
            description: "Your local Claude, Codex, and Antigravity subscriptions. Each account is an independent CLI slot.",
            aria_label: "CLI accounts",
            test_id: "accounts-modal",
            div { class: "flex flex-col", style: "gap: 1rem; min-width: 0;",
                match result.as_ref() {
                    None => rsx! { p { role: "status", "data-testid": "accounts-loading", "Discovering local accounts…" } },
                    Some(Err(error)) => rsx! { p { role: "alert", "data-testid": "accounts-error", "Could not discover accounts: {error}" } },
                    Some(Ok(accounts)) => rsx! { AccountList { slots: accounts.clone() } },
                }
                Button {
                    variant: ButtonVariant::Outline,
                    aria_label: "Refresh CLI account status",
                    test_id: "accounts-refresh",
                    onclick: move |_| slots.restart(),
                    "Refresh status"
                }
            }
        }
    }
}

#[component]
pub fn AccountList(slots: Vec<AccountSlot>) -> Element {
    rsx! {
        if slots.is_empty() {
            p { role: "status", "data-testid": "accounts-empty",
                "No local CLI accounts found. Sign in with claude, codex, or agy, then restart LearnPuppy."
            }
        } else {
            ul { "aria-label": "Discovered CLI slots", "data-testid": "accounts-list",
                style: "list-style: none; padding: 0; margin: 0; max-height: 50vh; overflow-y: auto;",
                for slot in slots {
                    li { key: "{slot.id}", "data-testid": "account-slot", "data-account-id": "{slot.id}",
                        style: "padding: 0.75rem 0; border-bottom: 1px solid var(--border); overflow-wrap: anywhere;",
                        div { class: "flex items-center justify-between", style: "gap: 1rem;",
                            strong { "{slot.provider}" }
                            Badge {
                                variant: if slot.status == "Ready" { BadgeVariant::Success } else { BadgeVariant::Warning },
                                aria_label: format!("Account status: {}", slot.status),
                                test_id: "account-status",
                                "{slot.status}"
                            }
                        }
                        p { style: "margin: 0.5rem 0;", {slot.email.as_deref().unwrap_or("Default account")} }
                        if let Some(tier) = slot.tier { p { "Subscription: {tier}" } }
                        if let Some(dir) = slot.config_dir { p { "Profile: {dir.display()}" } }
                        p { "Requests this window: {slot.requests}" }
                    }
                }
            }
        }
    }
}
