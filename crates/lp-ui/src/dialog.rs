//! Pure-Rust controlled and uncontrolled modal `Dialog` primitives for Dioxus 0.7.
//!
//! Eliminates inline `<script>` polling and `wasm-bindgen` listeners in favor of
//! pure Dioxus `Signal<bool>` state, `use_context`, `Key::Escape` keyboard handling,
//! and backdrop click dismissal.

use dioxus::prelude::*;

fn join_classes(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Shared context provided by [`Dialog`] to its descendant components.
#[derive(Clone, Copy, PartialEq)]
pub struct DialogCtx {
    open_state: Signal<bool>,
    controlled_open: Option<bool>,
    external_signal: Option<Signal<bool>>,
    on_open_change: Option<EventHandler<bool>>,
    on_close: Option<EventHandler<()>>,
    title_id: Signal<String>,
    description_id: Signal<String>,
}

impl DialogCtx {
    /// Returns whether the dialog is currently open.
    pub fn is_open(&self) -> bool {
        if let Some(sig) = self.external_signal {
            sig()
        } else if let Some(open) = self.controlled_open {
            open
        } else {
            (self.open_state)()
        }
    }

    /// Updates the open state of the dialog and notifies listeners.
    pub fn set_open(&mut self, next: bool) {
        if let Some(mut sig) = self.external_signal {
            sig.set(next);
        } else if self.controlled_open.is_none() {
            self.open_state.set(next);
        }
        if let Some(handler) = &self.on_open_change {
            handler.call(next);
        }
        if !next {
            if let Some(close_handler) = &self.on_close {
                close_handler.call(());
            }
        }
    }

    /// Opens the dialog.
    pub fn open(&mut self) {
        self.set_open(true);
    }

    /// Closes the dialog.
    pub fn close(&mut self) {
        self.set_open(false);
    }

    /// Toggles the dialog open/closed state.
    pub fn toggle(&mut self) {
        let next = !self.is_open();
        self.set_open(next);
    }

    /// Element ID for `aria-labelledby`.
    pub fn title_id(&self) -> String {
        (self.title_id)()
    }

    /// Element ID for `aria-describedby`.
    pub fn description_id(&self) -> String {
        (self.description_id)()
    }
}

/// Returns true if the given key event should dismiss a modal dialog.
pub fn is_dialog_escape_key(key: &Key) -> bool {
    matches!(key, Key::Escape)
}

/// Props for the [`Dialog`] component.
#[derive(Props, Clone, PartialEq)]
pub struct DialogProps {
    /// Controlled open boolean value.
    #[props(optional)]
    pub open: Option<bool>,
    /// Optional reactive signal for two-way open binding.
    #[props(optional)]
    pub open_signal: Option<Signal<bool>>,
    /// Initial open state when uncontrolled.
    #[props(default = false)]
    pub default_open: bool,
    /// Callback invoked when the dialog requests an open state change.
    #[props(default)]
    pub on_open_change: Option<EventHandler<bool>>,
    /// Callback invoked when the dialog closes.
    #[props(default)]
    pub on_close: Option<EventHandler<()>>,
    /// Optional shorthand title rendered when using `Dialog` directly.
    #[props(into, optional)]
    pub title: Option<String>,
    /// Optional shorthand description rendered below `title`.
    #[props(into, optional)]
    pub description: Option<String>,
    /// Accessible label for the dialog container.
    #[props(into, default = "Dialog".to_string())]
    pub aria_label: String,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "dialog".to_string())]
    pub test_id: String,
    /// Whether clicking the backdrop closes the dialog.
    #[props(default = true)]
    pub close_on_backdrop_click: bool,
    /// Whether pressing `Escape` closes the dialog.
    #[props(default = true)]
    pub close_on_escape: bool,
    /// Keep DOM mounted with `hidden` when closed.
    #[props(default = false)]
    pub keep_mounted: bool,
    /// Optional extra CSS classes for the dialog container.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Dialog children (`DialogContent`, `DialogHeader`, etc.).
    pub children: Element,
}

