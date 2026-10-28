use dioxus::prelude::*;

/// Shared properties for all inline 24x24 Lucide SVG icons in `lp-ui`.
#[derive(Props, Clone, PartialEq)]
pub struct IconProps {
    /// Width and height in pixels (defaults to `24`).
    #[props(default = 24)]
    pub size: u32,
    /// Optional CSS class names.
    #[props(into, default)]
    pub class: String,
    /// Optional accessible label. When omitted, `aria-hidden="true"` is set.
    #[props(into, default)]
    pub aria_label: Option<String>,
    /// Optional `data-testid` override.
    #[props(into, default)]
    pub test_id: Option<String>,
    /// Forwarded HTML/SVG global attributes.
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

fn render_svg(props: &IconProps, default_test_id: &str, children: Element) -> Element {
    let tid = props.test_id.as_deref().unwrap_or(default_test_id);
    let aria_hidden = if props.aria_label.is_some() {
        None
    } else {
        Some("true")
    };
    let role = if props.aria_label.is_some() {
        Some("img")
    } else {
        None
    };

    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            width: "{props.size}",
            height: "{props.size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            class: "{props.class}",
            role: role,
            "aria-label": props.aria_label.as_deref(),
            "aria-hidden": aria_hidden,
            "data-testid": "{tid}",
            ..props.attributes.clone(),
            {children}
        }
    }
}

/// Knowledge Graph network icon.
#[component]
pub fn IconGraph(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-graph",
        rsx! {
            circle { cx: "6", cy: "6", r: "3" }
            circle { cx: "18", cy: "6", r: "3" }
            circle { cx: "12", cy: "18", r: "3" }
            path { d: "M8.7 7.5 10.8 15.3" }
            path { d: "M15.3 7.5 13.2 15.3" }
            path { d: "M9 6h6" }
        },
    )
}

/// Classroom graduation cap icon.
#[component]
pub fn IconClassroom(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-classroom",
        rsx! {
            path { d: "M22 10v6M2 10l10-5 10 5-10 5z" }
            path { d: "M6 12v5c3 3 9 3 12 0v-5" }
        },
    )
}

/// FSRS & CLI Transcript Digest stacked layers icon.
#[component]
pub fn IconDigest(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-digest",
        rsx! {
            path { d: "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z" }
            path { d: "m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65" }
            path { d: "m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65" }
        },
    )
}

/// AI Sparkles icon.
#[component]
pub fn IconSparkles(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-sparkles",
        rsx! {
            path { d: "M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z" }
            path { d: "M20 3v4" }
            path { d: "M22 5h-4" }
        },
    )
}

/// Checkmark icon.
#[component]
pub fn IconCheck(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-check",
        rsx! {
            path { d: "M20 6 9 17l-5-5" }
        },
    )
}

/// Close / X icon.
#[component]
pub fn IconX(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-x",
        rsx! {
            path { d: "M18 6 6 18" }
            path { d: "m6 6 12 12" }
        },
    )
}

/// Chevron right icon.
#[component]
pub fn IconChevronRight(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-chevron-right",
        rsx! {
            path { d: "m9 18 6-6-6-6" }
        },
    )
}

/// Chevron down icon.
#[component]
pub fn IconChevronDown(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-chevron-down",
        rsx! {
            path { d: "m6 9 6 6 6-6" }
        },
    )
}

/// Chevron left icon.
#[component]
pub fn IconChevronLeft(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-chevron-left",
        rsx! {
            path { d: "m15 18-6-6 6-6" }
        },
    )
}

/// Chevron up icon.
#[component]
pub fn IconChevronUp(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-chevron-up",
        rsx! {
            path { d: "m18 15-6-6-6 6" }
        },
    )
}

/// Search magnifying glass icon.
#[component]
pub fn IconSearch(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-search",
        rsx! {
            circle { cx: "11", cy: "11", r: "8" }
            path { d: "m21 21-4.3-4.3" }
        },
    )
}

/// Settings gear icon.
#[component]
pub fn IconSettings(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-settings",
        rsx! {
            path { d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" }
            circle { cx: "12", cy: "12", r: "3" }
        },
    )
}

/// Terminal prompt icon.
#[component]
pub fn IconTerminal(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-terminal",
        rsx! {
            polyline { points: "4 17 10 11 4 5" }
            line { x1: "12", x2: "20", y1: "19", y2: "19" }
        },
    )
}

/// Open book icon.
#[component]
pub fn IconBook(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-book",
        rsx! {
            path { d: "M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" }
            path { d: "M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" }
        },
    )
}

/// Brain icon.
#[component]
pub fn IconBrain(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-brain",
        rsx! {
            path { d: "M12 5a3 3 0 1 0-5.997.125 4 4 0 0 0-2.526 5.77 4 4 0 0 0 .556 6.588A4 4 0 1 0 12 18Z" }
            path { d: "M12 5a3 3 0 1 1 5.997.125 4 4 0 0 1 2.526 5.77 4 4 0 0 1-.556 6.588A4 4 0 1 1 12 18Z" }
            path { d: "M15 13a4.5 4.5 0 0 1-3-4 4.5 4.5 0 0 1-3 4" }
        },
    )
}

