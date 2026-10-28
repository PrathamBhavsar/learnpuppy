use dioxus::prelude::*;

/// Visual style variant for [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Destructive,
    Link,
}

impl ButtonVariant {
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Primary => "lp-btn-primary bg-primary text-primary-foreground",
            Self::Secondary => "lp-btn-secondary bg-secondary text-secondary-foreground",
            Self::Outline => "lp-btn-outline border border-border bg-background text-foreground",
            Self::Ghost => "lp-btn-ghost text-foreground",
            Self::Destructive => "lp-btn-destructive bg-destructive text-destructive-foreground",
            Self::Link => "lp-btn-link text-primary underline",
        }
    }
}

/// Dimensions and padding preset for [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    Xs,
    Sm,
    #[default]
    Default,
    Lg,
    Icon,
}

impl ButtonSize {
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Xs => "lp-btn-xs",
            Self::Sm => "lp-btn-sm",
            Self::Default => "lp-btn-default",
            Self::Lg => "lp-btn-lg",
            Self::Icon => "lp-btn-icon",
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct ButtonProps {
    #[props(default)]
    pub variant: ButtonVariant,
    #[props(default)]
    pub size: ButtonSize,
    #[props(default)]
    pub disabled: bool,
    #[props(into, default)]
    pub class: String,
    #[props(into, default)]
    pub aria_label: Option<String>,
    #[props(into, default)]
    pub test_id: Option<String>,
    #[props(into, default = "button".to_string())]
    pub button_type: String,
    #[props(default)]
    pub onclick: Option<EventHandler<MouseEvent>>,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    pub children: Element,
}

/// Accessible shadcn-style button primitive with OKLCH dark-mode variants.
#[component]
pub fn Button(props: ButtonProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-button");
    let full_class = if props.class.is_empty() {
        format!(
            "lp-btn {} {}",
            props.variant.class_name(),
            props.size.class_name()
        )
    } else {
        format!(
            "lp-btn {} {} {}",
            props.variant.class_name(),
            props.size.class_name(),
            props.class
        )
    };

    rsx! {
        button {
            r#type: "{props.button_type}",
            class: "{full_class}",
            disabled: props.disabled,
            role: "button",
            "aria-disabled": if props.disabled { "true" } else { "false" },
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            onclick: move |evt| {
                if !props.disabled {
                    if let Some(handler) = &props.onclick {
                        handler.call(evt);
                    }
                }
            },
            ..props.attributes,
            {props.children}
        }
    }
}

/// Visual variant for [`Badge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Success,
    Warning,
    Destructive,
    Outline,
    Phase,
}

