//! `CommandPalette` (`Cmd+K` / `Ctrl+K`) modal with live fuzzy filtering, keyboard
//! `ArrowUp`/`ArrowDown`/`Enter`/`Escape` navigation, and shortcut badges.
//!
//! Built with 100% pure Rust `Signal<T>` state and `evt.prevent_default()` form handling
//! for native macOS `WKWebView` compatibility.

use dioxus::prelude::*;

fn join_classes(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A single actionable command or proposition entry in the [`CommandPalette`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    /// Stable identifier passed to `on_select` when chosen.
    pub id: String,
    /// Primary display title.
    pub label: String,
    /// Optional secondary subtitle or provenance description.
    pub description: Option<String>,
    /// Optional group/category header (e.g. `"Navigation"`, `"Classroom"`, `"Graph"`).
    pub category: Option<String>,
    /// Optional keyboard shortcut badge (e.g. `"⌘1"`, `"⌘2"`, `"⌘A"`).
    pub shortcut: Option<String>,
    /// Additional search keywords matched during fuzzy filtering.
    pub keywords: Vec<String>,
    /// Whether the command is currently disabled.
    pub disabled: bool,
}

impl CommandItem {
    /// Creates a new enabled [`CommandItem`] with the given `id` and `label`.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            category: None,
            shortcut: None,
            keywords: Vec::new(),
            disabled: false,
        }
    }

    /// Sets the secondary description text.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the category/group label.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Sets the keyboard shortcut badge (e.g. `"⌘1"`).
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Appends searchable keywords for fuzzy matching.
    pub fn with_keywords<I, S>(mut self, keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.keywords = keywords.into_iter().map(Into::into).collect();
        self
    }

    /// Sets whether this item is disabled.
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Computes a case-insensitive fuzzy match score for `query` against `candidate`.
///
/// Returns `Some(score)` when every non-whitespace character in `query` appears in order
/// within `candidate`, rewarding prefix, word-boundary, and consecutive character runs.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Some(0);
    }
    let cand = candidate.to_lowercase();
    if cand == q {
        return Some(1_000);
    }
    if cand.starts_with(&q) {
        return Some(800 - (cand.len() as i32 - q.len() as i32).max(0));
    }
    if let Some(pos) = cand.find(&q) {
        return Some(600 - (pos as i32 * 4));
    }

    let cand_chars: Vec<char> = cand.chars().collect();
    let mut score = 0i32;
    let mut search_idx = 0usize;
    let mut prev_match_idx: Option<usize> = None;

    for qc in q.chars().filter(|c| !c.is_whitespace()) {
        let mut found = None;
        for (idx, &cc) in cand_chars.iter().enumerate().skip(search_idx) {
            if cc == qc {
                found = Some(idx);
                break;
            }
        }
        let match_idx = found?;
        score += 15;
        if match_idx == 0 {
            score += 25;
        } else {
            let prev_char = cand_chars[match_idx - 1];
            if prev_char == ' ' || prev_char == '-' || prev_char == '_' || prev_char == ':' {
                score += 20;
            }
        }
        if let Some(prev) = prev_match_idx {
            if match_idx == prev + 1 {
                score += 18;
            } else {
                score -= (match_idx - prev - 1) as i32;
            }
        }
        prev_match_idx = Some(match_idx);
        search_idx = match_idx + 1;
    }

    Some(score)
}

/// Filters and ranks `commands` by fuzzy matching `query` against `label`, `category`,
/// `description`, `id`, and `keywords`.
pub fn filter_commands(commands: &[CommandItem], query: &str) -> Vec<CommandItem> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return commands.to_vec();
    }

    let mut scored: Vec<(i32, usize, CommandItem)> = commands
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let label_score = fuzzy_score(trimmed, &item.label).map(|s| s + 50);
            let id_score = fuzzy_score(trimmed, &item.id);
            let cat_score = item
                .category
                .as_deref()
                .and_then(|c| fuzzy_score(trimmed, c));
            let desc_score = item
                .description
                .as_deref()
                .and_then(|d| fuzzy_score(trimmed, d));
            let kw_score = item
                .keywords
                .iter()
                .filter_map(|k| fuzzy_score(trimmed, k))
                .max();

            let best = [label_score, id_score, cat_score, desc_score, kw_score]
                .into_iter()
                .flatten()
                .max()?;
            Some((best, idx, item.clone()))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, item)| item).collect()
}

