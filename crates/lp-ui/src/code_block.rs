use dioxus::prelude::*;

use crate::icons::{IconCheck, IconCopy};
use crate::primitives::{Badge, BadgeVariant, Button, ButtonSize, ButtonVariant};

#[derive(Props, Clone, PartialEq)]
pub struct CodeBlockProps {
    /// Raw source code string to render.
    #[props(into)]
    pub code: String,
    /// Optional programming language identifier (e.g. `"rust"`, `"sql"`, `"toml"`).
    #[props(into, default)]
    pub language: Option<String>,
    /// Optional filename or caption displayed in the header bar.
    #[props(into, default)]
    pub title: Option<String>,
    /// Whether to render the 1-indexed line number gutter (defaults to `true`).
    #[props(default = true)]
    pub show_line_numbers: bool,
    /// Starting line number for the gutter (defaults to `1`).
    #[props(default = 1)]
    pub start_line: usize,
    /// Optional 1-indexed line numbers to visually highlight.
    #[props(default)]
    pub highlight_lines: Vec<usize>,
    /// Optional callback invoked with the code string when the copy button is clicked.
    #[props(default)]
    pub on_copy: Option<EventHandler<String>>,
    #[props(into, default)]
    pub class: String,
    #[props(into, default)]
    pub aria_label: Option<String>,
    #[props(into, default)]
    pub test_id: Option<String>,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// Pure-Rust code block viewer with line number gutter, language badge, and signal-driven copy state.
#[component]
pub fn CodeBlock(props: CodeBlockProps) -> Element {
    let mut copied = use_signal(|| false);
    let tid = props.test_id.as_deref().unwrap_or("lp-code-block");
    let lang = props
        .language
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("text");
    let label = props
        .aria_label
        .clone()
        .unwrap_or_else(|| format!("Code block ({lang})"));
    let full_class = if props.class.is_empty() {
        "lp-code-block".to_string()
    } else {
        format!("lp-code-block {}", props.class)
    };

    let lines: Vec<&str> = if props.code.is_empty() {
        vec![""]
    } else {
        props.code.lines().collect()
    };
    let code_for_copy = props.code.clone();
    let on_copy = props.on_copy;

    rsx! {
        div {
            role: "region",
            "aria-label": "{label}",
            "data-testid": "{tid}",
            "data-language": "{lang}",
            class: "{full_class}",
            ..props.attributes,
            div {
                class: "lp-code-header",
                "data-testid": "{tid}-header",
                div {
                    class: "inline-flex items-center",
                    style: "gap: 0.5rem;",
                    Badge {
                        variant: BadgeVariant::Secondary,
                        test_id: format!("{tid}-language"),
                        "{lang}"
                    }
                    if let Some(title) = &props.title {
                        span {
                            "data-testid": "{tid}-title",
                            class: "text-muted-foreground",
                            style: "font-size: 0.75rem;",
                            "{title}"
                        }
                    }
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    size: ButtonSize::Xs,
                    aria_label: if copied() { "Copied code" } else { "Copy code" },
                    test_id: format!("{tid}-copy"),
                    onclick: move |_| {
                        copied.set(true);
                        if let Some(handler) = &on_copy {
                            handler.call(code_for_copy.clone());
                        }
                    },
                    if copied() {
                        IconCheck { size: 14 }
                        span { "Copied" }
                    } else {
                        IconCopy { size: 14 }
                        span { "Copy" }
                    }
                }
            }
            pre {
                class: "lp-code-lines",
                style: "margin: 0;",
                "data-testid": "{tid}-pre",
                code {
                    for (idx, line) in lines.iter().enumerate() {
                        {
                            let line_no = props.start_line.saturating_add(idx);
                            let is_highlighted = props.highlight_lines.contains(&line_no);
                            let row_style = if is_highlighted {
                                "background-color: color-mix(in oklch, var(--primary) 22%, var(--background)); color: var(--foreground);"
                            } else {
                                ""
                            };
                            rsx! {
                                div {
                                    key: "{line_no}",
                                    class: "lp-code-line",
                                    style: "{row_style}",
                                    "data-testid": "{tid}-line-{line_no}",
                                    "data-highlighted": if is_highlighted { "true" } else { "false" },
                                    if props.show_line_numbers {
                                        span {
                                            class: "lp-code-gutter",
                                            "aria-hidden": "true",
                                            "data-testid": "{tid}-gutter-{line_no}",
                                            "{line_no}"
                                        }
                                    }
                                    span {
                                        class: "lp-code-content",
                                        "{line}"
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

    #[test]
    fn code_block_renders_gutter_lines_and_language_badge() {
        let sample = "fn main() {\n    println!(\"hello learnpuppy\");\n}";
        let html = dioxus::ssr::render_element(rsx! {
            CodeBlock {
                code: sample,
                language: "rust",
                title: "src/main.rs",
                start_line: 10,
                highlight_lines: vec![11],
                test_id: "sample-code",
            }
        });

        assert!(html.contains("data-testid=\"sample-code\""));
        assert!(html.contains("data-language=\"rust\""));
        assert!(html.contains("src/main.rs"));
        assert!(html.contains("data-testid=\"sample-code-gutter-10\""));
        assert!(html.contains("data-testid=\"sample-code-gutter-11\""));
        assert!(html.contains("data-testid=\"sample-code-gutter-12\""));
        assert!(html.contains("data-highlighted=\"true\""));
        assert!(html.contains("println!"));
        assert!(html.contains("data-testid=\"sample-code-copy\""));
    }

    #[test]
    fn code_block_can_hide_line_numbers() {
        let html = dioxus::ssr::render_element(rsx! {
            CodeBlock {
                code: "SELECT * FROM propositions_fts;",
                language: "sql",
                show_line_numbers: false,
                test_id: "sql-code",
            }
        });

        assert!(html.contains("data-testid=\"sql-code\""));
        assert!(!html.contains("sql-code-gutter-1"));
        assert!(html.contains("SELECT * FROM propositions_fts;"));
    }
}
