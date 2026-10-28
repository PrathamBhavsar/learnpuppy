use dioxus::prelude::*;

/// Shared properties for all inline 24x24 Hugeicons SVG icons in `lp-ui`.
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
            stroke_width: "1.5",
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

/// Hugeicons `NeuralNetworkIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconGraph(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-graph",
        rsx! {
            path { d: "M21.5 12C21.5 13.3807 20.3807 14.5 19 14.5C17.6193 14.5 16.5 13.3807 16.5 12C16.5 10.6193 17.6193 9.5 19 9.5C20.3807 9.5 21.5 10.6193 21.5 12Z", stroke: "currentColor", stroke_width: "1.5" }
            path { d: "M13.5 4C13.5 4.82843 12.8284 5.5 12 5.5C11.1716 5.5 10.5 4.82843 10.5 4C10.5 3.17157 11.1716 2.5 12 2.5C12.8284 2.5 13.5 3.17157 13.5 4Z", stroke: "currentColor", stroke_width: "1.5" }
            path { d: "M12.5 11.5C12.5 12.3284 11.8284 13 11 13C10.1716 13 9.5 12.3284 9.5 11.5C9.5 10.6716 10.1716 10 11 10C11.8284 10 12.5 10.6716 12.5 11.5Z", stroke: "currentColor", stroke_width: "1.5" }
            path { d: "M6.5 7.5C6.5 8.60457 5.60457 9.5 4.5 9.5C3.39543 9.5 2.5 8.60457 2.5 7.5C2.5 6.39543 3.39543 5.5 4.5 5.5C5.60457 5.5 6.5 6.39543 6.5 7.5Z", stroke: "currentColor", stroke_width: "1.5" }
            path { d: "M10.5 19.5C10.5 20.6046 9.60457 21.5 8.5 21.5C7.39543 21.5 6.5 20.6046 6.5 19.5C6.5 18.3954 7.39543 17.5 8.5 17.5C9.60457 17.5 10.5 18.3954 10.5 19.5Z", stroke: "currentColor", stroke_width: "1.5" }
            path { d: "M13.5 5L17.5 10M14.5 15.5L10.5 18.5M8 17.5L5 9.5M6.31298 6.65431L10.5 4.5M12.5 11.5L16.505 11.8443", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M12 5.5L11 10", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Mortarboard01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconClassroom(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-classroom",
        rsx! {
            path { d: "M1.99805 7.99928C1.99805 9.34126 10.0943 13 11.9857 13C13.8772 13 21.9734 9.34126 21.9734 7.99928C21.9734 6.6573 13.8772 2.99854 11.9857 2.99854C10.0943 2.99854 1.99805 6.6573 1.99805 7.99928Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M5.99219 11L7.24348 16.8002C7.32919 17.1975 7.52703 17.5687 7.85696 17.8054C10.0787 19.3998 13.8908 19.3998 16.1126 17.8054C16.4426 17.5687 16.6404 17.1975 16.7261 16.8002L17.9774 11", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M20.4774 9.49951V16.5006M20.4774 16.5006C19.6864 17.9471 19.3366 18.7221 18.9813 20.0011C18.9042 20.4562 18.9654 20.6855 19.2786 20.8891C19.4059 20.9718 19.5588 21.0012 19.7104 21.0012H21.229C21.3904 21.0012 21.5533 20.9675 21.6863 20.8757C21.9774 20.6747 22.0523 20.4541 21.9734 20.0011C21.662 18.8135 21.2653 18.0016 20.4774 16.5006Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Layers01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconDigest(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-digest",
        rsx! {
            path { d: "M8.64298 3.14559L6.93816 3.93362C4.31272 5.14719 3 5.75397 3 6.75C3 7.74603 4.31272 8.35281 6.93817 9.56638L8.64298 10.3544C10.2952 11.1181 11.1214 11.5 12 11.5C12.8786 11.5 13.7048 11.1181 15.357 10.3544L17.0618 9.56638C19.6873 8.35281 21 7.74603 21 6.75C21 5.75397 19.6873 5.14719 17.0618 3.93362L15.357 3.14559C13.7048 2.38186 12.8786 2 12 2C11.1214 2 10.2952 2.38186 8.64298 3.14559Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M20.788 11.0972C20.9293 11.2959 21 11.5031 21 11.7309C21 12.7127 19.6873 13.3109 17.0618 14.5072L15.357 15.284C13.7048 16.0368 12.8786 16.4133 12 16.4133C11.1214 16.4133 10.2952 16.0368 8.64298 15.284L6.93817 14.5072C4.31272 13.3109 3 12.7127 3 11.7309C3 11.5031 3.07067 11.2959 3.212 11.0972", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M20.3767 16.2661C20.7922 16.5971 21 16.927 21 17.3176C21 18.2995 19.6873 18.8976 17.0618 20.0939L15.357 20.8707C13.7048 21.6236 12.8786 22 12 22C11.1214 22 10.2952 21.6236 8.64298 20.8707L6.93817 20.0939C4.31272 18.8976 3 18.2995 3 17.3176C3 16.927 3.20778 16.5971 3.62334 16.2661", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `SparklesIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconSparkles(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-sparkles",
        rsx! {
            path { d: "M15 2L15.5387 4.39157C15.9957 6.42015 17.5798 8.00431 19.6084 8.46127L22 9L19.6084 9.53873C17.5798 9.99569 15.9957 11.5798 15.5387 13.6084L15 16L14.4613 13.6084C14.0043 11.5798 12.4202 9.99569 10.3916 9.53873L8 9L10.3916 8.46127C12.4201 8.00431 14.0043 6.42015 14.4613 4.39158L15 2Z", stroke: "currentColor", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M7 12L7.38481 13.7083C7.71121 15.1572 8.84275 16.2888 10.2917 16.6152L12 17L10.2917 17.3848C8.84275 17.7112 7.71121 18.8427 7.38481 20.2917L7 22L6.61519 20.2917C6.28879 18.8427 5.15725 17.7112 3.70827 17.3848L2 17L3.70827 16.6152C5.15725 16.2888 6.28879 15.1573 6.61519 13.7083L7 12Z", stroke: "currentColor", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Tick02Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconCheck(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-check",
        rsx! {
            path { d: "M5 14L8.5 17.5L19 6.5", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Cancel01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconX(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-x",
        rsx! {
            path { d: "M18 6L6.00081 17.9992M17.9992 18L6 6.00085", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `ArrowRight01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconChevronRight(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-chevron-right",
        rsx! {
            path { d: "M9.00005 6C9.00005 6 15 10.4189 15 12C15 13.5812 9 18 9 18", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `ArrowDown01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconChevronDown(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-chevron-down",
        rsx! {
            path { d: "M18 9.00005C18 9.00005 13.5811 15 12 15C10.4188 15 6 9 6 9", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `ArrowLeft01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconChevronLeft(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-chevron-left",
        rsx! {
            path { d: "M15 6C15 6 9.00001 10.4189 9 12C8.99999 13.5812 15 18 15 18", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `ArrowUp01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconChevronUp(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-chevron-up",
        rsx! {
            path { d: "M17.9998 15C17.9998 15 13.5809 9.00001 11.9998 9C10.4187 8.99999 5.99985 15 5.99985 15", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Search01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconSearch(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-search",
        rsx! {
            path { d: "M17 17L21 21", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M19 11C19 6.58172 15.4183 3 11 3C6.58172 3 3 6.58172 3 11C3 15.4183 6.58172 19 11 19C15.4183 19 19 15.4183 19 11Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Settings01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconSettings(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-settings",
        rsx! {
            path { d: "M21.3175 7.14139L20.8239 6.28479C20.4506 5.63696 20.264 5.31305 19.9464 5.18388C19.6288 5.05472 19.2696 5.15664 18.5513 5.36048L17.3311 5.70418C16.8725 5.80994 16.3913 5.74994 15.9726 5.53479L15.6357 5.34042C15.2766 5.11043 15.0004 4.77133 14.8475 4.37274L14.5136 3.37536C14.294 2.71534 14.1842 2.38533 13.9228 2.19657C13.6615 2.00781 13.3143 2.00781 12.6199 2.00781H11.5051C10.8108 2.00781 10.4636 2.00781 10.2022 2.19657C9.94085 2.38533 9.83106 2.71534 9.61149 3.37536L9.27753 4.37274C9.12465 4.77133 8.84845 5.11043 8.48937 5.34042L8.15249 5.53479C7.73374 5.74994 7.25259 5.80994 6.79398 5.70418L5.57375 5.36048C4.85541 5.15664 4.49625 5.05472 4.17867 5.18388C3.86109 5.31305 3.67445 5.63696 3.30115 6.28479L2.80757 7.14139C2.45766 7.74864 2.2827 8.05227 2.31666 8.37549C2.35061 8.69871 2.58483 8.95918 3.05326 9.48012L4.0843 10.6328C4.3363 10.9518 4.51521 11.5078 4.51521 12.0077C4.51521 12.5078 4.33636 13.0636 4.08433 13.3827L3.05326 14.5354C2.58483 15.0564 2.35062 15.3168 2.31666 15.6401C2.2827 15.9633 2.45766 16.2669 2.80757 16.8741L3.30114 17.7307C3.67443 18.3785 3.86109 18.7025 4.17867 18.8316C4.49625 18.9608 4.85542 18.8589 5.57377 18.655L6.79394 18.3113C7.25263 18.2055 7.73387 18.2656 8.15267 18.4808L8.4895 18.6752C8.84851 18.9052 9.12464 19.2442 9.2775 19.6428L9.61149 20.6403C9.83106 21.3003 9.94085 21.6303 10.2022 21.8191C10.4636 22.0078 10.8108 22.0078 11.5051 22.0078H12.6199C13.3143 22.0078 13.6615 22.0078 13.9228 21.8191C14.1842 21.6303 14.294 21.3003 14.5136 20.6403L14.8476 19.6428C15.0004 19.2442 15.2765 18.9052 15.6356 18.6752L15.9724 18.4808C16.3912 18.2656 16.8724 18.2055 17.3311 18.3113L18.5513 18.655C19.2696 18.8589 19.6288 18.9608 19.9464 18.8316C20.264 18.7025 20.4506 18.3785 20.8239 17.7307L21.3175 16.8741C21.6674 16.2669 21.8423 15.9633 21.8084 15.6401C21.7744 15.3168 21.5402 15.0564 21.0718 14.5354L20.0407 13.3827C19.7887 13.0636 19.6098 12.5078 19.6098 12.0077C19.6098 11.5078 19.7888 10.9518 20.0407 10.6328L21.0718 9.48012C21.5402 8.95918 21.7744 8.69871 21.8084 8.37549C21.8423 8.05227 21.6674 7.74864 21.3175 7.14139Z", stroke: "currentColor", stroke_linecap: "round", stroke_width: "1.5" }
            path { d: "M15.5195 12C15.5195 13.933 13.9525 15.5 12.0195 15.5C10.0865 15.5 8.51953 13.933 8.51953 12C8.51953 10.067 10.0865 8.5 12.0195 8.5C13.9525 8.5 15.5195 10.067 15.5195 12Z", stroke: "currentColor", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `CommandLineIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconTerminal(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-terminal",
        rsx! {
            path { d: "M14 4H10C6.22876 4 4.34315 4 3.17157 5.17157C2 6.34315 2 8.22876 2 12C2 15.7712 2 17.6569 3.17157 18.8284C4.34315 20 6.22876 20 10 20H14C17.7712 20 19.6569 20 20.8284 18.8284C22 17.6569 22 15.7712 22 12C22 8.22876 22 6.34315 20.8284 5.17157C19.6569 4 17.7712 4 14 4Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M7 9L8.83981 10.5858C9.61327 11.2525 10 11.5858 10 12C10 12.4142 9.61327 12.7475 8.83981 13.4142L7 15", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M13 16H17", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `BookOpen01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconBook(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-book",
        rsx! {
            path { d: "M7.99978 3.5H6.60021C4.43183 3.5 3.34764 3.5 2.67399 4.17362C2.00034 4.84724 2.00029 5.93144 2.00021 8.09982L2 13.3998C1.99992 15.5684 1.99987 16.6526 2.67353 17.3263C3.34719 18 4.43146 18 6.6 18H8.95042C10.4329 18 11.7092 19.0464 11.9999 20.5V5.5C11.0556 4.24097 9.99989 3.5 7.99978 3.5Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M16.0001 3.5H17.3997C19.5681 3.5 20.6523 3.5 21.3259 4.17362C21.9996 4.84724 21.9996 5.93144 21.9997 8.09982L21.9999 13.3998C22 15.5684 22 16.6526 21.3264 17.3263C20.6527 18 19.5684 18 17.3999 18H15.0495C13.567 18 12.2907 19.0464 12 20.5V5.5C12.9443 4.24097 14 3.5 16.0001 3.5Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `BrainIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconBrain(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-brain",
        rsx! {
            path { d: "M16.998 7.12652C17.3182 7.04393 17.654 7 18 7C20.2091 7 22 8.79086 22 11C22 13.2091 20.2091 15 18 15C17.6451 15 17.3009 14.9538 16.9733 14.867M16.998 7.12652C16.9993 7.08451 17 7.04233 17 7C17 4.79086 15.2091 3 13 3C11.0824 3 9.47994 4.34939 9.09041 6.15043M16.998 7.12652C16.9769 7.80763 16.7854 8.44584 16.4649 9M16.9733 14.867C16.9909 14.7472 17 14.6247 17 14.5C17 13.2905 16.1411 12.2816 15 12.05M16.9733 14.867C16.7957 16.0737 15.756 17 14.5 17H14C11.7909 17 10 18.7909 10 21M9.09041 6.15043C8.74377 6.05243 8.37801 6 8 6C5.79086 6 4 7.79086 4 10C4 10.3886 4.05542 10.7643 4.15878 11.1195M9.09041 6.15043C10.1015 6.43625 10.9498 7.10965 11.4649 8M4.15878 11.1195C2.9114 11.4832 2 12.6352 2 14C2 15.6569 3.34315 17 5 17C6.30622 17 7.41746 16.1652 7.82929 15M4.15878 11.1195C4.24921 11.4303 4.37632 11.7255 4.53513 12", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M11.8361 11.7435C11.3257 12.2353 10.453 12.3202 9.70713 11.9008C8.9612 11.4814 8.58031 10.6917 8.73535 10", stroke: "currentColor", stroke_linecap: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `RefreshIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconRefresh(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-refresh",
        rsx! {
            path { d: "M20.0092 2V5.13219C20.0092 5.42605 19.6418 5.55908 19.4537 5.33333C17.6226 3.2875 14.9617 2 12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22C17.5228 22 22 17.5228 22 12", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `PlayIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconPlay(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-play",
        rsx! {
            path { d: "M18.8906 12.846C18.5371 14.189 16.8667 15.138 13.5257 17.0361C10.296 18.8709 8.6812 19.7884 7.37983 19.4196C6.8418 19.2671 6.35159 18.9776 5.95624 18.5787C5 17.6139 5 15.7426 5 12C5 8.2574 5 6.3861 5.95624 5.42132C6.35159 5.02245 6.8418 4.73288 7.37983 4.58042C8.6812 4.21165 10.296 5.12907 13.5257 6.96393C16.8667 8.86197 18.5371 9.811 18.8906 11.154C19.0365 11.7084 19.0365 12.2916 18.8906 12.846Z", stroke: "currentColor", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Shield01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconShield(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-shield",
        rsx! {
            path { d: "M18.7088 3.49534C16.8165 2.55382 14.5009 2 12 2C9.4991 2 7.1835 2.55382 5.29116 3.49534C4.36318 3.95706 3.89919 4.18792 3.4496 4.91378C3 5.63965 3 6.34248 3 7.74814V11.2371C3 16.9205 7.54236 20.0804 10.173 21.4338C10.9067 21.8113 11.2735 22 12 22C12.7265 22 13.0933 21.8113 13.8269 21.4338C16.4576 20.0804 21 16.9205 21 11.2371L21 7.74814C21 6.34249 21 5.63966 20.5504 4.91378C20.1008 4.18791 19.6368 3.95706 18.7088 3.49534Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Add01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconPlus(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-plus",
        rsx! {
            path { d: "M12.001 5.00003V19.002", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M19.002 12.002L4.99998 12.002", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Delete01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconTrash(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-trash",
        rsx! {
            path { d: "M19.5 5.5L18.8803 15.5251C18.7219 18.0864 18.6428 19.3671 18.0008 20.2879C17.6833 20.7431 17.2747 21.1273 16.8007 21.416C15.8421 22 14.559 22 11.9927 22C9.42312 22 8.1383 22 7.17905 21.4149C6.7048 21.1257 6.296 20.7408 5.97868 20.2848C5.33688 19.3626 5.25945 18.0801 5.10461 15.5152L4.5 5.5", stroke: "currentColor", stroke_linecap: "round", stroke_width: "1.5" }
            path { d: "M3 5.5H21M16.0557 5.5L15.3731 4.09173C14.9196 3.15626 14.6928 2.68852 14.3017 2.39681C14.215 2.3321 14.1231 2.27454 14.027 2.2247C13.5939 2 13.0741 2 12.0345 2C10.9688 2 10.436 2 9.99568 2.23412C9.8981 2.28601 9.80498 2.3459 9.71729 2.41317C9.32164 2.7167 9.10063 3.20155 8.65861 4.17126L8.05292 5.5", stroke: "currentColor", stroke_linecap: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `Copy01Icon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconCopy(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-copy",
        rsx! {
            path { d: "M7.5 14.5C7.5 11.2002 7.5 9.55025 8.52513 8.52513C9.55025 7.5 11.2002 7.5 14.5 7.5C17.7998 7.5 19.4497 7.5 20.4749 8.52513C21.5 9.55025 21.5 11.2002 21.5 14.5C21.5 17.7998 21.5 19.4497 20.4749 20.4749C19.4497 21.5 17.7998 21.5 14.5 21.5C11.2002 21.5 9.55025 21.5 8.52513 20.4749C7.5 19.4497 7.5 17.7998 7.5 14.5Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M7.5 16.5C6.10355 16.5 5.40533 16.5 4.84402 16.3036C3.83866 15.9518 3.0482 15.1613 2.69641 14.156C2.5 13.5947 2.5 12.8964 2.5 11.5V9.5C2.5 6.20017 2.5 4.55025 3.52513 3.52513C4.55025 2.5 6.20017 2.5 9.5 2.5H11.5C12.8964 2.5 13.5947 2.5 14.156 2.69641C15.1613 3.0482 15.9518 3.83866 16.3036 4.84402C16.5 5.40533 16.5 6.10355 16.5 7.5", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `CodeIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconCode(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-code",
        rsx! {
            path { d: "M16 6.99998L19.0664 9.64296C20.3554 10.7541 21 11.3096 21 12C21 12.6903 20.3555 13.2459 19.0664 14.357L16 17", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M8 6.99998L4.93365 9.64296C3.64455 10.7541 3 11.3096 3 12C3 12.6903 3.64455 13.2459 4.93365 14.357L8 17", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `CommandIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconCommand(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-command",
        rsx! {
            path { d: "M15 9V15H9V9H15Z", stroke: "currentColor", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M15 15H18C19.6569 15 21 16.3431 21 18C21 19.6569 19.6569 21 18 21C16.3431 21 15 19.6569 15 18V15Z", stroke: "currentColor", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M9 15.002H6C4.34315 15.002 3 16.3451 3 18.002C3 19.6588 4.34315 21.002 6 21.002C7.65685 21.002 9 19.6588 9 18.002V15.002Z", stroke: "currentColor", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M15 9L15 6C15 4.34315 16.3431 3 18 3C19.6569 3 21 4.34315 21 6C21 7.65685 19.6569 9 18 9H15Z", stroke: "currentColor", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M9 9V6C9 4.34315 7.65685 3 6 3C4.34315 3 3 4.34315 3 6C3 7.65685 4.34315 9 6 9H9Z", stroke: "currentColor", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `UserGroupIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconUsers(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-users",
        rsx! {
            path { d: "M18.4995 20.5C18.2663 17.5685 15.8417 15.2477 12.808 15.0521L11.9995 15C11.7107 15.0076 11.4416 15.0178 11.1877 15.0298C8.18075 15.1723 5.7304 17.5974 5.49951 20.5", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M15.2495 9.25C15.2495 11.0449 13.7944 12.5 11.9995 12.5C10.2046 12.5 8.74952 11.0449 8.74952 9.25C8.74952 7.45507 10.2046 6 11.9995 6C13.7944 6 15.2495 7.45507 15.2495 9.25Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M5.50249 8.5C5.17908 7.99485 4.99158 7.39432 4.99158 6.75C4.99158 4.95507 6.44665 3.5 8.24157 3.5C8.68752 3.5 9.1125 3.58982 9.49939 3.75235", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M18.4963 8.5C18.8197 7.99485 19.0072 7.39432 19.0072 6.75C19.0072 4.95507 17.5521 3.5 15.7572 3.5C15.3113 3.5 14.8863 3.58982 14.4994 3.75235", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M22.0007 17.9996C21.8208 15.7374 19.9995 13.5 17.9995 13", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M1.99927 17.9996C2.17923 15.7374 4.00049 13.5 6.00049 13", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `ZoomInIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconZoomIn(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-zoom-in",
        rsx! {
            path { d: "M17 17L21 21", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M19 11C19 6.58172 15.4183 3 11 3C6.58172 3 3 6.58172 3 11C3 15.4183 6.58172 19 11 19C15.4183 19 19 15.4183 19 11Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M7.5 11L14.5 11M11 7.5V14.5", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `ZoomOutIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconZoomOut(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-zoom-out",
        rsx! {
            path { d: "M17 17L21 21", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M19 11C19 6.58172 15.4183 3 11 3C6.58172 3 3 6.58172 3 11C3 15.4183 6.58172 19 11 19C15.4183 19 19 15.4183 19 11Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M7.5 11L14.5 11", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `FilterIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconFilter(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-filter",
        rsx! {
            path { d: "M8.85746 12.5061C6.36901 10.6456 4.59564 8.59915 3.62734 7.44867C3.3276 7.09253 3.22938 6.8319 3.17033 6.3728C2.96811 4.8008 2.86701 4.0148 3.32795 3.5074C3.7889 3 4.60404 3 6.23433 3H17.7657C19.396 3 20.2111 3 20.672 3.5074C21.133 4.0148 21.0319 4.8008 20.8297 6.37281C20.7706 6.83191 20.6724 7.09254 20.3726 7.44867C19.403 8.60062 17.6261 10.6507 15.1326 12.5135C14.907 12.6821 14.7583 12.9567 14.7307 13.2614C14.4837 15.992 14.2559 17.4876 14.1141 18.2442C13.8853 19.4657 12.1532 20.2006 11.226 20.8563C10.6741 21.2466 10.0043 20.782 9.93278 20.1778C9.79643 19.0261 9.53961 16.6864 9.25927 13.2614C9.23409 12.9539 9.08486 12.6761 8.85746 12.5061Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

/// Hugeicons `InformationCircleIcon` (core-free-icons 4.3.5; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn IconInfo(props: IconProps) -> Element {
    render_svg(
        &props,
        "icon-info",
        rsx! {
            circle { cx: "12", cy: "12", r: "10", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M12 16V12", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
            path { d: "M12.125 8.25H12M12.25 8.25C12.25 8.11193 12.1381 8 12 8C11.8619 8 11.75 8.11193 11.75 8.25C11.75 8.38807 11.8619 8.5 12 8.5C12.1381 8.5 12.25 8.38807 12.25 8.25Z", stroke: "currentColor", stroke_linecap: "round", stroke_linejoin: "round", stroke_width: "1.5" }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_graph_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconGraph {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 7);
        assert!(html.contains("data-testid=\"icon-graph\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_classroom_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconClassroom {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 3);
        assert!(html.contains("data-testid=\"icon-classroom\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_digest_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconDigest {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 3);
        assert!(html.contains("data-testid=\"icon-digest\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_sparkles_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconSparkles {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-sparkles\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_check_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconCheck {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-check\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_x_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconX {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-x\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_chevron_right_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconChevronRight {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-chevron-right\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_chevron_down_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconChevronDown {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-chevron-down\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_chevron_left_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconChevronLeft {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-chevron-left\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_chevron_up_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconChevronUp {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-chevron-up\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_search_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconSearch {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-search\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_settings_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconSettings {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-settings\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_terminal_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconTerminal {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 3);
        assert!(html.contains("data-testid=\"icon-terminal\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_book_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconBook {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-book\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_brain_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconBrain {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-brain\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_refresh_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconRefresh {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-refresh\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_play_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconPlay {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-play\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_shield_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconShield {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-shield\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_plus_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconPlus {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-plus\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_trash_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconTrash {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-trash\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_copy_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconCopy {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-copy\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_code_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconCode {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-code\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_command_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconCommand {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 5);
        assert!(html.contains("data-testid=\"icon-command\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_users_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconUsers {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 6);
        assert!(html.contains("data-testid=\"icon-users\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_zoom_in_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconZoomIn {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 3);
        assert!(html.contains("data-testid=\"icon-zoom-in\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_zoom_out_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconZoomOut {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 3);
        assert!(html.contains("data-testid=\"icon-zoom-out\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_filter_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconFilter {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 1);
        assert!(html.contains("data-testid=\"icon-filter\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_info_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { IconInfo {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), 2);
        assert!(html.contains("data-testid=\"icon-info\""));
        assert!(html.contains("aria-hidden=\"true\""));
    }

    #[test]
    fn icon_props_preserve_accessibility_size_and_attributes() {
        let html = dioxus::ssr::render_element(rsx! {
            IconGraph {
                size: 20,
                class: "graph-icon",
                aria_label: "Knowledge Graph",
                test_id: "custom-graph",
                title: "Graph",
            }
        });
        for attribute in [
            "width=\"20\"",
            "height=\"20\"",
            "class=\"graph-icon\"",
            "aria-label=\"Knowledge Graph\"",
            "role=\"img\"",
            "data-testid=\"custom-graph\"",
            "title=\"Graph\"",
            "stroke=\"currentColor\"",
        ] {
            assert!(html.contains(attribute), "Missing {attribute}");
        }
        assert!(!html.contains("aria-hidden="));
    }
}
