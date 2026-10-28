//! Interactive overlay, input, scroll, and layout primitives for Dioxus 0.7 (`lp-ui`).
//!
//! Includes:
//! - [`Tooltip`] (`TooltipProvider`, `TooltipTrigger`, `TooltipContent`)
//! - [`Slider`] and [`ConfidenceSlider`] (`1..5` confidence & numeric parameter explorer)
//! - [`Switch`] and [`SwitchThumb`]
//! - [`Checkbox`]
//! - [`ScrollArea`]
//! - [`SplitPane`] (resizable horizontal/vertical desktop divider)
//!
//! All state lives in pure Rust `Signal<T>` with zero inline `<script>` or `wasm-bindgen`.

use dioxus::prelude::*;

fn join_classes(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// ============================================================================
// 1. Tooltip
// ============================================================================

/// Placement side for a [`Tooltip`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TooltipSide {
    /// Above the trigger element.
    #[default]
    Top,
    /// Below the trigger element.
    Bottom,
    /// To the left of the trigger element.
    Left,
    /// To the right of the trigger element.
    Right,
}

impl TooltipSide {
    /// Returns the `data-side` attribute string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    fn position_style(self) -> &'static str {
        match self {
            Self::Top => "bottom: calc(100% + 0.375rem); left: 50%; transform: translateX(-50%);",
            Self::Bottom => "top: calc(100% + 0.375rem); left: 50%; transform: translateX(-50%);",
            Self::Left => "right: calc(100% + 0.375rem); top: 50%; transform: translateY(-50%);",
            Self::Right => "left: calc(100% + 0.375rem); top: 50%; transform: translateY(-50%);",
        }
    }
}

/// Shared context for composable [`Tooltip`] subcomponents.
#[derive(Clone, Copy, PartialEq)]
pub struct TooltipCtx {
    open: Signal<bool>,
    side: TooltipSide,
}

impl TooltipCtx {
    /// Returns whether the tooltip is currently visible.
    pub fn is_open(&self) -> bool {
        (self.open)()
    }

    /// Sets the visibility of the tooltip.
    pub fn set_open(&mut self, value: bool) {
        self.open.set(value);
    }
}

/// Props for [`TooltipProvider`].
#[derive(Props, Clone, PartialEq)]
pub struct TooltipProviderProps {
    /// Child elements inside the tooltip provider scope.
    pub children: Element,
}

/// Optional wrapper matching `shadcn/ui` `TooltipProvider` ergonomics.
#[component]
pub fn TooltipProvider(props: TooltipProviderProps) -> Element {
    rsx! {
        {props.children}
    }
}

/// Props for [`Tooltip`].
#[derive(Props, Clone, PartialEq)]
pub struct TooltipProps {
    /// Shorthand tooltip text content. When provided, renders the bubble automatically.
    #[props(into, optional)]
    pub content: Option<String>,
    /// Alias for `content`.
    #[props(into, optional)]
    pub label: Option<String>,
    /// Optional keyboard shortcut badge displayed inside the tooltip (e.g. `"⌘1"`).
    #[props(into, optional)]
    pub shortcut: Option<String>,
    /// Controlled open state for deterministic testing or programmatic display.
    #[props(optional)]
    pub open: Option<bool>,
    /// Initial open state when uncontrolled.
    #[props(default = false)]
    pub default_open: bool,
    /// Placement side (`Top`, `Bottom`, `Left`, `Right`).
    #[props(default)]
    pub side: TooltipSide,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "tooltip".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes on the wrapper.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Trigger element(s) or composable `TooltipTrigger` + `TooltipContent`.
    pub children: Element,
}

/// Accessible zero-JS tooltip powered by pure Rust `Signal<bool>` hover/focus state
/// and CSS group-hover transitions.
#[component]
pub fn Tooltip(props: TooltipProps) -> Element {
    let mut internal_open = use_signal(|| props.default_open);
    let _ctx = use_context_provider(|| TooltipCtx {
        open: internal_open,
        side: props.side,
    });

    let is_open = props.open.unwrap_or(internal_open());
    let state_str = if is_open { "open" } else { "closed" };
    let side_str = props.side.as_str();
    let tooltip_text = props.content.clone().or_else(|| props.label.clone());
    let bubble_test_id = format!("{}-content", props.test_id);
    let wrapper_cls = join_classes(&[
        "lp-tooltip-wrapper group relative inline-flex items-center",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "{wrapper_cls}",
            style: "position: relative; display: inline-flex; align-items: center;",
            "data-state": "{state_str}",
            "data-side": "{side_str}",
            "data-testid": "{props.test_id}",
            onmouseenter: move |_| internal_open.set(true),
            onmouseleave: move |_| internal_open.set(false),
            onfocusin: move |_| internal_open.set(true),
            onfocusout: move |_| internal_open.set(false),
            onkeydown: move |evt: KeyboardEvent| {
                if evt.key() == Key::Escape {
                    internal_open.set(false);
                }
            },
            ..props.attributes,

            {props.children}

            if let Some(text) = tooltip_text {
                span {
                    class: join_classes(&[
                        "lp-tooltip-bubble pointer-events-none absolute z-50 inline-flex items-center gap-1.5 whitespace-nowrap rounded-md border border-border bg-popover px-2.5 py-1 text-xs font-medium text-popover-foreground shadow-md transition-opacity",
                        if is_open { "opacity-100" } else { "opacity-0 group-hover:opacity-100" },
                    ]),
                    style: format!(
                        "position: absolute; z-index: 50; pointer-events: none; white-space: nowrap; background: var(--popover, oklch(0.24 0.015 260)); color: var(--popover-foreground, oklch(0.985 0 0)); border: 1px solid var(--border, oklch(1 0 0 / 12%)); border-radius: 0.375rem; padding: 0.25rem 0.625rem; font-size: 0.75rem; opacity: {}; {}",
                        if is_open { "1" } else { "0" },
                        props.side.position_style()
                    ),
                    role: "tooltip",
                    aria_label: "{text}",
                    aria_hidden: if is_open { "false" } else { "true" },
                    "data-state": "{state_str}",
                    "data-side": "{side_str}",
                    "data-testid": "{bubble_test_id}",
                    span { "{text}" }
                    if let Some(shortcut) = &props.shortcut {
                        kbd {
                            class: "lp-tooltip-kbd rounded border border-border bg-muted px-1 py-0.5 font-mono text-[10px] text-muted-foreground",
                            "{shortcut}"
                        }
                    }
                }
            }
        }
    }
}