/// Root modal dialog component with pure-Rust `Signal<bool>` state, backdrop click
/// dismissal, and `Escape` key handling.
#[component]
pub fn Dialog(props: DialogProps) -> Element {
    let internal_open = use_signal(|| props.default_open);
    let title_id = use_signal(|| format!("{}-title", props.test_id));
    let description_id = use_signal(|| format!("{}-description", props.test_id));

    let mut ctx = use_context_provider(|| DialogCtx {
        open_state: internal_open,
        controlled_open: props.open,
        external_signal: props.open_signal,
        on_open_change: props.on_open_change,
        on_close: props.on_close,
        title_id,
        description_id,
    });

    // Keep controlled props synchronized across re-renders without mutating signals in render.
    ctx.controlled_open = props.open;
    ctx.external_signal = props.open_signal;
    ctx.on_open_change = props.on_open_change;
    ctx.on_close = props.on_close;

    let is_open = ctx.is_open();
    if !is_open && !props.keep_mounted {
        return rsx! {};
    }

    let close_on_escape = props.close_on_escape;
    let close_on_backdrop = props.close_on_backdrop_click;
    let state_str = if is_open { "open" } else { "closed" };
    let root_class = join_classes(&[
        "lp-dialog-overlay fixed inset-0 z-50 flex items-center justify-center p-4",
        if is_open { "" } else { "hidden" },
        props.class.as_deref().unwrap_or(""),
    ]);
    let backdrop_test_id = format!("{}-backdrop", props.test_id);
    let labelledby = ctx.title_id();
    let describedby = ctx.description_id();

    rsx! {
        div {
            class: "{root_class}",
            style: if is_open {
                "position: fixed; inset: 0; z-index: 50; display: flex; align-items: center; justify-content: center; padding: 1rem;"
            } else {
                "display: none;"
            },
            role: "dialog",
            aria_modal: "true",
            aria_label: "{props.aria_label}",
            aria_labelledby: "{labelledby}",
            aria_describedby: "{describedby}",
            "data-state": "{state_str}",
            "data-testid": "{props.test_id}",
            hidden: (!is_open).then_some("true"),
            tabindex: "-1",
            onkeydown: move |evt: KeyboardEvent| {
                if close_on_escape && is_dialog_escape_key(&evt.key()) {
                    evt.stop_propagation();
                    ctx.close();
                }
            },
            ..props.attributes,

            // Backdrop layer
            div {
                class: "lp-dialog-backdrop fixed inset-0 bg-black/75 backdrop-blur-sm transition-opacity",
                style: "position: fixed; inset: 0; background: oklch(0.08 0.01 260 / 0.78); backdrop-filter: blur(4px);",
                "data-testid": "{backdrop_test_id}",
                "data-state": "{state_str}",
                aria_hidden: "true",
                onclick: move |_| {
                    if close_on_backdrop {
                        ctx.close();
                    }
                },
            }

            // Foreground wrapper (stops click events from bubbling to backdrop)
            div {
                class: "lp-dialog-stage relative z-10 flex w-full max-w-lg items-center justify-center",
                style: "position: relative; z-index: 10; width: 100%; max-width: 32rem;",
                onclick: move |evt: MouseEvent| {
                    evt.stop_propagation();
                },
                if let Some(title_text) = &props.title {
                    div {
                        class: "lp-dialog-panel w-full rounded-none border border-border bg-card p-6 text-card-foreground shadow-2xl",
                        style: "background: var(--card, oklch(0.205 0.015 260)); color: var(--card-foreground, oklch(0.985 0 0)); border: 1px solid var(--border, oklch(1 0 0 / 12%)); border-radius: 0; padding: 1.5rem;",
                        div {
                            class: "lp-dialog-header flex flex-col space-y-1.5 pb-4",
                            h2 {
                                id: "{labelledby}",
                                class: "lp-dialog-title font-mono text-sm font-medium leading-none text-foreground",
                                "{title_text}"
                            }
                            if let Some(desc_text) = &props.description {
                                p {
                                    id: "{describedby}",
                                    class: "lp-dialog-description text-xs text-muted-foreground",
                                    "{desc_text}"
                                }
                            }
                        }
                        {props.children}
                    }
                } else {
                    {props.children}
                }
            }
        }
    }
}

/// Alias for [`Dialog`] matching `dioxus-primitives` naming conventions.
pub use Dialog as DialogRoot;

/// Props for [`DialogTrigger`].
#[derive(Props, Clone, PartialEq)]
pub struct DialogTriggerProps {
    /// Optional `data-testid` attribute.
    #[props(into, default = "dialog-trigger".to_string())]
    pub test_id: String,
    /// Accessible label for the trigger button.
    #[props(into, default = "Open dialog".to_string())]
    pub aria_label: String,
    /// Optional custom class names.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Optional click callback invoked in addition to opening the dialog context.
    #[props(default)]
    pub onclick: Option<EventHandler<MouseEvent>>,
    /// Forwarded global attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Trigger child content.
    pub children: Element,
}