/// Semantic keyboard action recognized inside [`CommandPalette`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKeyAction {
    /// `Cmd+K` or `Ctrl+K` toggles the palette open/closed.
    ToggleOpen,
    /// `Escape` closes the palette.
    Close,
    /// `ArrowDown` moves selection to the next command.
    MoveNext,
    /// `ArrowUp` moves selection to the previous command.
    MovePrev,
    /// `Home` jumps to the first command.
    MoveFirst,
    /// `End` jumps to the last command.
    MoveLast,
    /// `Enter` executes the currently highlighted command.
    ExecuteSelected,
}

/// Returns true when the key + modifiers correspond to `Cmd+K` (macOS) or `Ctrl+K` (Windows/Linux).
pub fn is_command_palette_shortcut(key: &Key, modifiers: Modifiers) -> bool {
    let has_cmd_or_ctrl = modifiers.meta() || modifiers.ctrl();
    if !has_cmd_or_ctrl {
        return false;
    }
    matches!(key, Key::Character(c) if c.eq_ignore_ascii_case("k"))
}

/// Returns true when a Dioxus [`KeyboardEvent`] matches `Cmd+K` or `Ctrl+K`.
pub fn is_command_palette_event(evt: &KeyboardEvent) -> bool {
    is_command_palette_shortcut(&evt.key(), evt.modifiers())
}

/// Maps a keyboard key and modifier set to a [`CommandKeyAction`].
pub fn classify_command_palette_key(key: &Key, modifiers: Modifiers) -> Option<CommandKeyAction> {
    if is_command_palette_shortcut(key, modifiers) {
        return Some(CommandKeyAction::ToggleOpen);
    }
    match key {
        Key::Escape => Some(CommandKeyAction::Close),
        Key::ArrowDown => Some(CommandKeyAction::MoveNext),
        Key::ArrowUp => Some(CommandKeyAction::MovePrev),
        Key::Home => Some(CommandKeyAction::MoveFirst),
        Key::End => Some(CommandKeyAction::MoveLast),
        Key::Enter => Some(CommandKeyAction::ExecuteSelected),
        _ => None,
    }
}

/// Computes the next highlighted index in a filtered command list of length `len`.
pub fn step_selected_index(current: usize, len: usize, action: CommandKeyAction) -> usize {
    if len == 0 {
        return 0;
    }
    let clamped = current.min(len - 1);
    match action {
        CommandKeyAction::MoveNext => (clamped + 1) % len,
        CommandKeyAction::MovePrev => (clamped + len - 1) % len,
        CommandKeyAction::MoveFirst => 0,
        CommandKeyAction::MoveLast => len - 1,
        _ => clamped,
    }
}