/// Props for [`TooltipTrigger`].
#[derive(Props, Clone, PartialEq)]
pub struct TooltipTriggerProps {
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "tooltip-trigger".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Trigger child element.
    pub children: Element,
}

/// Composable trigger inside a [`Tooltip`].
#[component]
pub fn TooltipTrigger(props: TooltipTriggerProps) -> Element {
    let mut ctx = try_use_context::<TooltipCtx>();
    let cls = join_classes(&[
        "lp-tooltip-trigger inline-flex items-center",
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "{cls}",
            "data-testid": "{props.test_id}",
            onmouseenter: move |_| {
                if let Some(c) = &mut ctx {
                    c.set_open(true);
                }
            },
            onmouseleave: move |_| {
                if let Some(c) = &mut ctx {
                    c.set_open(false);
                }
            },
            ..props.attributes,
            {props.children}
        }
    }
}

/// Props for [`TooltipContent`].
#[derive(Props, Clone, PartialEq)]
pub struct TooltipContentProps {
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "tooltip-content".to_string())]
    pub test_id: String,
    /// Placement side override.
    #[props(optional)]
    pub side: Option<TooltipSide>,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Tooltip body content.
    pub children: Element,
}

/// Composable popover bubble (`role="tooltip"`) inside a [`Tooltip`].
#[component]
pub fn TooltipContent(props: TooltipContentProps) -> Element {
    let ctx = try_use_context::<TooltipCtx>();
    let is_open = ctx.map(|c| c.is_open()).unwrap_or(false);
    let side = props
        .side
        .or_else(|| ctx.map(|c| c.side))
        .unwrap_or(TooltipSide::Top);
    let state_str = if is_open { "open" } else { "closed" };
    let side_str = side.as_str();
    let cls = join_classes(&[
        "lp-tooltip-content pointer-events-none absolute z-50 whitespace-nowrap rounded-md border border-border bg-popover px-2.5 py-1 text-xs font-medium text-popover-foreground shadow-md transition-opacity",
        if is_open { "opacity-100" } else { "opacity-0 group-hover:opacity-100" },
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "{cls}",
            style: format!(
                "position: absolute; z-index: 50; pointer-events: none; white-space: nowrap; background: var(--popover, oklch(0.24 0.015 260)); color: var(--popover-foreground, oklch(0.985 0 0)); border: 1px solid var(--border, oklch(1 0 0 / 12%)); border-radius: 0.375rem; padding: 0.25rem 0.625rem; font-size: 0.75rem; opacity: {}; {}",
                if is_open { "1" } else { "0" },
                side.position_style()
            ),
            role: "tooltip",
            "data-state": "{state_str}",
            "data-side": "{side_str}",
            "data-testid": "{props.test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

// ============================================================================
// 2. Slider & ConfidenceSlider (1..5 Confidence + General Numeric Range)
// ============================================================================

/// Human-readable label for a `1..=5` confidence level in the Classroom Question Dock.
pub const fn confidence_level_label(level: u8) -> &'static str {
    match level {
        1 => "1 · Guess",
        2 => "2 · Low",
        3 => "3 · Medium",
        4 => "4 · High",
        5 => "5 · Certain",
        _ => "3 · Medium",
    }
}

/// Clamps and snaps `raw` to `[min, max]` along increments of `step`.
pub fn snap_slider_value(raw: f64, min: f64, max: f64, step: f64) -> f64 {
    let lo = min.min(max);
    let hi = max.max(min);
    let clamped = raw.clamp(lo, hi);
    if step <= 0.0 {
        return clamped;
    }
    let steps = ((clamped - lo) / step).round();
    (lo + steps * step).clamp(lo, hi)
}

/// Computes the next slider value for a keyboard key press.
pub fn slider_value_from_key(
    current: f64,
    min: f64,
    max: f64,
    step: f64,
    key: &Key,
) -> Option<f64> {
    let step_amt = if step > 0.0 { step } else { 1.0 };
    match key {
        Key::ArrowRight | Key::ArrowUp => {
            Some(snap_slider_value(current + step_amt, min, max, step))
        }
        Key::ArrowLeft | Key::ArrowDown => {
            Some(snap_slider_value(current - step_amt, min, max, step))
        }
        Key::Home => Some(min),
        Key::End => Some(max),
        Key::Character(s) => {
            if let Ok(parsed) = s.parse::<u8>() {
                let numeric = f64::from(parsed);
                if numeric >= min && numeric <= max {
                    return Some(snap_slider_value(numeric, min, max, step));
                }
            }
            None
        }
        _ => None,
    }
}

/// Props for [`Slider`].
#[derive(Props, Clone, PartialEq)]
pub struct SliderProps {
    /// Controlled slider value.
    #[props(optional)]
    pub value: Option<f64>,
    /// Optional reactive signal for two-way binding.
    #[props(optional)]
    pub value_signal: Option<Signal<f64>>,
    /// Default value when uncontrolled (defaults to `3.0`).
    #[props(default = 3.0)]
    pub default_value: f64,
    /// Minimum value (defaults to `1.0` for 1..5 confidence).
    #[props(default = 1.0)]
    pub min: f64,
    /// Maximum value (defaults to `5.0` for 1..5 confidence).
    #[props(default = 5.0)]
    pub max: f64,
    /// Step size (defaults to `1.0`).
    #[props(default = 1.0)]
    pub step: f64,
    /// Callback invoked when the slider value changes.
    #[props(default)]
    pub on_value_change: Option<EventHandler<f64>>,
    /// Alias callback invoked when the slider value changes.
    #[props(default)]
    pub on_change: Option<EventHandler<f64>>,
    /// Accessible label for the slider (`aria-label`).
    #[props(into, default = "Slider".to_string())]
    pub aria_label: String,
    /// Optional visible label rendered above the track.
    #[props(into, optional)]
    pub label: Option<String>,
    /// Whether to display `1..5` confidence pills (`1=Guess .. 5=Certain`).
    #[props(default = false)]
    pub show_confidence_steps: bool,
    /// Whether to render the current numeric/text value badge.
    #[props(default = true)]
    pub show_value: bool,
    /// Whether the slider is disabled.
    #[props(default = false)]
    pub disabled: bool,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "slider".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// Pure-Rust range and confidence slider (`role="slider"`) with keyboard arrow/number-key
/// support, visual progress fill, and optional `1..5` clickable step buttons.
#[component]
pub fn Slider(props: SliderProps) -> Element {
    let min = props.min;
    let max = props.max.max(props.min);
    let step = if props.step > 0.0 { props.step } else { 1.0 };

    let mut internal_val = use_signal(|| snap_slider_value(props.default_value, min, max, step));
    let current_val = if let Some(sig) = props.value_signal {
        snap_slider_value(sig(), min, max, step)
    } else if let Some(v) = props.value {
        snap_slider_value(v, min, max, step)
    } else {
        snap_slider_value(internal_val(), min, max, step)
    };

    let span = (max - min).max(f64::EPSILON);
    let fill_pct = ((current_val - min) / span * 100.0).clamp(0.0, 100.0);

    let is_confidence_scale = props.show_confidence_steps
        || ((min - 1.0).abs() < 1e-6 && (max - 5.0).abs() < 1e-6 && (step - 1.0).abs() < 1e-6);

    let value_text = if is_confidence_scale {
        confidence_level_label(current_val.round() as u8).to_string()
    } else if (step.fract()).abs() < 1e-6 {
        format!("{}", current_val.round() as i64)
    } else {
        format!("{current_val:.2}")
    };

    let apply_value = {
        let external_sig = props.value_signal;
        let controlled = props.value;
        let on_value_change = props.on_value_change;
        let on_change = props.on_change;
        let disabled = props.disabled;
        move |next_raw: f64| {
            if disabled {
                return;
            }
            let snapped = snap_slider_value(next_raw, min, max, step);
            if let Some(mut sig) = external_sig {
                sig.set(snapped);
            } else if controlled.is_none() {
                internal_val.set(snapped);
            }
            if let Some(cb) = &on_value_change {
                cb.call(snapped);
            }
            if let Some(cb) = &on_change {
                cb.call(snapped);
            }
        }
    };

    let mut apply_key = apply_value;
    let mut apply_input = apply_value;

    let cls = join_classes(&[
        "lp-slider flex w-full flex-col gap-2 select-none",
        if props.disabled {
            "opacity-50 pointer-events-none"
        } else {
            ""
        },
        props.class.as_deref().unwrap_or(""),
    ]);

    let input_test_id = format!("{}-input", props.test_id);
    let track_test_id = format!("{}-track", props.test_id);

    rsx! {
        div {
            class: "{cls}",
            role: "slider",
            aria_label: "{props.aria_label}",
            aria_valuemin: "{min}",
            aria_valuemax: "{max}",
            aria_valuenow: "{current_val}",
            aria_valuetext: "{value_text}",
            aria_disabled: if props.disabled { "true" } else { "false" },
            tabindex: if props.disabled { "-1" } else { "0" },
            "data-value": "{current_val}",
            "data-disabled": if props.disabled { "true" } else { "false" },
            "data-testid": "{props.test_id}",
            onkeydown: move |evt: KeyboardEvent| {
                if let Some(next) = slider_value_from_key(current_val, min, max, step, &evt.key()) {
                    evt.prevent_default();
                    apply_key(next);
                }
            },
            ..props.attributes,

            if props.label.is_some() || props.show_value {
                div {
                    class: "lp-slider-header flex items-center justify-between text-xs",
                    if let Some(label_text) = &props.label {
                        span {
                            class: "lp-slider-label font-medium text-muted-foreground",
                            "{label_text}"
                        }
                    }
                    if props.show_value {
                        span {
                            class: "lp-slider-value rounded bg-muted px-1.5 py-0.5 font-mono text-xs font-semibold text-foreground",
                            "data-testid": "{props.test_id}-value",
                            "{value_text}"
                        }
                    }
                }
            }

            div {
                class: "lp-slider-track-container relative flex h-5 w-full items-center",
                "data-testid": "{track_test_id}",
                div {
                    class: "lp-slider-track relative h-2 w-full overflow-hidden rounded-full bg-secondary",
                    style: "position: relative; height: 0.5rem; width: 100%; border-radius: 9999px; background: var(--secondary, oklch(0.269 0.015 260));",
                    div {
                        class: "lp-slider-range h-full bg-primary transition-all",
                        style: "height: 100%; width: {fill_pct:.1}%; background: var(--primary, oklch(0.72 0.16 250));",
                    }
                }
                input {
                    r#type: "range",
                    class: "lp-slider-native-input absolute inset-0 h-full w-full cursor-pointer opacity-0",
                    style: "position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: pointer;",
                    min: "{min}",
                    max: "{max}",
                    step: "{step}",
                    value: "{current_val}",
                    disabled: props.disabled,
                    aria_label: "{props.aria_label}",
                    "data-testid": "{input_test_id}",
                    oninput: move |evt: FormEvent| {
                        if let Ok(v) = evt.value().parse::<f64>() {
                            apply_input(v);
                        }
                    },
                }
            }

            if props.show_confidence_steps {
                div {
                    class: "lp-slider-steps grid grid-cols-5 gap-1 pt-0.5",
                    for level in 1u8..=5u8 {
                        {
                            let is_selected = (current_val.round() as u8) == level;
                            let step_test_id = format!("{}-step-{}", props.test_id, level);
                            let mut apply_step = apply_value;
                            rsx! {
                                button {
                                    key: "{level}",
                                    r#type: "button",
                                    class: join_classes(&[
                                        "lp-slider-step-btn rounded border px-1.5 py-0.5 text-[11px] font-medium transition-colors",
                                        if is_selected {
                                            "border-primary bg-primary text-primary-foreground"
                                        } else {
                                            "border-border bg-card text-muted-foreground hover:text-foreground"
                                        },
                                    ]),
                                    aria_label: confidence_level_label(level),
                                    aria_pressed: if is_selected { "true" } else { "false" },
                                    disabled: props.disabled,
                                    "data-testid": "{step_test_id}",
                                    onclick: move |_| apply_step(f64::from(level)),
                                    "{level}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Props for [`ConfidenceSlider`].
#[derive(Props, Clone, PartialEq)]
pub struct ConfidenceSliderProps {
    /// Controlled `1..=5` confidence rating.
    #[props(optional)]
    pub value: Option<u8>,
    /// Optional reactive signal for `1..=5` confidence rating.
    #[props(optional)]
    pub value_signal: Option<Signal<u8>>,
    /// Default confidence when uncontrolled (`1..=5`, defaults to `3`).
    #[props(default = 3)]
    pub default_value: u8,
    /// Callback invoked when confidence changes.
    #[props(default)]
    pub on_change: Option<EventHandler<u8>>,
    /// Whether the control is disabled.
    #[props(default = false)]
    pub disabled: bool,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "confidence-slider".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
}

/// Dedicated `1..5` Confidence Selector (`1=Guess .. 5=Certain`) for the Classroom Question Dock.
#[component]
pub fn ConfidenceSlider(props: ConfidenceSliderProps) -> Element {
    let controlled = props
        .value
        .map(|v| f64::from(v.clamp(1, 5)))
        .or_else(|| props.value_signal.map(|sig| f64::from(sig().clamp(1, 5))));
    let mut sig_out = props.value_signal;
    let on_change = props.on_change;

    rsx! {
        Slider {
            value: controlled,
            default_value: f64::from(props.default_value.clamp(1, 5)),
            min: 1.0,
            max: 5.0,
            step: 1.0,
            label: "Confidence (1=Guess · 5=Certain)",
            aria_label: "Answer confidence level",
            show_confidence_steps: true,
            show_value: true,
            disabled: props.disabled,
            test_id: "{props.test_id}",
            class: props.class,
            on_value_change: move |v: f64| {
                let lvl = (v.round() as u8).clamp(1, 5);
                if let Some(s) = &mut sig_out {
                    s.set(lvl);
                }
                if let Some(cb) = &on_change {
                    cb.call(lvl);
                }
            },
        }
    }
}

// ============================================================================
// 3. Switch & SwitchThumb
// ============================================================================

/// Props for [`Switch`].
#[derive(Props, Clone, PartialEq)]
pub struct SwitchProps {
    /// Controlled checked state.
    #[props(optional)]
    pub checked: Option<bool>,
    /// Optional reactive signal for two-way binding.
    #[props(optional)]
    pub checked_signal: Option<Signal<bool>>,
    /// Default checked state when uncontrolled.
    #[props(default = false)]
    pub default_checked: bool,
    /// Callback invoked when the switch toggles.
    #[props(default)]
    pub on_checked_change: Option<EventHandler<bool>>,
    /// Alias callback invoked when the switch toggles.
    #[props(default)]
    pub on_change: Option<EventHandler<bool>>,
    /// Whether the switch is disabled.
    #[props(default = false)]
    pub disabled: bool,
    /// Accessible label (`aria-label`).
    #[props(into, default = "Toggle switch".to_string())]
    pub aria_label: String,
    /// Optional visible label rendered next to the switch.
    #[props(into, optional)]
    pub label: Option<String>,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "switch".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Optional custom thumb child (`SwitchThumb`).
    #[props(default)]
    pub children: Element,
}

/// Controlled or uncontrolled toggle switch (`role="switch"`) with keyboard `Space`/`Enter` support.
#[component]
pub fn Switch(props: SwitchProps) -> Element {
    let mut internal_checked = use_signal(|| props.default_checked);
    let is_checked = if let Some(sig) = props.checked_signal {
        sig()
    } else if let Some(c) = props.checked {
        c
    } else {
        internal_checked()
    };

    let state_str = if is_checked { "checked" } else { "unchecked" };
    let mut toggle = {
        let external_sig = props.checked_signal;
        let controlled = props.checked;
        let on_checked_change = props.on_checked_change;
        let on_change = props.on_change;
        let disabled = props.disabled;
        move || {
            if disabled {
                return;
            }
            let next = !is_checked;
            if let Some(mut sig) = external_sig {
                sig.set(next);
            } else if controlled.is_none() {
                internal_checked.set(next);
            }
            if let Some(cb) = &on_checked_change {
                cb.call(next);
            }
            if let Some(cb) = &on_change {
                cb.call(next);
            }
        }
    };
    let mut toggle_key = toggle;

    let btn_cls = join_classes(&[
        "lp-switch peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
        if is_checked { "bg-primary" } else { "bg-input" },
        props.class.as_deref().unwrap_or(""),
    ]);

    let track_style = if is_checked {
        "display: inline-flex; width: 2.25rem; height: 1.25rem; align-items: center; border-radius: 9999px; padding: 0.125rem; background: var(--primary, oklch(0.72 0.16 250)); transition: background 150ms;"
    } else {
        "display: inline-flex; width: 2.25rem; height: 1.25rem; align-items: center; border-radius: 9999px; padding: 0.125rem; background: var(--input, oklch(1 0 0 / 18%)); transition: background 150ms;"
    };

    let thumb_style = if is_checked {
        "display: block; width: 1rem; height: 1rem; border-radius: 9999px; background: var(--primary-foreground, oklch(0.145 0 0)); transform: translateX(1rem); transition: transform 150ms;"
    } else {
        "display: block; width: 1rem; height: 1rem; border-radius: 9999px; background: var(--foreground, oklch(0.985 0 0)); transform: translateX(0); transition: transform 150ms;"
    };

    rsx! {
        div {
            class: "lp-switch-field inline-flex items-center gap-2",
            button {
                r#type: "button",
                class: "{btn_cls}",
                style: "{track_style}",
                role: "switch",
                aria_checked: if is_checked { "true" } else { "false" },
                aria_label: "{props.aria_label}",
                disabled: props.disabled,
                "data-state": "{state_str}",
                "data-disabled": if props.disabled { "true" } else { "false" },
                "data-testid": "{props.test_id}",
                onclick: move |_| toggle(),
                onkeydown: move |evt: KeyboardEvent| {
                    match evt.key() {
                        Key::Enter => {
                            evt.prevent_default();
                            toggle_key();
                        }
                        Key::Character(ref s) if s == " " => {
                            evt.prevent_default();
                            toggle_key();
                        }
                        _ => {}
                    }
                },
                ..props.attributes,
                span {
                    class: join_classes(&[
                        "lp-switch-thumb pointer-events-none block h-4 w-4 rounded-full bg-background shadow-lg ring-0 transition-transform",
                        if is_checked { "translate-x-4" } else { "translate-x-0" },
                    ]),
                    style: "{thumb_style}",
                    "data-state": "{state_str}",
                    {props.children}
                }
            }
            if let Some(label_text) = &props.label {
                span {
                    class: "lp-switch-label text-sm font-medium text-foreground select-none",
                    "{label_text}"
                }
            }
        }
    }
}

/// Optional composable thumb element for [`Switch`].
#[component]
pub fn SwitchThumb() -> Element {
    rsx! {
        span { class: "lp-switch-thumb-inner sr-only" }
    }
}

// ============================================================================
// 4. Checkbox
// ============================================================================

/// Props for [`Checkbox`].
#[derive(Props, Clone, PartialEq)]
pub struct CheckboxProps {
    /// Controlled checked state.
    #[props(optional)]
    pub checked: Option<bool>,
    /// Optional reactive signal for two-way binding.
    #[props(optional)]
    pub checked_signal: Option<Signal<bool>>,
    /// Default checked state when uncontrolled.
    #[props(default = false)]
    pub default_checked: bool,
    /// Callback invoked when the checkbox toggles.
    #[props(default)]
    pub on_checked_change: Option<EventHandler<bool>>,
    /// Alias callback invoked when the checkbox toggles.
    #[props(default)]
    pub on_change: Option<EventHandler<bool>>,
    /// Whether the checkbox is disabled.
    #[props(default = false)]
    pub disabled: bool,
    /// Accessible label (`aria-label`).
    #[props(into, default = "Checkbox".to_string())]
    pub aria_label: String,
    /// Optional visible label rendered beside the checkbox.
    #[props(into, optional)]
    pub label: Option<String>,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "checkbox".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// Pure-Rust controlled/uncontrolled `Checkbox` (`role="checkbox"`) for multi-node Graph selection
/// and Chain Table filtering.
#[component]
pub fn Checkbox(props: CheckboxProps) -> Element {
    let mut internal_checked = use_signal(|| props.default_checked);
    let is_checked = if let Some(sig) = props.checked_signal {
        sig()
    } else if let Some(c) = props.checked {
        c
    } else {
        internal_checked()
    };

    let state_str = if is_checked { "checked" } else { "unchecked" };
    let mut toggle = {
        let external_sig = props.checked_signal;
        let controlled = props.checked;
        let on_checked_change = props.on_checked_change;
        let on_change = props.on_change;
        let disabled = props.disabled;
        move || {
            if disabled {
                return;
            }
            let next = !is_checked;
            if let Some(mut sig) = external_sig {
                sig.set(next);
            } else if controlled.is_none() {
                internal_checked.set(next);
            }
            if let Some(cb) = &on_checked_change {
                cb.call(next);
            }
            if let Some(cb) = &on_change {
                cb.call(next);
            }
        }
    };
    let mut toggle_key = toggle;

    let cls = join_classes(&[
        "lp-checkbox peer flex h-4 w-4 shrink-0 items-center justify-center rounded border border-primary shadow focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50",
        if is_checked {
            "bg-primary text-primary-foreground"
        } else {
            "bg-transparent"
        },
        props.class.as_deref().unwrap_or(""),
    ]);

    rsx! {
        div {
            class: "lp-checkbox-field inline-flex items-center gap-2",
            button {
                r#type: "button",
                class: "{cls}",
                role: "checkbox",
                aria_checked: if is_checked { "true" } else { "false" },
                aria_label: "{props.aria_label}",
                disabled: props.disabled,
                "data-state": "{state_str}",
                "data-disabled": if props.disabled { "true" } else { "false" },
                "data-testid": "{props.test_id}",
                onclick: move |_| toggle(),
                onkeydown: move |evt: KeyboardEvent| {
                    match evt.key() {
                        Key::Enter => {
                            evt.prevent_default();
                            toggle_key();
                        }
                        Key::Character(ref s) if s == " " => {
                            evt.prevent_default();
                            toggle_key();
                        }
                        _ => {}
                    }
                },
                ..props.attributes,
                if is_checked {
                    svg {
                        xmlns: "http://www.w3.org/2000/svg",
                        width: "12",
                        height: "12",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "3",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        "aria-hidden": "true",
                        path { d: "M20 6 9 17l-5-5" }
                    }
                }
            }
            if let Some(label_text) = &props.label {
                span {
                    class: "lp-checkbox-label text-sm text-foreground select-none",
                    "{label_text}"
                }
            }
        }
    }
}

// ============================================================================
// 5. ScrollArea
// ============================================================================

/// Scroll axis configuration for [`ScrollArea`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollOrientation {
    /// Vertical scrolling only (`overflow-y-auto overflow-x-hidden`).
    #[default]
    Vertical,
    /// Horizontal scrolling only (`overflow-x-auto overflow-y-hidden`).
    Horizontal,
    /// Both axes (`overflow-auto`).
    Both,
}

impl ScrollOrientation {
    /// Returns the `data-orientation` value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
            Self::Both => "both",
        }
    }

    const fn overflow_class(self) -> &'static str {
        match self {
            Self::Vertical => "overflow-y-auto overflow-x-hidden",
            Self::Horizontal => "overflow-x-auto overflow-y-hidden",
            Self::Both => "overflow-auto",
        }
    }

    const fn overflow_style(self) -> &'static str {
        match self {
            Self::Vertical => "overflow-y: auto; overflow-x: hidden;",
            Self::Horizontal => "overflow-x: auto; overflow-y: hidden;",
            Self::Both => "overflow: auto;",
        }
    }
}

/// Props for [`ScrollArea`].
#[derive(Props, Clone, PartialEq)]
pub struct ScrollAreaProps {
    /// Scroll axis orientation (`Vertical`, `Horizontal`, `Both`).
    #[props(default)]
    pub orientation: ScrollOrientation,
    /// Optional CSS `max-height` constraint (e.g. `"24rem"` or `"100%"`).
    #[props(into, optional)]
    pub max_height: Option<String>,
    /// Accessible label for the scrollable region.
    #[props(into, default = "Scrollable content".to_string())]
    pub aria_label: String,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "scroll-area".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Scrollable content children.
    pub children: Element,
}

/// Internal pane scroll container that prevents window-level double scrollbars on macOS desktop.
#[component]
pub fn ScrollArea(props: ScrollAreaProps) -> Element {
    let orientation_str = props.orientation.as_str();
    let cls = join_classes(&[
        "lp-scroll-area relative w-full",
        props.orientation.overflow_class(),
        props.class.as_deref().unwrap_or(""),
    ]);

    let style_str = match &props.max_height {
        Some(mh) => format!(
            "position: relative; max-height: {mh}; {}",
            props.orientation.overflow_style()
        ),
        None => format!("position: relative; {}", props.orientation.overflow_style()),
    };

    rsx! {
        div {
            class: "{cls}",
            style: "{style_str}",
            role: "region",
            aria_label: "{props.aria_label}",
            tabindex: "0",
            "data-orientation": "{orientation_str}",
            "data-testid": "{props.test_id}",
            ..props.attributes,
            {props.children}
        }
    }
}

// ============================================================================
// 6. SplitPane
// ============================================================================

/// Split layout direction for [`SplitPane`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitDirection {
    /// Side-by-side left and right panes (`flex-row`).
    #[default]
    Horizontal,
    /// Stacked top and bottom panes (`flex-col`).
    Vertical,
}

impl SplitDirection {
    /// Returns the `data-direction` string (`"horizontal"` or `"vertical"`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }

    /// Returns the ARIA orientation of the separator bar.
    /// A horizontal left/right split has a vertical divider bar (`"vertical"`).
    pub const fn separator_aria_orientation(self) -> &'static str {
        match self {
            Self::Horizontal => "vertical",
            Self::Vertical => "horizontal",
        }
    }
}

/// Computes the updated split ratio (`0.1..=0.9`) from a keyboard event on the separator.
pub fn split_ratio_from_key(
    current: f64,
    min_ratio: f64,
    max_ratio: f64,
    default_ratio: f64,
    step: f64,
    direction: SplitDirection,
    key: &Key,
) -> Option<f64> {
    let lo = min_ratio.clamp(0.05, 0.95);
    let hi = max_ratio.clamp(lo, 0.95);
    let delta = if step > 0.0 { step } else { 0.05 };

    let raw = match (direction, key) {
        (_, Key::Home) => Some(lo),
        (_, Key::End) => Some(hi),
        (_, Key::Enter) => Some(default_ratio.clamp(lo, hi)),
        (SplitDirection::Horizontal, Key::ArrowLeft) | (SplitDirection::Vertical, Key::ArrowUp) => {
            Some((current - delta).clamp(lo, hi))
        }
        (SplitDirection::Horizontal, Key::ArrowRight)
        | (SplitDirection::Vertical, Key::ArrowDown) => Some((current + delta).clamp(lo, hi)),
        _ => None,
    }?;
    Some((raw * 10_000.0).round() / 10_000.0)
}

/// Props for [`SplitPane`].
#[derive(Props, Clone, PartialEq)]
pub struct SplitPaneProps {
    /// Split direction (`Horizontal` for left/right, `Vertical` for top/bottom).
    #[props(default)]
    pub direction: SplitDirection,
    /// Controlled ratio (`0.0..=1.0`, e.g. `0.5` for 50/50).
    #[props(optional)]
    pub ratio: Option<f64>,
    /// Optional reactive signal for the split ratio (`0.0..=1.0`).
    #[props(optional)]
    pub ratio_signal: Option<Signal<f64>>,
    /// Initial split ratio when uncontrolled (defaults to `0.5`).
    #[props(default = 0.5)]
    pub initial_ratio: f64,
    /// Minimum allowed ratio for the primary pane (defaults to `0.15`).
    #[props(default = 0.15)]
    pub min_ratio: f64,
    /// Maximum allowed ratio for the primary pane (defaults to `0.85`).
    #[props(default = 0.85)]
    pub max_ratio: f64,
    /// Keyboard resize step (defaults to `0.05` = 5%).
    #[props(default = 0.05)]
    pub step: f64,
    /// Callback invoked when the split ratio changes.
    #[props(default)]
    pub on_ratio_change: Option<EventHandler<f64>>,
    /// Optional primary (left or top) pane element.
    #[props(optional)]
    pub first: Option<Element>,
    /// Optional secondary (right or bottom) pane element.
    #[props(optional)]
    pub second: Option<Element>,
    /// Alias for `first` (left pane).
    #[props(optional)]
    pub left: Option<Element>,
    /// Alias for `second` (right pane).
    #[props(optional)]
    pub right: Option<Element>,
    /// Accessible label for the resizer handle.
    #[props(into, default = "Resize workspace panes".to_string())]
    pub aria_label: String,
    /// Deterministic `data-testid` attribute.
    #[props(into, default = "split-pane".to_string())]
    pub test_id: String,
    /// Optional extra CSS classes.
    #[props(into, optional)]
    pub class: Option<String>,
    /// Forwarded global HTML attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    /// Optional children when not using `first`/`second` or `left`/`right`.
    #[props(default)]
    pub children: Element,
}

/// Resizable desktop workspace split container with pure-Rust `Signal` ratio state,
/// keyboard accessible separator (`role="separator"`), and pointer drag support.
#[component]
pub fn SplitPane(props: SplitPaneProps) -> Element {
    let min_r = props.min_ratio.clamp(0.05, 0.95);
    let max_r = props.max_ratio.clamp(min_r, 0.95);
    let default_r = props.initial_ratio.clamp(min_r, max_r);

    let mut internal_ratio = use_signal(|| default_r);
    let mut is_dragging = use_signal(|| false);

    let current_ratio = if let Some(sig) = props.ratio_signal {
        sig().clamp(min_r, max_r)
    } else if let Some(r) = props.ratio {
        r.clamp(min_r, max_r)
    } else {
        internal_ratio().clamp(min_r, max_r)
    };

    let first_pct = (current_ratio * 100.0).clamp(5.0, 95.0);
    let second_pct = (100.0 - first_pct).clamp(5.0, 95.0);
    let aria_now = first_pct.round() as i64;
    let aria_min = (min_r * 100.0).round() as i64;
    let aria_max = (max_r * 100.0).round() as i64;

    let mut set_ratio = {
        let external_sig = props.ratio_signal;
        let controlled = props.ratio;
        let on_ratio_change = props.on_ratio_change;
        move |next: f64| {
            let clamped = next.clamp(min_r, max_r);
            if let Some(mut sig) = external_sig {
                sig.set(clamped);
            } else if controlled.is_none() {
                internal_ratio.set(clamped);
            }
            if let Some(cb) = &on_ratio_change {
                cb.call(clamped);
            }
        }
    };

    let dir_str = props.direction.as_str();
    let sep_orientation = props.direction.separator_aria_orientation();
    let resizer_test_id = format!("{}-resizer", props.test_id);
    let first_test_id = format!("{}-first", props.test_id);
    let second_test_id = format!("{}-second", props.test_id);

    let root_cls = join_classes(&[
        "lp-split-pane flex h-full w-full overflow-hidden",
        match props.direction {
            SplitDirection::Horizontal => "flex-row",
            SplitDirection::Vertical => "flex-col",
        },
        props.class.as_deref().unwrap_or(""),
    ]);

    let first_pane = props.first.clone().or_else(|| props.left.clone());
    let second_pane = props.second.clone().or_else(|| props.right.clone());
    let step = props.step;
    let direction = props.direction;

    let first_style = match direction {
        SplitDirection::Horizontal => {
            format!("flex: 0 0 {first_pct:.2}%; width: {first_pct:.2}%; min-width: 0; overflow: hidden;")
        }
        SplitDirection::Vertical => {
            format!("flex: 0 0 {first_pct:.2}%; height: {first_pct:.2}%; min-height: 0; overflow: hidden;")
        }
    };
    let second_style = match direction {
        SplitDirection::Horizontal => {
            format!("flex: 1 1 {second_pct:.2}%; width: {second_pct:.2}%; min-width: 0; overflow: hidden;")
        }
        SplitDirection::Vertical => {
            format!("flex: 1 1 {second_pct:.2}%; height: {second_pct:.2}%; min-height: 0; overflow: hidden;")
        }
    };
    let sep_cls = join_classes(&[
        "lp-split-resizer relative z-10 flex shrink-0 items-center justify-center bg-border transition-colors hover:bg-primary focus-visible:bg-primary focus-visible:outline-none",
        match direction {
            SplitDirection::Horizontal => "w-1.5 cursor-col-resize h-full",
            SplitDirection::Vertical => "h-1.5 cursor-row-resize w-full",
        },
    ]);

    rsx! {
        div {
            class: "{root_cls}",
            style: "display: flex; width: 100%; height: 100%; overflow: hidden;",
            "data-direction": "{dir_str}",
            "data-dragging": if is_dragging() { "true" } else { "false" },
            "data-ratio": "{current_ratio:.2}",
            "data-testid": "{props.test_id}",
            onmouseup: move |_| {
                if is_dragging() {
                    is_dragging.set(false);
                }
            },
            ..props.attributes,

            if first_pane.is_some() || second_pane.is_some() {
                div {
                    class: "lp-split-pane-first h-full min-h-0 min-w-0 overflow-hidden",
                    style: "{first_style}",
                    "data-testid": "{first_test_id}",
                    {first_pane}
                }
                div {
                    class: "{sep_cls}",
                    role: "separator",
                    aria_label: "{props.aria_label}",
                    aria_orientation: "{sep_orientation}",
                    aria_valuemin: "{aria_min}",
                    aria_valuemax: "{aria_max}",
                    aria_valuenow: "{aria_now}",
                    tabindex: "0",
                    "data-testid": "{resizer_test_id}",
                    onmousedown: move |_| is_dragging.set(true),
                    onmouseup: move |_| is_dragging.set(false),
                    onkeydown: move |evt: KeyboardEvent| {
                        if let Some(next) = split_ratio_from_key(
                            current_ratio,
                            min_r,
                            max_r,
                            default_r,
                            step,
                            direction,
                            &evt.key(),
                        ) {
                            evt.prevent_default();
                            set_ratio(next);
                        }
                    },
                    div {
                        class: match direction {
                            SplitDirection::Horizontal => "h-8 w-0.5 rounded-full bg-muted-foreground/50",
                            SplitDirection::Vertical => "h-0.5 w-8 rounded-full bg-muted-foreground/50",
                        },
                    }
                }
                div {
                    class: "lp-split-pane-second h-full min-h-0 min-w-0 overflow-hidden",
                    style: "{second_style}",
                    "data-testid": "{second_test_id}",
                    {second_pane}
                }
            } else {
                {props.children}
                div {
                    class: "{sep_cls}",
                    role: "separator",
                    aria_label: "{props.aria_label}",
                    aria_orientation: "{sep_orientation}",
                    aria_valuemin: "{aria_min}",
                    aria_valuemax: "{aria_max}",
                    aria_valuenow: "{aria_now}",
                    tabindex: "0",
                    "data-testid": "{resizer_test_id}",
                    onmousedown: move |_| is_dragging.set(true),
                    onmouseup: move |_| is_dragging.set(false),
                    onkeydown: move |evt: KeyboardEvent| {
                        if let Some(next) = split_ratio_from_key(
                            current_ratio,
                            min_r,
                            max_r,
                            default_r,
                            step,
                            direction,
                            &evt.key(),
                        ) {
                            evt.prevent_default();
                            set_ratio(next);
                        }
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[component]
    fn TooltipHarness() -> Element {
        rsx! {
            Tooltip {
                content: "Knowledge Graph Home",
                shortcut: "⌘1",
                open: true,
                test_id: "nav-graph-tooltip",
                button { "Graph" }
            }
        }
    }

    #[component]
    fn SliderAndConfidenceHarness() -> Element {
        let confidence = use_signal(|| 4u8);
        rsx! {
            div {
                ConfidenceSlider {
                    value_signal: confidence,
                    test_id: "question-confidence",
                }
            }
        }
    }

    #[component]
    fn SwitchAndCheckboxHarness() -> Element {
        let sw = use_signal(|| true);
        let cb = use_signal(|| true);
        rsx! {
            div {
                Switch {
                    checked_signal: sw,
                    aria_label: "Auto-fold propositions",
                    label: "Auto-fold",
                    test_id: "auto-fold-switch",
                }
                Checkbox {
                    checked_signal: cb,
                    aria_label: "Select proposition n_1",
                    label: "n_1",
                    test_id: "node-checkbox-1",
                }
            }
        }
    }

    #[component]
    fn ScrollAndSplitHarness() -> Element {
        rsx! {
            SplitPane {
                initial_ratio: 0.6,
                test_id: "classroom-split",
                first: rsx! {
                    ScrollArea {
                        aria_label: "Lesson stream",
                        test_id: "lesson-scroll",
                        div { "Lesson Turn 1" }
                    }
                },
                second: rsx! {
                    div { "Plan DAG Pane" }
                },
            }
        }
    }

    #[test]
    fn tooltip_renders_role_shortcut_and_testid() {
        let mut dom = VirtualDom::new(TooltipHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"role="tooltip""#));
        assert!(html.contains(r#"data-testid="nav-graph-tooltip""#));
        assert!(html.contains(r#"data-testid="nav-graph-tooltip-content""#));
        assert!(html.contains("Knowledge Graph Home"));
        assert!(html.contains("⌘1"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn confidence_slider_renders_aria_and_steps() {
        let mut dom = VirtualDom::new(SliderAndConfidenceHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"role="slider""#));
        assert!(html.contains(r#"aria-valuenow="4""#));
        assert!(html.contains("4 · High"));
        assert!(html.contains(r#"data-testid="question-confidence""#));
        assert!(html.contains(r#"data-testid="question-confidence-step-4""#));

        assert_eq!(
            slider_value_from_key(3.0, 1.0, 5.0, 1.0, &Key::ArrowRight),
            Some(4.0)
        );
        assert_eq!(
            slider_value_from_key(3.0, 1.0, 5.0, 1.0, &Key::Character("5".to_string())),
            Some(5.0)
        );
    }

    #[test]
    fn switch_and_checkbox_render_checked_states_and_aria() {
        let mut dom = VirtualDom::new(SwitchAndCheckboxHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"role="switch""#));
        assert!(html.contains(r#"role="checkbox""#));
        assert!(html.contains(r#"data-testid="auto-fold-switch""#));
        assert!(html.contains(r#"data-testid="node-checkbox-1""#));
        assert!(html.contains(r#"data-state="checked""#));
        assert!(html.contains(r#"aria-checked="true""#));
    }

    #[test]
    fn scroll_area_and_split_pane_render_regions_and_separator() {
        let mut dom = VirtualDom::new(ScrollAndSplitHarness);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"data-testid="classroom-split""#));
        assert!(html.contains(r#"data-testid="classroom-split-resizer""#));
        assert!(html.contains(r#"role="separator""#));
        assert!(html.contains(r#"aria-valuenow="60""#));
        assert!(html.contains(r#"data-testid="lesson-scroll""#));
        assert!(html.contains("Lesson Turn 1"));
        assert!(html.contains("Plan DAG Pane"));

        assert_eq!(
            split_ratio_from_key(
                0.60,
                0.15,
                0.85,
                0.50,
                0.05,
                SplitDirection::Horizontal,
                &Key::ArrowLeft
            ),
            Some(0.55)
        );
    }
}