/// Button that opens the enclosing [`Dialog`] when clicked.
#[component]
pub fn DialogTrigger(props: DialogTriggerProps) -> Element {
    let mut ctx = try_use_context::<DialogCtx>();
    let cls = join_classes(&[
        "lp-dialog-trigger inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        button {
            r#type: "button",
            class: "{cls}",
            aria_label: "{props.aria_label}",
            aria_haspopup: "dialog",
            aria_expanded: ctx.map(|c| c.is_open()).unwrap_or(false),
            "data-testid": "{props.test_id}",
            onclick: move |evt| {
                if let Some(c) = &mut ctx {
                    c.open();
                }
                if let Some(handler) = &props.onclick {
                    handler.call(evt);
                }
            },
            ..props.attributes,
            {props.children}
        }
    }
}

/// Props for [`DialogContent`].
#[derive(Props, Clone, PartialEq)]
pub struct DialogContentProps {
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "dialog-content".to_string())]
    pub test_id: String,
    /// Accessible label for the dialog surface.
    #[props(into, optional)]
    pub aria_label: Option<String>,
    /// Whether to render the top-right close (`×`) button.
    #[props(default = true)]
    pub show_close_button: bool,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Modal body children.
    pub children: Element,
}

/// Styled modal surface inside a [`Dialog`].
#[component]
pub fn DialogContent(props: DialogContentProps) -> Element {
    let mut ctx = try_use_context::<DialogCtx>();
    let cls = join_classes(&[
        "lp-dialog-content relative grid w-full max-w-lg gap-4 rounded-none border border-border bg-card p-6 text-card-foreground shadow-2xl outline-none",
        props.class.as_deref().unwrap_or(""),
    ]);
    let close_test_id = format!("{}-close", props.test_id);

    rsx! {
        div {
            class: "{cls}",
            style: "position: relative; width: 100%; background: var(--card, oklch(0.205 0.015 260)); color: var(--card-foreground, oklch(0.985 0 0)); border: 1px solid var(--border, oklch(1 0 0 / 12%)); border-radius: 0; padding: 1.5rem; box-shadow: 0 24px 48px oklch(0 0 0 / 0.55);",
            "data-state": "open",
            "data-testid": "{props.test_id}",
            aria_label: props.aria_label.as_deref().unwrap_or("Dialog content"),
            onclick: move |evt: MouseEvent| {
                evt.stop_propagation();
            },
            ..props.attributes,

            {props.children}

            if props.show_close_button {
                button {
                    r#type: "button",
                    class: "lp-dialog-close absolute right-4 top-4 inline-flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground opacity-80 transition-opacity hover:bg-accent hover:text-accent-foreground hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring",
                    style: "position: absolute; top: 0.875rem; right: 0.875rem; display: inline-flex; align-items: center; justify-content: center; width: 1.75rem; height: 1.75rem; border-radius: 0.375rem;",
                    aria_label: "Close dialog",
                    "data-testid": "{close_test_id}",
                    onclick: move |_| {
                        if let Some(c) = &mut ctx {
                            c.close();
                        }
                    },
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "16",
                        height: "16",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        "aria-hidden": "true",
                        path { d: "M18 6 6 18" }
                        path { d: "m6 6 12 12" }
                    }
                }
            }
        }
    }
}

/// Props for [`DialogHeader`].
#[derive(Props, Clone, PartialEq)]
pub struct DialogHeaderProps {
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "dialog-header".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Header children (`DialogTitle`, `DialogDescription`).
    pub children: Element,
}

