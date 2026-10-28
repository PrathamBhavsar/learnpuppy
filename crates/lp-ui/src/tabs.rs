//! Pure-Rust context-driven `Tabs`, `TabsList`, `TabsTrigger`, and `TabsContent` components.
//!
//! Uses Dioxus 0.7 `Signal<String>` and `use_context` for controlled and uncontrolled tab
//! switching, keyboard navigation (`ArrowLeft`, `ArrowRight`, `Home`, `End`, `Enter`, `Space`),
//! and semantic ARIA tab roles without any inline JS.

use dioxus::prelude::*;

fn join_classes(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Orientation of a [`Tabs`] group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabsOrientation {
    /// Horizontal tab strip (`ArrowLeft` / `ArrowRight`).
    #[default]
    Horizontal,
    /// Vertical tab list (`ArrowUp` / `ArrowDown`).
    Vertical,
}

impl TabsOrientation {
    /// Returns the ARIA orientation string (`"horizontal"` or `"vertical"`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// Computes the next active tab value given an ordered list of tab keys and a keyboard key.
pub fn next_tab_value(
    tabs: &[String],
    current: &str,
    key: &Key,
    orientation: TabsOrientation,
) -> Option<String> {
    if tabs.is_empty() {
        return None;
    }
    let current_idx = tabs.iter().position(|t| t == current).unwrap_or(0);
    let len = tabs.len();

    let next_idx = match (orientation, key) {
        (_, Key::Home) => Some(0),
        (_, Key::End) => Some(len - 1),
        (TabsOrientation::Horizontal, Key::ArrowRight)
        | (TabsOrientation::Vertical, Key::ArrowDown) => Some((current_idx + 1) % len),
        (TabsOrientation::Horizontal, Key::ArrowLeft)
        | (TabsOrientation::Vertical, Key::ArrowUp) => Some((current_idx + len - 1) % len),
        _ => None,
    }?;

    tabs.get(next_idx).cloned()
}

/// Context shared between [`Tabs`], [`TabsList`], [`TabsTrigger`], and [`TabsContent`].
#[derive(Clone, PartialEq)]
pub struct TabsContext {
    internal_value: Signal<String>,
    controlled_value: Option<String>,
    external_signal: Option<Signal<String>>,
    on_value_change: Option<EventHandler<String>>,
    on_change: Option<EventHandler<String>>,
    orientation: TabsOrientation,
    disabled: bool,
    ordered_tabs: Vec<String>,
}

impl TabsContext {
    /// Returns the currently active tab value.
    pub fn active_value(&self) -> String {
        if let Some(sig) = self.external_signal {
            sig()
        } else if let Some(val) = &self.controlled_value {
            val.clone()
        } else {
            (self.internal_value)()
        }
    }

    /// Returns true if `tab_value` matches the active tab.
    pub fn is_active(&self, tab_value: &str) -> bool {
        self.active_value() == tab_value
    }

    /// Sets the active tab value and fires change callbacks.
    pub fn select(&mut self, next: impl Into<String>) {
        if self.disabled {
            return;
        }
        let next_val = next.into();
        if let Some(mut sig) = self.external_signal {
            sig.set(next_val.clone());
        } else if self.controlled_value.is_none() {
            self.internal_value.set(next_val.clone());
        }
        if let Some(handler) = &self.on_value_change {
            handler.call(next_val.clone());
        }
        if let Some(handler) = &self.on_change {
            handler.call(next_val);
        }
    }

    /// Handles keyboard navigation from a tab trigger.
    pub fn handle_trigger_key(&mut self, current_tab: &str, evt: &KeyboardEvent) -> bool {
        if self.disabled {
            return false;
        }
        match evt.key() {
            Key::Enter => {
                self.select(current_tab.to_string());
                true
            }
            Key::Character(ref s) if s == " " => {
                self.select(current_tab.to_string());
                true
            }
            ref key => {
                if let Some(next) =
                    next_tab_value(&self.ordered_tabs, current_tab, key, self.orientation)
                {
                    self.select(next);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Returns the orientation of the tabs group.
    pub fn orientation(&self) -> TabsOrientation {
        self.orientation
    }

    /// Returns whether the entire tabs group is disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

/// Props for [`Tabs`].
#[derive(Props, Clone, PartialEq)]
pub struct TabsProps {
    /// Controlled active tab value.
    #[props(into, optional)]
    pub value: Option<String>,
    /// Optional reactive signal for two-way active tab binding.
    #[props(optional)]
    pub active_signal: Option<Signal<String>>,
    /// Default active tab value when uncontrolled.
    #[props(into, default)]
    pub default_value: String,
    /// Callback invoked when the active tab changes.
    #[props(default)]
    pub on_value_change: Option<EventHandler<String>>,
    /// Alias callback invoked when the active tab changes.
    #[props(default)]
    pub on_change: Option<EventHandler<String>>,
    /// Optional ordered list of tab keys for roving keyboard navigation.
    #[props(default)]
    pub tabs: Vec<String>,
    /// Orientation of the tabs (`Horizontal` or `Vertical`).
    #[props(default)]
    pub orientation: TabsOrientation,
    /// Shorthand boolean matching `dioxus-primitives` (`horizontal: true`).
    #[props(optional)]
    pub horizontal: Option<bool>,
    /// Whether all tabs are disabled.
    #[props(default = false)]
    pub disabled: bool,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "tabs".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Child `TabsList` and `TabsContent` elements.
    pub children: Element,
}

/// Root container providing [`TabsContext`] to [`TabsList`], [`TabsTrigger`], and [`TabsContent`].
#[component]
pub fn Tabs(props: TabsProps) -> Element {
    let resolved_orientation = match props.horizontal {
        Some(true) => TabsOrientation::Horizontal,
        Some(false) => TabsOrientation::Vertical,
        None => props.orientation,
    };

    let initial_default = if !props.default_value.is_empty() {
        props.default_value.clone()
    } else {
        props.tabs.first().cloned().unwrap_or_default()
    };
    let internal_value = use_signal(|| initial_default);

    let mut ctx = use_context_provider(|| TabsContext {
        internal_value,
        controlled_value: props.value.clone(),
        external_signal: props.active_signal,
        on_value_change: props.on_value_change,
        on_change: props.on_change,
        orientation: resolved_orientation,
        disabled: props.disabled,
        ordered_tabs: props.tabs.clone(),
    });

    ctx.controlled_value = props.value.clone();
    ctx.external_signal = props.active_signal;
    ctx.on_value_change = props.on_value_change;
    ctx.on_change = props.on_change;
    ctx.orientation = resolved_orientation;
    ctx.disabled = props.disabled;
    ctx.ordered_tabs = props.tabs.clone();

    let orientation_str = resolved_orientation.as_str();
    let cls = join_classes(&[
        "lp-tabs flex",
        match resolved_orientation {
            TabsOrientation::Horizontal => "flex-col gap-2",
            TabsOrientation::Vertical => "flex-row gap-4",
        },
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "{cls}",
            "data-orientation": "{orientation_str}",
            "data-disabled": if props.disabled { "true" } else { "false" },
            "data-testid": "{props.test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Props for [`TabsList`].
#[derive(Props, Clone, PartialEq)]
pub struct TabsListProps {
    /// Accessible label for the tablist.
    #[props(into, default = "Tabs".to_string())]
    pub aria_label: String,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "tabs-list".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Child `TabsTrigger` buttons.
    pub children: Element,
}

/// Container for [`TabsTrigger`] items with `role="tablist"`.
#[component]
pub fn TabsList(props: TabsListProps) -> Element {
    let ctx = try_use_context::<TabsContext>();
    let orientation = ctx
        .as_ref()
        .map(|c| c.orientation())
        .unwrap_or(TabsOrientation::Horizontal);
    let orientation_str = orientation.as_str();

    let cls = join_classes(&[
        "lp-tabs-list inline-flex items-center justify-start rounded-lg bg-muted p-1 text-muted-foreground",
        match orientation {
            TabsOrientation::Horizontal => "flex-row h-9",
            TabsOrientation::Vertical => "flex-col h-auto items-stretch",
        },
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "{cls}",
            style: "background: var(--muted, oklch(0.24 0.015 260)); color: var(--muted-foreground, oklch(0.708 0 0)); border-radius: 0.5rem; padding: 0.25rem; display: inline-flex; gap: 0.25rem;",
            role: "tablist",
            aria_label: "{props.aria_label}",
            aria_orientation: "{orientation_str}",
            "data-orientation": "{orientation_str}",
            "data-testid": "{props.test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Alias for [`TabsList`].
pub use TabsList as TabList;

/// Props for [`TabsTrigger`].
#[derive(Props, Clone, PartialEq)]
pub struct TabsTriggerProps {
    /// Unique value identifying the tab activated by this trigger.
    #[props(into)]
    pub value: String,
    /// Optional index for compatibility with `dioxus-primitives`.
    #[props(optional)]
    pub index: Option<usize>,
    /// Whether this specific tab trigger is disabled.
    #[props(default = false)]
    pub disabled: bool,
    /// Optional accessible label override.
    #[props(into, optional)]
    pub aria_label: Option<String>,
    /// Optional deterministic `data-testid` override (defaults to `tab-trigger-{value}`).
    #[props(into, optional)]
    pub test_id: Option<String>,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Trigger label/icon content.
    pub children: Element,
}

/// Interactive button (`role="tab"`) that activates its associated [`TabsContent`].
#[component]
pub fn TabsTrigger(props: TabsTriggerProps) -> Element {
    let ctx = try_use_context::<TabsContext>();
    let is_active = ctx
        .as_ref()
        .map(|c| c.is_active(&props.value))
        .unwrap_or(false);
    let is_disabled = props.disabled || ctx.as_ref().map(|c| c.is_disabled()).unwrap_or(false);

    let state_str = if is_active { "active" } else { "inactive" };
    let test_id = props
        .test_id
        .clone()
        .unwrap_or_else(|| format!("tab-trigger-{}", props.value));
    let trigger_id = format!("tab-trigger-{}", props.value);
    let content_id = format!("tab-content-{}", props.value);

    let cls = join_classes(&[
        "lp-tabs-trigger inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50",
        if is_active {
            "bg-background text-foreground shadow-sm"
        } else {
            "text-muted-foreground hover:text-foreground"
        },
        props.class.as_deref().unwrap_or(""),
    ]);

    let inline_style = if is_active {
        "background: var(--background, oklch(0.18 0.01 260)); color: var(--foreground, oklch(0.985 0 0)); border-radius: 0.375rem; padding: 0.25rem 0.75rem; font-weight: 600;"
    } else {
        "background: transparent; color: var(--muted-foreground, oklch(0.708 0 0)); border-radius: 0.375rem; padding: 0.25rem 0.75rem;"
    };

    let tab_val_click = props.value.clone();
    let tab_val_key = props.value.clone();
    let mut ctx_click = ctx.clone();
    let mut ctx_key = ctx;

    rsx! {
        button {
            r#type: "button",
            id: "{trigger_id}",
            class: "{cls}",
            style: "{inline_style}",
            role: "tab",
            aria_selected: if is_active { "true" } else { "false" },
            aria_controls: "{content_id}",
            aria_disabled: if is_disabled { "true" } else { "false" },
            aria_label: props.aria_label.as_deref().unwrap_or(&props.value),
            disabled: is_disabled,
            tabindex: if is_active { "0" } else { "-1" },
            "data-state": "{state_str}",
            "data-value": "{props.value}",
            "data-disabled": if is_disabled { "true" } else { "false" },
            "data-testid": "{test_id}",
            onclick: move |_| {
                if !is_disabled {
                    if let Some(c) = &mut ctx_click {
                        c.select(tab_val_click.clone());
                    }
                }
            },
            onkeydown: move |evt: KeyboardEvent| {
                if !is_disabled {
                    if let Some(c) = &mut ctx_key {
                        if c.handle_trigger_key(&tab_val_key, &evt) {
                            evt.prevent_default();
                        }
                    }
                }
            },
            ..props.attributes,
            {props.children}
        }
    }
}

/// Alias for [`TabsTrigger`].
pub use TabsTrigger as TabTrigger;

/// Props for [`TabsContent`].
#[derive(Props, Clone, PartialEq)]
pub struct TabsContentProps {
    /// Tab value matching a [`TabsTrigger`].
    #[props(into)]
    pub value: String,
    /// Optional index for compatibility with `dioxus-primitives`.
    #[props(optional)]
    pub index: Option<usize>,
    /// Keep inactive tab panel mounted in DOM with `hidden`.
    #[props(default = false)]
    pub force_mount: bool,
    /// Optional deterministic `data-testid` override (defaults to `tab-content-{value}`).
    #[props(into, optional)]
    pub test_id: Option<String>,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Panel content.
    pub children: Element,
}

/// Tab panel (`role="tabpanel"`) rendered when its `value` matches the active tab in [`Tabs`].
#[component]
pub fn TabsContent(props: TabsContentProps) -> Element {
    let ctx = try_use_context::<TabsContext>();
    let is_active = ctx
        .as_ref()
        .map(|c| c.is_active(&props.value))
        .unwrap_or(false);

    if !is_active && !props.force_mount {
        return rsx! {};
    }

    let state_str = if is_active { "active" } else { "inactive" };
    let test_id = props
        .test_id
        .clone()
        .unwrap_or_else(|| format!("tab-content-{}", props.value));
    let content_id = format!("tab-content-{}", props.value);
    let trigger_id = format!("tab-trigger-{}", props.value);

    let cls = join_classes(&[
        "lp-tabs-content mt-2 ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        if is_active { "" } else { "hidden" },
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            id: "{content_id}",
            class: "{cls}",
            style: if is_active { "" } else { "display: none;" },
            role: "tabpanel",
            aria_labelledby: "{trigger_id}",
            hidden: (!is_active).then_some("true"),
            tabindex: "0",
            "data-state": "{state_str}",
            "data-value": "{props.value}",
            "data-testid": "{test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Alias for [`TabsContent`].
pub use TabsContent as TabContent;

#[cfg(test)]
mod tests {
    use super::*;

    #[component]
    fn UncontrolledTabsHarness() -> Element {
        rsx! {
            Tabs {
                default_value: "notes",
                tabs: vec!["notes".to_string(), "reasoning".to_string()],
                TabsList {
                    aria_label: "Thinking Modes",
                    TabsTrigger { value: "notes", "Reading Notes" }
                    TabsTrigger { value: "reasoning", "Answer Reasoning" }
                }
                TabsContent {
                    value: "notes",
                    p { "Notes Panel Body" }
                }
                TabsContent {
                    value: "reasoning",
                    p { "Reasoning Panel Body" }
                }
            }
        }
    }

    #[component]
    fn ControlledSecondTabHarness() -> Element {
        let active = use_signal(|| "reasoning".to_string());
        rsx! {
            Tabs {
                active_signal: active,
                TabsList {
                    TabsTrigger { value: "notes", "Reading Notes" }
                    TabsTrigger { value: "reasoning", "Answer Reasoning" }
                }
                TabsContent {
                    value: "notes",
                    p { "Notes Panel Body" }
                }
                TabsContent {
                    value: "reasoning",
                    p { "Reasoning Panel Body" }
                }
            }
        }
    }

    #[test]
    fn tabs_render_default_active_panel_and_aria_roles() {
        let mut dom = VirtualDom::new(UncontrolledTabsHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"role="tablist""#));
        assert!(html.contains(r#"role="tab""#));
        assert!(html.contains(r#"role="tabpanel""#));
        assert!(html.contains(r#"data-testid="tab-trigger-notes""#));
        assert!(html.contains(r#"data-testid="tab-trigger-reasoning""#));
        assert!(html.contains(r#"data-testid="tab-content-notes""#));
        assert!(html.contains("Notes Panel Body"));
        assert!(!html.contains("Reasoning Panel Body"));
        assert!(!html.contains(r#"hidden="false""#));
        assert!(!html.contains("<script"));
    }

    #[component]
    fn ForceMountTabsHarness() -> Element {
        rsx! {
            Tabs {
                default_value: "notes",
                tabs: vec!["notes".to_string(), "reasoning".to_string()],
                TabsList {
                    TabsTrigger { value: "notes", "Reading Notes" }
                    TabsTrigger { value: "reasoning", "Answer Reasoning" }
                }
                TabsContent {
                    value: "notes",
                    force_mount: true,
                    p { "Notes Panel Body" }
                }
                TabsContent {
                    value: "reasoning",
                    force_mount: true,
                    p { "Reasoning Panel Body" }
                }
            }
        }
    }

    #[test]
    fn force_mount_tabs_omit_hidden_on_active_and_hide_inactive_panel() {
        let mut dom = VirtualDom::new(ForceMountTabsHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains("Notes Panel Body"));
        assert!(html.contains("Reasoning Panel Body"));
        assert!(!html.contains(r#"hidden="false""#));
        assert!(html.contains(r#"hidden="true""#));
        assert!(html.contains(r#"data-state="active""#));
        assert!(html.contains(r#"data-state="inactive""#));
    }

    #[test]
    fn tabs_switch_to_controlled_signal_value() {
        let mut dom = VirtualDom::new(ControlledSecondTabHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(!html.contains("Notes Panel Body"));
        assert!(html.contains("Reasoning Panel Body"));
        assert!(html.contains(r#"data-testid="tab-content-reasoning""#));
    }

    #[test]
    fn roving_keyboard_navigation_cycles_tabs() {
        let tabs = vec![
            "canvas".to_string(),
            "chains".to_string(),
            "digest".to_string(),
        ];

        assert_eq!(
            next_tab_value(
                &tabs,
                "canvas",
                &Key::ArrowRight,
                TabsOrientation::Horizontal
            ),
            Some("chains".to_string())
        );
        assert_eq!(
            next_tab_value(
                &tabs,
                "canvas",
                &Key::ArrowLeft,
                TabsOrientation::Horizontal
            ),
            Some("digest".to_string())
        );
        assert_eq!(
            next_tab_value(&tabs, "chains", &Key::Home, TabsOrientation::Horizontal),
            Some("canvas".to_string())
        );
        assert_eq!(
            next_tab_value(&tabs, "canvas", &Key::End, TabsOrientation::Horizontal),
            Some("digest".to_string())
        );
    }
}