/// Props for [`CommandPalette`].
#[derive(Props, Clone, PartialEq)]
pub struct CommandPaletteProps {
    /// Controlled open boolean state.
    #[props(optional)]
    pub open: Option<bool>,
    /// Optional reactive signal for two-way open state binding.
    #[props(optional)]
    pub open_signal: Option<Signal<bool>>,
    /// Default open state when uncontrolled.
    #[props(default = false)]
    pub default_open: bool,
    /// Callback invoked when the open state changes.
    #[props(default)]
    pub on_open_change: Option<EventHandler<bool>>,
    /// Callback invoked when the palette closes.
    #[props(default)]
    pub on_close: Option<EventHandler<()>>,
    /// Available command items.
    #[props(default)]
    pub commands: Vec<CommandItem>,
    /// Alias for `commands` (`items`).
    #[props(default)]
    pub items: Vec<CommandItem>,
    /// Controlled search query string.
    #[props(into, optional)]
    pub query: Option<String>,
    /// Optional reactive signal for the search query string.
    #[props(optional)]
    pub query_signal: Option<Signal<String>>,
    /// Initial search query when uncontrolled.
    #[props(into, default)]
    pub default_query: String,
    /// Callback invoked when the search query changes.
    #[props(default)]
    pub on_query_change: Option<EventHandler<String>>,
    /// Initial highlighted item index (defaults to `0`).
    #[props(default = 0)]
    pub default_selected_index: usize,
    /// Callback invoked with `item.id` when a command is executed.
    #[props(default)]
    pub on_select: Option<EventHandler<String>>,
    /// Callback invoked with the full [`CommandItem`] when a command is executed.
    #[props(default)]
    pub on_select_item: Option<EventHandler<CommandItem>>,
    /// Placeholder text inside the search input.
    #[props(into, default = "Type a command or search propositions... (⌘K)".to_string())]
    pub placeholder: String,
    /// Message displayed when no commands match the current query.
    #[props(into, default = "No matching commands found.".to_string())]
    pub empty_message: String,
    /// Accessible label for the command palette dialog.
    #[props(into, default = "Command Palette".to_string())]
    pub aria_label: String,
    /// Whether to render a compact `⌘K` trigger button when closed.
    #[props(default = false)]
    pub show_trigger: bool,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "command-palette".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// Modal `Cmd+K` / `Ctrl+K` Command Palette with live fuzzy filtering, keyboard navigation,
/// and `evt.prevent_default()` form handling.
#[component]
pub fn CommandPalette(props: CommandPaletteProps) -> Element {
    let mut internal_open = use_signal(|| props.default_open);
    let mut internal_query = use_signal(|| props.default_query.clone());
    let mut selected_idx = use_signal(|| props.default_selected_index);

    let is_open = if let Some(sig) = props.open_signal {
        sig()
    } else if let Some(open) = props.open {
        open
    } else {
        internal_open()
    };

    let mut was_open = use_signal(|| is_open);
    if *was_open.peek() != is_open {
        was_open.set(is_open);
        if !is_open {
            selected_idx.set(0);
            if let Some(mut q_sig) = props.query_signal {
                q_sig.set(String::new());
            } else if props.query.is_none() {
                internal_query.set(String::new());
            }
        }
    }

    let current_query = if let Some(sig) = props.query_signal {
        sig()
    } else if let Some(q) = &props.query {
        q.clone()
    } else {
        internal_query()
    };

    let source_commands = if !props.commands.is_empty() {
        &props.commands
    } else {
        &props.items
    };
    let filtered = filter_commands(source_commands, &current_query);
    let filtered_len = filtered.len();
    let active_idx = if filtered_len == 0 {
        0
    } else {
        selected_idx().min(filtered_len - 1)
    };

    let set_open = {
        let external_sig = props.open_signal;
        let controlled = props.open;
        let external_query_sig = props.query_signal;
        let controlled_q = props.query.is_some();
        let on_query_change = props.on_query_change;
        let on_open_change = props.on_open_change;
        let on_close = props.on_close;
        move |next: bool| {
            if !next {
                selected_idx.set(0);
                if let Some(mut q_sig) = external_query_sig {
                    q_sig.set(String::new());
                } else if !controlled_q {
                    internal_query.set(String::new());
                }
                if let Some(cb) = &on_query_change {
                    cb.call(String::new());
                }
            }
            if let Some(mut sig) = external_sig {
                sig.set(next);
            } else if controlled.is_none() {
                internal_open.set(next);
            }
            if let Some(cb) = &on_open_change {
                cb.call(next);
            }
            if !next {
                if let Some(cb) = &on_close {
                    cb.call(());
                }
            }
        }
    };

    let set_query = {
        let external_sig = props.query_signal;
        let controlled_q = props.query.is_some();
        let on_query_change = props.on_query_change;
        move |next: String| {
            selected_idx.set(0);
            if let Some(mut sig) = external_sig {
                sig.set(next.clone());
            } else if !controlled_q {
                internal_query.set(next.clone());
            }
            if let Some(cb) = &on_query_change {
                cb.call(next);
            }
        }
    };

    if !is_open {
        if !props.show_trigger {
            return rsx! {};
        }
        let mut set_open_trigger = set_open;
        let mut set_open_key = set_open;
        let trigger_test_id = format!("{}-trigger", props.test_id);
        return rsx! {
            button {
                r#type: "button",
                class: "lp-command-palette-trigger inline-flex items-center gap-2 rounded-md border border-border bg-card px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                aria_label: "Open Command Palette (Cmd+K)",
                aria_haspopup: "dialog",
                aria_expanded: "false",
                "data-testid": "{trigger_test_id}",
                onclick: move |_| set_open_trigger(true),
                onkeydown: move |evt: KeyboardEvent| {
                    if is_command_palette_event(&evt) {
                        evt.prevent_default();
                        set_open_key(true);
                    }
                },
                span { "Search commands..." }
                kbd {
                    class: "rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground",
                    "⌘K"
                }
            }
        };
    }

    let backdrop_test_id = format!("{}-backdrop", props.test_id);
    let form_test_id = format!("{}-form", props.test_id);
    let input_test_id = format!("{}-input", props.test_id);
    let list_test_id = format!("{}-list", props.test_id);
    let empty_test_id = format!("{}-empty", props.test_id);
    let active_option_id = filtered
        .get(active_idx)
        .map(|item| format!("{}-item-{}", props.test_id, item.id))
        .unwrap_or_default();

    let filtered_for_key = filtered.clone();
    let filtered_for_submit = filtered.clone();
    let on_select = props.on_select;
    let on_select_item = props.on_select_item;

    let mut set_open_backdrop = set_open;
    let mut set_open_key = set_open;
    let mut set_open_submit = set_open;
    let mut set_query_input = set_query;

    let root_cls = join_classes(&[
        "lp-command-palette-overlay fixed inset-0 z-50 flex items-start justify-center pt-20 p-4",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "{root_cls}",
            style: "position: fixed; inset: 0; z-index: 50; display: flex; align-items: flex-start; justify-content: center; padding-top: 5rem; padding-left: 1rem; padding-right: 1rem;",
            role: "dialog",
            aria_modal: "true",
            aria_label: "{props.aria_label}",
            "data-state": "open",
            "data-testid": "{props.test_id}",
            tabindex: "-1",
            onkeydown: move |evt: KeyboardEvent| {
                if let Some(action) = classify_command_palette_key(&evt.key(), evt.modifiers()) {
                    match action {
                        CommandKeyAction::ToggleOpen | CommandKeyAction::Close => {
                            evt.prevent_default();
                            evt.stop_propagation();
                            set_open_key(false);
                        }
                        CommandKeyAction::MoveNext
                        | CommandKeyAction::MovePrev
                        | CommandKeyAction::MoveFirst
                        | CommandKeyAction::MoveLast => {
                            evt.prevent_default();
                            let next = step_selected_index(active_idx, filtered_len, action);
                            selected_idx.set(next);
                        }
                        CommandKeyAction::ExecuteSelected => {
                            evt.prevent_default();
                            if let Some(chosen) = filtered_for_key.get(active_idx) {
                                if !chosen.disabled {
                                    if let Some(cb) = &on_select {
                                        cb.call(chosen.id.clone());
                                    }
                                    if let Some(cb) = &on_select_item {
                                        cb.call(chosen.clone());
                                    }
                                    set_open_key(false);
                                }
                            }
                        }
                    }
                }
            },
            ..props.attributes,

            // Backdrop
            div {
                class: "lp-command-palette-backdrop fixed inset-0 bg-black/75 backdrop-blur-sm",
                style: "position: fixed; inset: 0; background: oklch(0.08 0.01 260 / 0.78); backdrop-filter: blur(4px);",
                "data-testid": "{backdrop_test_id}",
                aria_hidden: "true",
                onclick: move |_| set_open_backdrop(false),
            }

            // Palette Surface
            div {
                class: "lp-command-palette-panel relative z-10 flex w-full max-w-xl flex-col overflow-hidden rounded-none border border-border bg-popover text-popover-foreground ring-1 ring-foreground/10 shadow-2xl",
                style: "position: relative; z-index: 10; width: 100%; max-width: 36rem; background: var(--popover, oklch(0.21 0.015 260)); color: var(--popover-foreground, oklch(0.985 0 0)); border: 1px solid var(--border, oklch(1 0 0 / 14%)); overflow: hidden; box-shadow: 0 24px 48px oklch(0 0 0 / 0.6);",
                onclick: move |evt: MouseEvent| evt.stop_propagation(),

                // Search Form (strictly calls evt.prevent_default() on onsubmit)
                form {
                    class: "lp-command-palette-form flex items-center gap-2 border-b border-border px-3 py-2.5",
                    style: "display: flex; align-items: center; gap: 0.625rem; padding: 0.625rem 0.875rem; border-bottom: 1px solid var(--border, oklch(1 0 0 / 12%));",
                    role: "search",
                    aria_label: "Filter commands",
                    "data-testid": "{form_test_id}",
                    onsubmit: move |evt: FormEvent| {
                        evt.prevent_default();
                        if let Some(chosen) = filtered_for_submit.get(active_idx) {
                            if !chosen.disabled {
                                if let Some(cb) = &on_select {
                                    cb.call(chosen.id.clone());
                                }
                                if let Some(cb) = &on_select_item {
                                    cb.call(chosen.clone());
                                }
                                set_open_submit(false);
                            }
                        }
                    },

                    // Search Magnifier SVG Icon
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
                        class: "shrink-0 text-muted-foreground",
                        "aria-hidden": "true",
                        circle { cx: "11", cy: "11", r: "8" }
                        path { d: "m21 21-4.3-4.3" }
                    }

                    input {
                        r#type: "text",
                        class: "lp-command-palette-input flex h-7 w-full bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none",
                        style: "width: 100%; background: transparent; border: none; outline: none; color: inherit; font-size: 0.875rem;",
                        role: "combobox",
                        aria_label: "{props.placeholder}",
                        aria_expanded: "true",
                        aria_autocomplete: "list",
                        aria_controls: "{list_test_id}",
                        aria_activedescendant: "{active_option_id}",
                        placeholder: "{props.placeholder}",
                        value: "{current_query}",
                        autofocus: true,
                        "data-testid": "{input_test_id}",
                        oninput: move |evt: FormEvent| {
                            set_query_input(evt.value());
                        },
                    }

                    kbd {
                        class: "lp-command-palette-esc hidden rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground sm:inline-block",
                        "ESC"
                    }
                }

                // Filtered Results Listbox
                div {
                    id: "{list_test_id}",
                    class: "lp-command-palette-list max-h-80 overflow-y-auto p-1.5",
                    style: "max-height: 20rem; overflow-y: auto; padding: 0.375rem;",
                    role: "listbox",
                    aria_label: "Commands",
                    "data-testid": "{list_test_id}",

                    if filtered.is_empty() {
                        div {
                            class: "lp-command-palette-empty py-6 text-center text-sm text-muted-foreground",
                            role: "status",
                            aria_label: "{props.empty_message}",
                            "data-testid": "{empty_test_id}",
                            "{props.empty_message}"
                        }
                    } else {
                        for (idx, item) in filtered.iter().enumerate() {
                            {
                                let is_selected = idx == active_idx;
                                let item_id_attr = format!("{}-item-{}", props.test_id, item.id);
                                let item_clone = item.clone();
                                let item_id_str = item.id.clone();
                                let is_disabled = item.disabled;
                                let mut set_open_click = set_open;

                                let item_cls = join_classes(&[
                                    "lp-command-palette-item flex w-full items-center justify-between gap-3 rounded-lg px-3 py-2 text-left text-sm transition-colors focus:outline-none",
                                    if is_selected {
                                        "bg-accent text-accent-foreground"
                                    } else {
                                        "text-foreground hover:bg-accent/60"
                                    },
                                    if is_disabled { "opacity-50 cursor-not-allowed" } else { "cursor-pointer" },
                                ]);

                                let item_style = if is_selected {
                                    "display: flex; width: 100%; align-items: center; justify-content: space-between; padding: 0.5rem 0.75rem; border-radius: 0.5rem; background: var(--accent, oklch(0.32 0.02 260)); color: var(--accent-foreground, oklch(0.985 0 0));"
                                } else {
                                    "display: flex; width: 100%; align-items: center; justify-content: space-between; padding: 0.5rem 0.75rem; border-radius: 0.5rem; background: transparent;"
                                };

                                rsx! {
                                    button {
                                        key: "{item.id}",
                                        id: "{item_id_attr}",
                                        r#type: "button",
                                        class: "{item_cls}",
                                        style: "{item_style}",
                                        role: "option",
                                        aria_selected: if is_selected { "true" } else { "false" },
                                        aria_disabled: if is_disabled { "true" } else { "false" },
                                        aria_label: "{item.label}",
                                        disabled: is_disabled,
                                        "data-selected": if is_selected { "true" } else { "false" },
                                        "data-value": "{item.id}",
                                        "data-testid": "{item_id_attr}",
                                        onmouseenter: move |_| {
                                            selected_idx.set(idx);
                                        },
                                        onclick: move |_| {
                                            if !is_disabled {
                                                if let Some(cb) = &on_select {
                                                    cb.call(item_id_str.clone());
                                                }
                                                if let Some(cb) = &on_select_item {
                                                    cb.call(item_clone.clone());
                                                }
                                                set_open_click(false);
                                            }
                                        },

                                        div {
                                            class: "flex min-w-0 flex-col gap-0.5",
                                            div {
                                                class: "flex items-center gap-2",
                                                if let Some(cat) = &item.category {
                                                    span {
                                                        class: "lp-command-palette-category rounded bg-muted px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground",
                                                        "{cat}"
                                                    }
                                                }
                                                span {
                                                    class: "lp-command-palette-label truncate font-medium",
                                                    "{item.label}"
                                                }
                                            }
                                            if let Some(desc) = &item.description {
                                                span {
                                                    class: "lp-command-palette-desc truncate text-xs text-muted-foreground",
                                                    "{desc}"
                                                }
                                            }
                                        }

                                        if let Some(shortcut) = &item.shortcut {
                                            kbd {
                                                class: "lp-command-palette-shortcut shrink-0 rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground",
                                                "{shortcut}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_commands() -> Vec<CommandItem> {
        vec![
            CommandItem::new("nav.graph", "Go to Knowledge Graph")
                .with_category("Navigation")
                .with_shortcut("⌘1")
                .with_description("Interactive SVG propositions & implication chains"),
            CommandItem::new("nav.classroom", "Open Classroom Split View")
                .with_category("Navigation")
                .with_shortcut("⌘2")
                .with_keywords(["teach", "probe", "quiz"]),
            CommandItem::new("nav.digest", "Review FSRS Flashcards & CLI Digest")
                .with_category("Practice")
                .with_shortcut("⌘3"),
            CommandItem::new("nav.accounts", "Manage Multi-Account CLI Pool")
                .with_category("Settings")
                .with_shortcut("⌘A"),
        ]
    }

    #[component]
    fn OpenPaletteHarness() -> Element {
        rsx! {
            CommandPalette {
                open: true,
                commands: sample_commands(),
                test_id: "cmd-palette",
            }
        }
    }

    #[component]
    fn FilteredPaletteHarness() -> Element {
        rsx! {
            CommandPalette {
                open: true,
                query: "fsrs".to_string(),
                commands: sample_commands(),
                test_id: "cmd-palette",
            }
        }
    }

    #[component]
    fn EmptyFilteredPaletteHarness() -> Element {
        rsx! {
            CommandPalette {
                open: true,
                query: "zzzz-nonexistent".to_string(),
                commands: sample_commands(),
                test_id: "cmd-palette",
            }
        }
    }

    #[test]
    fn command_palette_renders_open_dialog_combobox_and_items() {
        let mut dom = VirtualDom::new(OpenPaletteHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"role="dialog""#));
        assert!(html.contains(r#"role="combobox""#));
        assert!(html.contains(r#"role="listbox""#));
        assert!(html.contains(r#"role="option""#));
        assert!(html.contains(r#"data-testid="cmd-palette""#));
        assert!(html.contains(r#"data-testid="cmd-palette-input""#));
        assert!(html.contains(r#"data-testid="cmd-palette-item-nav.graph""#));
        assert!(html.contains("Go to Knowledge Graph"));
        assert!(html.contains("⌘1"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn command_palette_fuzzy_filters_by_query() {
        let mut dom = VirtualDom::new(FilteredPaletteHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains("Review FSRS Flashcards"));
        assert!(!html.contains("Go to Knowledge Graph"));
    }

    #[test]
    fn command_palette_shows_empty_status_when_no_matches() {
        let mut dom = VirtualDom::new(EmptyFilteredPaletteHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"role="status""#));
        assert!(html.contains(r#"data-testid="cmd-palette-empty""#));
        assert!(html.contains("No matching commands found."));
    }

    #[test]
    fn cmd_k_and_ctrl_k_shortcut_and_key_navigation_work() {
        assert!(is_command_palette_shortcut(
            &Key::Character("k".to_string()),
            Modifiers::META
        ));
        assert!(is_command_palette_shortcut(
            &Key::Character("K".to_string()),
            Modifiers::CONTROL
        ));
        assert!(!is_command_palette_shortcut(
            &Key::Character("k".to_string()),
            Modifiers::empty()
        ));

        assert_eq!(step_selected_index(0, 4, CommandKeyAction::MoveNext), 1);
        assert_eq!(step_selected_index(0, 4, CommandKeyAction::MovePrev), 3);
        assert_eq!(step_selected_index(2, 4, CommandKeyAction::MoveFirst), 0);
        assert_eq!(step_selected_index(1, 4, CommandKeyAction::MoveLast), 3);
    }

    #[test]
    fn command_palette_clears_query_when_closed_and_reopened() {
        use std::cell::Cell;
        use std::rc::Rc;

        #[derive(Props, Clone)]
        struct ReopenHarnessProps {
            sig_slot: Rc<Cell<Option<Signal<bool>>>>,
        }
        impl PartialEq for ReopenHarnessProps {
            fn eq(&self, _other: &Self) -> bool {
                true
            }
        }

        #[component]
        fn ReopenHarness(props: ReopenHarnessProps) -> Element {
            let open_sig = use_signal(|| true);
            props.sig_slot.set(Some(open_sig));
            rsx! {
                CommandPalette {
                    open_signal: open_sig,
                    default_query: "fsrs".to_string(),
                    commands: sample_commands(),
                    test_id: "cmd-palette",
                }
            }
        }

        let sig_slot = Rc::new(Cell::new(None));
        let mut dom = VirtualDom::new_with_props(
            ReopenHarness,
            ReopenHarnessProps {
                sig_slot: sig_slot.clone(),
            },
        );
        dom.rebuild_in_place();
        let initial_html = dioxus::ssr::render(&dom);
        assert!(initial_html.contains("Review FSRS Flashcards"));
        assert!(!initial_html.contains("Go to Knowledge Graph"));

        let mut open_sig = sig_slot.get().expect("open_signal initialized");

        // Close the palette via signal
        dom.in_runtime(|| open_sig.set(false));
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        let closed_html = dioxus::ssr::render(&dom);
        assert!(!closed_html.contains(r#"data-testid="cmd-palette""#));

        // Reopen the palette: query must be cleared so all commands are visible again
        dom.in_runtime(|| open_sig.set(true));
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        let reopened_html = dioxus::ssr::render(&dom);
        assert!(reopened_html.contains("Review FSRS Flashcards"));
        assert!(reopened_html.contains("Go to Knowledge Graph"));
    }
}