/// Refresh / sync arrows icon.
#[component]
pub fn IconRefresh(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-refresh",
        rsx! {
            path { d: "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" }
            path { d: "M21 3v5h-5" }
            path { d: "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" }
            path { d: "M8 16H3v5" }
        },
    )
}

/// Play triangle icon.
#[component]
pub fn IconPlay(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-play",
        rsx! {
            polygon { points: "6 3 20 12 6 21 6 3" }
        },
    )
}

/// Shield verification icon.
#[component]
pub fn IconShield(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-shield",
        rsx! {
            path { d: "M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" }
        },
    )
}

/// Plus icon.
#[component]
pub fn IconPlus(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-plus",
        rsx! {
            path { d: "M5 12h14" }
            path { d: "M12 5v14" }
        },
    )
}

/// Trash delete icon.
#[component]
pub fn IconTrash(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-trash",
        rsx! {
            path { d: "M3 6h18" }
            path { d: "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" }
            path { d: "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }
        },
    )
}

/// Copy clipboard icon.
#[component]
pub fn IconCopy(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-copy",
        rsx! {
            rect { width: "14", height: "14", x: "8", y: "8", rx: "2", ry: "2" }
            path { d: "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" }
        },
    )
}

/// Code brackets icon.
#[component]
pub fn IconCode(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-code",
        rsx! {
            polyline { points: "16 18 22 12 16 6" }
            polyline { points: "8 6 2 12 8 18" }
        },
    )
}

/// Command key (`Cmd+K`) icon.
#[component]
pub fn IconCommand(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-command",
        rsx! {
            path { d: "M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3" }
        },
    )
}

/// Users / Multi-Account pool icon.
#[component]
pub fn IconUsers(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-users",
        rsx! {
            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
            circle { cx: "9", cy: "7", r: "4" }
            path { d: "M22 21v-2a4 4 0 0 0-3-3.87" }
            path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
        },
    )
}

/// Zoom In icon.
#[component]
pub fn IconZoomIn(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-zoom-in",
        rsx! {
            circle { cx: "11", cy: "11", r: "8" }
            line { x1: "21", x2: "16.65", y1: "21", y2: "16.65" }
            line { x1: "11", x2: "11", y1: "8", y2: "14" }
            line { x1: "8", x2: "14", y1: "11", y2: "11" }
        },
    )
}

/// Zoom Out icon.
#[component]
pub fn IconZoomOut(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-zoom-out",
        rsx! {
            circle { cx: "11", cy: "11", r: "8" }
            line { x1: "21", x2: "16.65", y1: "21", y2: "16.65" }
            line { x1: "8", x2: "14", y1: "11", y2: "11" }
        },
    )
}

/// Filter funnel icon.
#[component]
pub fn IconFilter(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-filter",
        rsx! {
            polygon { points: "22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" }
        },
    )
}

/// Info circle icon.
#[component]
pub fn IconInfo(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-info",
        rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 16v-4" }
            path { d: "M12 8h.01" }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_twenty_plus_icons_render_valid_svg() {
        let html = dioxus::ssr::render_element(rsx! {
            div {
                IconGraph { size: 20, aria_label: "Knowledge Graph" }
                IconClassroom {}
                IconDigest {}
                IconSparkles {}
                IconCheck {}
                IconX {}
                IconChevronRight {}
                IconChevronDown {}
                IconChevronLeft {}
                IconChevronUp {}
                IconSearch {}
                IconSettings {}
                IconTerminal {}
                IconBook {}
                IconBrain {}
                IconRefresh {}
                IconPlay {}
                IconShield {}
                IconPlus {}
                IconTrash {}
                IconCopy {}
                IconCode {}
                IconCommand {}
                IconUsers {}
                IconZoomIn {}
                IconZoomOut {}
                IconFilter {}
                IconInfo {}
            }
        });

        let expected_ids = [
            "icon-graph",
            "icon-classroom",
            "icon-digest",
            "icon-sparkles",
            "icon-check",
            "icon-x",
            "icon-chevron-right",
            "icon-chevron-down",
            "icon-chevron-left",
            "icon-chevron-up",
            "icon-search",
            "icon-settings",
            "icon-terminal",
            "icon-book",
            "icon-brain",
            "icon-refresh",
            "icon-play",
            "icon-shield",
            "icon-plus",
            "icon-trash",
            "icon-copy",
            "icon-code",
            "icon-command",
            "icon-users",
            "icon-zoom-in",
            "icon-zoom-out",
            "icon-filter",
            "icon-info",
        ];
        assert!(expected_ids.len() >= 20);
        for id in expected_ids {
            assert!(
                html.contains(&format!("data-testid=\"{id}\"")),
                "Missing rendered icon with data-testid={id}"
            );
        }
        assert!(html.contains("stroke=\"currentColor\""));
        assert!(html.contains("aria-label=\"Knowledge Graph\""));
        assert!(html.contains("role=\"img\""));
    }
}