impl BadgeVariant {
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Default => "lp-badge-default",
            Self::Secondary => "lp-badge-secondary",
            Self::Success => "lp-badge-success",
            Self::Warning => "lp-badge-warning",
            Self::Destructive => "lp-badge-destructive",
            Self::Outline => "lp-badge-outline",
            Self::Phase => "lp-badge-phase",
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct BadgeProps {
    #[props(default)]
    pub variant: BadgeVariant,
    #[props(into, default)]
    pub class: String,
    #[props(into, default)]
    pub aria_label: Option<String>,
    #[props(into, default)]
    pub test_id: Option<String>,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    pub children: Element,
}

/// Compact status, phase, or category pill badge.
#[component]
pub fn Badge(props: BadgeProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-badge");
    let full_class = if props.class.is_empty() {
        format!("lp-badge {}", props.variant.class_name())
    } else {
        format!("lp-badge {} {}", props.variant.class_name(), props.class)
    };

    rsx! {
        span {
            role: "status",
            class: "{full_class}",
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            ..props.attributes,
            {props.children}
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct CardSectionProps {
    #[props(into, default)]
    pub class: String,
    #[props(into, default)]
    pub aria_label: Option<String>,
    #[props(into, default)]
    pub test_id: Option<String>,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
    pub children: Element,
}

fn join_class(base: &str, extra: &str) -> String {
    if extra.is_empty() {
        base.to_string()
    } else {
        format!("{base} {extra}")
    }
}

/// Elevated surface container styled with `--card` and `--border` tokens.
#[component]
pub fn Card(props: CardSectionProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-card");
    let class = join_class("lp-card bg-card text-card-foreground", &props.class);

    rsx! {
        div {
            role: "region",
            class: "{class}",
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Header block inside a [`Card`].
#[component]
pub fn CardHeader(props: CardSectionProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-card-header");
    let class = join_class("lp-card-header", &props.class);

    rsx! {
        div {
            class: "{class}",
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Primary heading inside a [`CardHeader`].
#[component]
pub fn CardTitle(props: CardSectionProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-card-title");
    let class = join_class("lp-card-title", &props.class);

    rsx! {
        h3 {
            class: "{class}",
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Muted subtitle inside a [`CardHeader`].
#[component]
pub fn CardDescription(props: CardSectionProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-card-description");
    let class = join_class("lp-card-description text-muted-foreground", &props.class);

    rsx! {
        p {
            class: "{class}",
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Main body container inside a [`Card`].
#[component]
pub fn CardContent(props: CardSectionProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-card-content");
    let class = join_class("lp-card-content", &props.class);

    rsx! {
        div {
            class: "{class}",
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Action bar footer inside a [`Card`].
#[component]
pub fn CardFooter(props: CardSectionProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-card-footer");
    let class = join_class("lp-card-footer", &props.class);

    rsx! {
        div {
            class: "{class}",
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            ..props.attributes,
            {props.children}
        }
    }
}

/// Axis orientation for [`Separator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeparatorOrientation {
    #[default]
    Horizontal,
    Vertical,
}

impl SeparatorOrientation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }

    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Horizontal => "lp-separator-horizontal",
            Self::Vertical => "lp-separator-vertical",
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct SeparatorProps {
    #[props(default)]
    pub orientation: SeparatorOrientation,
    #[props(default = true)]
    pub decorative: bool,
    #[props(into, default)]
    pub class: String,
    #[props(into, default)]
    pub aria_label: Option<String>,
    #[props(into, default)]
    pub test_id: Option<String>,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// Visual or semantic divider between UI sections.
#[component]
pub fn Separator(props: SeparatorProps) -> Element {
    let tid = props.test_id.as_deref().unwrap_or("lp-separator");
    let role = if props.decorative {
        "none"
    } else {
        "separator"
    };
    let base = format!("lp-separator {}", props.orientation.class_name());
    let class = join_class(&base, &props.class);

    rsx! {
        div {
            role: "{role}",
            "aria-orientation": "{props.orientation.as_str()}",
            "aria-label": props.aria_label.as_deref(),
            "data-testid": "{tid}",
            class: "{class}",
            ..props.attributes,
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct ProgressProps {
    /// Current progress value (`0.0..=max`).
    pub value: f64,
    /// Maximum value (defaults to `100.0`).
    #[props(default = 100.0)]
    pub max: f64,
    #[props(into, default)]
    pub class: String,
    #[props(into, default)]
    pub aria_label: Option<String>,
    #[props(into, default)]
    pub test_id: Option<String>,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// Accessible linear progress bar indicator.
#[component]
pub fn Progress(props: ProgressProps) -> Element {
    let safe_max = if props.max.is_finite() && props.max > 0.0 {
        props.max
    } else {
        100.0
    };
    let clamped_value = if props.value.is_finite() {
        props.value.clamp(0.0, safe_max)
    } else {
        0.0
    };
    let pct = ((clamped_value / safe_max) * 100.0).clamp(0.0, 100.0);
    let tid = props.test_id.as_deref().unwrap_or("lp-progress");
    let label = props.aria_label.as_deref().unwrap_or("Progress");
    let class = join_class("lp-progress", &props.class);

    rsx! {
        div {
            role: "progressbar",
            "aria-label": "{label}",
            "aria-valuemin": "0",
            "aria-valuemax": "{safe_max:.0}",
            "aria-valuenow": "{clamped_value:.0}",
            "data-testid": "{tid}",
            class: "{class}",
            ..props.attributes,
            div {
                class: "lp-progress-indicator",
                "data-testid": "{tid}-indicator",
                style: "width: {pct:.1}%;",
            }
        }
    }
}

/// Mastery tier classification for FSRS and Knowledge Graph nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasteryTier {
    Novice,
    Learning,
    Mastered,
}

impl MasteryTier {
    /// Classifies a normalized mastery ratio in `0.0..=1.0`.
    pub fn from_ratio(ratio: f64) -> Self {
        if ratio >= 0.8 {
            Self::Mastered
        } else if ratio >= 0.4 {
            Self::Learning
        } else {
            Self::Novice
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Novice => "novice",
            Self::Learning => "learning",
            Self::Mastered => "mastered",
        }
    }

    pub const fn stroke_var(self) -> &'static str {
        match self {
            Self::Novice => "var(--mastery-novice)",
            Self::Learning => "var(--mastery-learning)",
            Self::Mastered => "var(--mastery-mastered)",
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct MasteryRingProps {
    /// Mastery score either as a ratio (`0.0..=1.0`) or percentage (`0.0..=100.0`).
    pub score: f64,
    /// Outer SVG width/height in pixels (defaults to `36`).
    #[props(default = 36)]
    pub size: u32,
    /// Ring stroke thickness in pixels (defaults to `4.0`).
    #[props(default = 4.0)]
    pub stroke_width: f64,
    /// Whether to display the rounded percentage label inside the ring.
    #[props(default = true)]
    pub show_label: bool,
    #[props(into, default)]
    pub class: String,
    #[props(into, default)]
    pub aria_label: Option<String>,
    #[props(into, default)]
    pub test_id: Option<String>,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// Circular SVG mastery indicator using OKLCH `--mastery-*` semantic tokens.
#[component]
pub fn MasteryRing(props: MasteryRingProps) -> Element {
    let raw = if props.score.is_finite() {
        props.score
    } else {
        0.0
    };
    let ratio = if raw > 1.0 {
        (raw / 100.0).clamp(0.0, 1.0)
    } else {
        raw.clamp(0.0, 1.0)
    };
    let pct = (ratio * 100.0).round() as u32;
    let tier = MasteryTier::from_ratio(ratio);

    let size_f = f64::from(props.size.max(16));
    let sw = props.stroke_width.clamp(1.5, size_f / 3.0);
    let center = size_f / 2.0;
    let radius = (center - sw).max(1.0);
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let dash_offset = circumference * (1.0 - ratio);

    let tid = props.test_id.as_deref().unwrap_or("lp-mastery-ring");
    let label = props
        .aria_label
        .clone()
        .unwrap_or_else(|| format!("Mastery {pct}% ({})", tier.as_str()));
    let class = join_class(
        "lp-mastery-ring inline-flex items-center justify-center",
        &props.class,
    );

    rsx! {
        div {
            role: "meter",
            "aria-label": "{label}",
            "aria-valuemin": "0",
            "aria-valuemax": "100",
            "aria-valuenow": "{pct}",
            "data-testid": "{tid}",
            "data-mastery-tier": "{tier.as_str()}",
            class: "{class}",
            style: "position: relative; width: {props.size}px; height: {props.size}px;",
            ..props.attributes,
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "{props.size}",
                height: "{props.size}",
                view_box: "0 0 {size_f} {size_f}",
                "aria-hidden": "true",
                circle {
                    cx: "{center}",
                    cy: "{center}",
                    r: "{radius}",
                    fill: "none",
                    stroke: "var(--muted)",
                    stroke_width: "{sw}",
                }
                circle {
                    cx: "{center}",
                    cy: "{center}",
                    r: "{radius}",
                    fill: "none",
                    stroke: "{tier.stroke_var()}",
                    stroke_width: "{sw}",
                    stroke_linecap: "round",
                    stroke_dasharray: "{circumference:.2}",
                    stroke_dashoffset: "{dash_offset:.2}",
                    transform: "rotate(-90 {center} {center})",
                }
            }
            if props.show_label {
                span {
                    "data-testid": "{tid}-label",
                    style: "position: absolute; font-size: 0.65rem; font-weight: 600;",
                    "{pct}%"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_renders_variants_sizes_and_aria() {
        let html = dioxus::ssr::render_element(rsx! {
            div {
                Button {
                    variant: ButtonVariant::Primary,
                    size: ButtonSize::Default,
                    aria_label: "Launch Practice",
                    test_id: "btn-launch",
                    "Launch"
                }
                Button {
                    variant: ButtonVariant::Destructive,
                    size: ButtonSize::Sm,
                    disabled: true,
                    test_id: "btn-delete",
                    "Delete"
                }
            }
        });
        assert!(html.contains("data-testid=\"btn-launch\""));
        assert!(html.contains("aria-label=\"Launch Practice\""));
        assert!(html.contains("lp-btn-primary"));
        assert!(html.contains("data-testid=\"btn-delete\""));
        assert!(html.contains("aria-disabled=\"true\""));
        assert!(html.contains("lp-btn-destructive"));
    }

    #[test]
    fn badge_renders_phase_and_status_variants() {
        let html = dioxus::ssr::render_element(rsx! {
            div {
                Badge { variant: BadgeVariant::Phase, test_id: "badge-phase", "TEACH" }
                Badge { variant: BadgeVariant::Success, test_id: "badge-ok", "Mastered" }
            }
        });
        assert!(html.contains("data-testid=\"badge-phase\""));
        assert!(html.contains("role=\"status\""));
        assert!(html.contains("lp-badge-phase"));
        assert!(html.contains("TEACH"));
        assert!(html.contains("lp-badge-success"));
    }

    #[test]
    fn card_composite_sections_render() {
        let html = dioxus::ssr::render_element(rsx! {
            Card {
                aria_label: "Session Summary",
                test_id: "summary-card",
                CardHeader {
                    CardTitle { "ForceAtlas2 Layout" }
                    CardDescription { "Deterministic spring-electric graph layout" }
                }
                CardContent { "Propositions: 42" }
                CardFooter { "Updated just now" }
            }
        });
        assert!(html.contains("data-testid=\"summary-card\""));
        assert!(html.contains("role=\"region\""));
        assert!(html.contains("ForceAtlas2 Layout"));
        assert!(html.contains("Deterministic spring-electric graph layout"));
        assert!(html.contains("Propositions: 42"));
        assert!(html.contains("Updated just now"));
    }

    #[test]
    fn separator_renders_orientation_and_role() {
        let html = dioxus::ssr::render_element(rsx! {
            div {
                Separator { orientation: SeparatorOrientation::Horizontal, decorative: false, test_id: "sep-h" }
                Separator { orientation: SeparatorOrientation::Vertical, decorative: true, test_id: "sep-v" }
            }
        });
        assert!(html.contains("data-testid=\"sep-h\""));
        assert!(html.contains("role=\"separator\""));
        assert!(html.contains("aria-orientation=\"horizontal\""));
        assert!(html.contains("data-testid=\"sep-v\""));
        assert!(html.contains("aria-orientation=\"vertical\""));
    }

    #[test]
    fn progress_clamps_and_renders_progressbar_aria() {
        let html = dioxus::ssr::render_element(rsx! {
            Progress { value: 65.0, max: 100.0, aria_label: "Lesson Progress", test_id: "lesson-prog" }
        });
        assert!(html.contains("data-testid=\"lesson-prog\""));
        assert!(html.contains("role=\"progressbar\""));
        assert!(html.contains("aria-valuenow=\"65\""));
        assert!(html.contains("width: 65.0%;"));
    }

    #[test]
    fn mastery_ring_classifies_novice_learning_and_mastered() {
        let html = dioxus::ssr::render_element(rsx! {
            div {
                MasteryRing { score: 0.25, test_id: "ring-novice" }
                MasteryRing { score: 0.60, test_id: "ring-learning" }
                MasteryRing { score: 0.92, test_id: "ring-mastered" }
            }
        });
        assert!(html.contains("data-testid=\"ring-novice\""));
        assert!(html.contains("data-mastery-tier=\"novice\""));
        assert!(html.contains("var(--mastery-novice)"));
        assert!(html.contains("25%"));

        assert!(html.contains("data-testid=\"ring-learning\""));
        assert!(html.contains("data-mastery-tier=\"learning\""));
        assert!(html.contains("var(--mastery-learning)"));
        assert!(html.contains("60%"));

        assert!(html.contains("data-testid=\"ring-mastered\""));
        assert!(html.contains("data-mastery-tier=\"mastered\""));
        assert!(html.contains("var(--mastery-mastered)"));
        assert!(html.contains("92%"));
    }
}
