use dioxus::prelude::*;
use std::sync::Arc;

use backend::{BackendHandle, DesktopBackend};
use state::{AppState, View};

pub mod backend;
pub mod shell;
pub mod state;
pub mod views;

pub fn app() -> Element {
    let backend = use_hook(|| Arc::new(DesktopBackend::from_env()) as BackendHandle);
    let initial_view = std::env::var("LP_INITIAL_VIEW")
        .ok()
        .and_then(|id| View::ALL.into_iter().find(|v| v.id() == id))
        .unwrap_or_default();
    let accounts_open = std::env::var("LP_ACCOUNTS_OPEN").is_ok_and(|v| v == "1");
    rsx! { AppWithBackend { backend, initial_view, accounts_open } }
}

#[derive(Props, Clone)]
pub struct AppWithBackendProps {
    pub backend: BackendHandle,
    #[props(default)]
    pub initial_view: View,
    #[props(default)]
    pub accounts_open: bool,
}

impl PartialEq for AppWithBackendProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.backend, &other.backend)
            && self.initial_view == other.initial_view
            && self.accounts_open == other.accounts_open
    }
}

#[component]
pub fn AppWithBackend(props: AppWithBackendProps) -> Element {
    AppState::use_provider(props.initial_view, props.accounts_open);
    use_context_provider(|| props.backend.clone());
    rsx! {
        style { {lp_ui::theme::theme_css()} }
        shell::Shell {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backend::{AccountSlot, FauxBackend};

    fn slot(provider: &str, email: &str) -> AccountSlot {
        AccountSlot {
            id: format!("{provider}:{email}"),
            provider: provider.into(),
            email: Some(email.into()),
            tier: Some("pro".into()),
            config_dir: None,
            status: "Ready".into(),
            requests: 0,
        }
    }

    fn fixture(view: View, accounts_open: bool, backend: FauxBackend) -> VirtualDom {
        VirtualDom::new_with_props(
            AppWithBackend,
            AppWithBackendProps {
                backend: Arc::new(backend),
                initial_view: view,
                accounts_open,
            },
        )
    }

    #[test]
    fn shell_renders_navigation_and_all_view_routes() {
        for view in View::ALL {
            let mut dom = fixture(view, false, FauxBackend::default());
            dom.rebuild_in_place();
            let html = dioxus::ssr::render(&dom);
            assert!(html.contains("h-screen w-screen overflow-hidden select-none flex flex-col bg-background text-foreground"));
            assert!(html.contains(&format!("data-testid=\"view-{}\"", view.id())));
            for target in View::ALL {
                assert!(html.contains(&format!("data-testid=\"nav-{}\"", target.id())));
            }
            for action in state::QuickAction::ALL {
                assert!(html.contains(&format!("data-testid=\"action-{}\"", action.id())));
            }
            assert!(!html.contains("data-testid=\"accounts-modal\""));
        }
    }

    async fn settle_accounts(dom: &mut VirtualDom, marker: &str) -> String {
        dom.rebuild_in_place();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let html = dioxus::ssr::render(dom);
                if html.contains(marker) {
                    return html;
                }
                dom.wait_for_work().await;
                dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
            }
        })
        .await
        .expect("account discovery should finish")
    }

    #[tokio::test]
    async fn accounts_modal_lists_each_subscription_without_collapsing_provider_slots() {
        let backend = FauxBackend {
            slots: vec![
                slot("claude", "personal@example.com"),
                slot("claude", "work@example.com"),
                slot("codex", "codex@example.com"),
                slot("agy", "agy@example.com"),
            ],
            error: None,
        };
        let mut dom = fixture(View::Graph, true, backend);
        let html = settle_accounts(&mut dom, "data-testid=\"accounts-list\"").await;
        assert!(html.contains("role=\"dialog\""));
        assert!(html.contains("aria-label=\"CLI accounts\""));
        for email in [
            "personal@example.com",
            "work@example.com",
            "codex@example.com",
            "agy@example.com",
        ] {
            assert!(html.contains(email));
        }
        assert_eq!(html.matches("data-testid=\"account-slot\"").count(), 4);
        assert!(html.contains("data-testid=\"accounts-refresh\""));
    }

    #[tokio::test]
    async fn accounts_modal_handles_empty_and_failed_discovery() {
        let mut empty = fixture(View::Graph, true, FauxBackend::default());
        let html = settle_accounts(&mut empty, "data-testid=\"accounts-empty\"").await;
        assert!(html.contains("No local CLI accounts found"));
        let mut failed = fixture(
            View::Graph,
            true,
            FauxBackend {
                error: Some("Cannot read profiles".into()),
                ..Default::default()
            },
        );
        let html = settle_accounts(&mut failed, "data-testid=\"accounts-error\"").await;
        assert!(html.contains("Cannot read profiles"));
        assert!(html.contains("role=\"alert\""));
    }

    #[test]
    fn accounts_shortcut_requires_command_or_control() {
        use state::is_accounts_shortcut;
        let key = Key::Character("A".into());
        assert!(is_accounts_shortcut(&key, Modifiers::META));
        assert!(is_accounts_shortcut(&key, Modifiers::CONTROL));
        assert!(!is_accounts_shortcut(&key, Modifiers::empty()));
        assert!(!is_accounts_shortcut(
            &key,
            Modifiers::META | Modifiers::ALT
        ));
        assert!(!is_accounts_shortcut(
            &Key::Character("k".into()),
            Modifiers::META
        ));
    }
}