/// Header container for [`DialogTitle`] and [`DialogDescription`].
#[component]
pub fn DialogHeader(props: DialogHeaderProps) -> Element {
    let cls = join_classes(&[
        "lp-dialog-header flex flex-col space-y-1.5 text-left",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "{cls}",
            "data-testid": "{props.test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Props for [`DialogTitle`].
#[derive(Props, Clone, PartialEq)]
pub struct DialogTitleProps {
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "dialog-title".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Title content.
    pub children: Element,
}

/// Semantic heading (`h2`) for a [`Dialog`].
#[component]
pub fn DialogTitle(props: DialogTitleProps) -> Element {
    let ctx = try_use_context::<DialogCtx>();
    let id = ctx
        .map(|c| c.title_id())
        .unwrap_or_else(|| "dialog-title".to_string());
    let cls = join_classes(&[
        "lp-dialog-title font-mono text-sm font-medium leading-none text-foreground",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        h2 {
            id: "{id}",
            class: "{cls}",
            "data-testid": "{props.test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Props for [`DialogDescription`].
#[derive(Props, Clone, PartialEq)]
pub struct DialogDescriptionProps {
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "dialog-description".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Description content.
    pub children: Element,
}

/// Muted description paragraph for a [`Dialog`].
#[component]
pub fn DialogDescription(props: DialogDescriptionProps) -> Element {
    let ctx = try_use_context::<DialogCtx>();
    let id = ctx
        .map(|c| c.description_id())
        .unwrap_or_else(|| "dialog-description".to_string());
    let cls = join_classes(&[
        "lp-dialog-description text-xs text-muted-foreground",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        p {
            id: "{id}",
            class: "{cls}",
            "data-testid": "{props.test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Props for [`DialogFooter`].
#[derive(Props, Clone, PartialEq)]
pub struct DialogFooterProps {
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "dialog-footer".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Footer action buttons.
    pub children: Element,
}

/// Action bar footer for a [`Dialog`].
#[component]
pub fn DialogFooter(props: DialogFooterProps) -> Element {
    let cls = join_classes(&[
        "lp-dialog-footer flex flex-row items-center justify-end gap-2 pt-2",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "{cls}",
            "data-testid": "{props.test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Props for [`DialogClose`].
#[derive(Props, Clone, PartialEq)]
pub struct DialogCloseProps {
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "dialog-close".to_string())]
    pub test_id: String,
    /// Accessible label for the close button.
    #[props(into, default = "Close dialog".to_string())]
    pub aria_label: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Button children.
    pub children: Element,
}

/// Explicit close button inside [`DialogFooter`] or [`DialogContent`].
#[component]
pub fn DialogClose(props: DialogCloseProps) -> Element {
    let mut ctx = try_use_context::<DialogCtx>();
    let cls = join_classes(&[
        "lp-dialog-close-btn inline-flex items-center justify-center rounded-md border border-border bg-secondary px-3 py-1.5 text-sm font-medium text-secondary-foreground transition-colors hover:bg-accent focus:outline-none focus:ring-2 focus:ring-ring",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        button {
            r#type: "button",
            class: "{cls}",
            aria_label: "{props.aria_label}",
            "data-testid": "{props.test_id}",
            onclick: move |_| {
                if let Some(c) = &mut ctx {
                    c.close();
                }
            },
            ..props.attributes,
            {props.children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[component]
    fn OpenDialogHarness() -> Element {
        rsx! {
            Dialog {
                open: true,
                test_id: "accounts-modal",
                aria_label: "CLI Accounts Modal",
                DialogContent {
                    test_id: "accounts-modal-content",
                    DialogHeader {
                        DialogTitle { "Multi-Account CLI Pool" }
                        DialogDescription { "Manage Claude, Codex, and AGY subscriptions." }
                    }
                    div { "Active Account #1" }
                    DialogFooter {
                        DialogClose { "Done" }
                    }
                }
            }
        }
    }

    #[component]
    fn ClosedDialogHarness() -> Element {
        rsx! {
            Dialog {
                open: false,
                test_id: "hidden-modal",
                DialogContent {
                    DialogHeader {
                        DialogTitle { "Should Not Render" }
                    }
                }
            }
        }
    }

    #[component]
    fn SignalDialogHarness() -> Element {
        let open_sig = use_signal(|| true);
        rsx! {
            Dialog {
                open_signal: open_sig,
                title: "Shorthand Title",
                description: "Shorthand Description",
                div { "Inner payload" }
            }
        }
    }

    #[test]
    fn dialog_renders_open_state_with_aria_and_testids() {
        let mut dom = VirtualDom::new(OpenDialogHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"role="dialog""#));
        assert!(html.contains(r#"aria-modal="true""#));
        assert!(html.contains(r#"data-testid="accounts-modal""#));
        assert!(html.contains(r#"data-testid="accounts-modal-backdrop""#));
        assert!(html.contains(r#"data-testid="accounts-modal-content""#));
        assert!(html.contains(r#"data-state="open""#));
        assert!(html.contains("Multi-Account CLI Pool"));
        assert!(html.contains("Manage Claude, Codex, and AGY subscriptions."));
        assert!(html.contains("Active Account #1"));
        assert!(!html.contains(r#"hidden="false""#));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn dialog_omits_markup_when_closed() {
        let mut dom = VirtualDom::new(ClosedDialogHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(!html.contains("Should Not Render"));
        assert!(!html.contains(r#"data-testid="hidden-modal""#));
    }

    #[test]
    fn dialog_supports_signal_and_shorthand_title() {
        let mut dom = VirtualDom::new(SignalDialogHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains("Shorthand Title"));
        assert!(html.contains("Shorthand Description"));
        assert!(html.contains("Inner payload"));
    }

    #[test]
    fn escape_key_detection_matches_escape_only() {
        assert!(is_dialog_escape_key(&Key::Escape));
        assert!(!is_dialog_escape_key(&Key::Enter));
        assert!(!is_dialog_escape_key(&Key::Character("a".to_string())));
    }
}
