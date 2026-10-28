use dioxus::prelude::*;

/// Embedded dark-first OKLCH stylesheet for LearnPuppy (`lp-ui` and `lp-app`).
///
/// Defines core `shadcn/ui` dark-mode design tokens in OKLCH color space,
/// mastery ring semantic tokens, base reset rules for native macOS `WKWebView`
/// (preventing root double scrollbars), and utility/component classes.
pub const LP_THEME_CSS: &str = r#"
:root, .dark {
  color-scheme: dark;
  --background: oklch(0.145 0.014 265);
  --foreground: oklch(0.965 0.007 265);
  --card: oklch(0.185 0.016 265);
  --card-foreground: oklch(0.965 0.007 265);
  --popover: oklch(0.195 0.018 265);
  --popover-foreground: oklch(0.965 0.007 265);
  --primary: oklch(0.72 0.16 215);
  --primary-foreground: oklch(0.145 0.014 265);
  --secondary: oklch(0.245 0.02 265);
  --secondary-foreground: oklch(0.92 0.01 265);
  --muted: oklch(0.225 0.018 265);
  --muted-foreground: oklch(0.68 0.018 265);
  --accent: oklch(0.265 0.025 265);
  --accent-foreground: oklch(0.965 0.007 265);
  --destructive: oklch(0.62 0.21 25);
  --destructive-foreground: oklch(0.98 0.005 25);
  --success: oklch(0.72 0.17 155);
  --success-foreground: oklch(0.145 0.014 155);
  --warning: oklch(0.78 0.16 80);
  --warning-foreground: oklch(0.145 0.014 80);
  --info: oklch(0.72 0.15 235);
  --info-foreground: oklch(0.145 0.014 235);
  --border: oklch(0.275 0.02 265);
  --input: oklch(0.255 0.02 265);
  --ring: oklch(0.72 0.16 215);
  --radius: 0.5rem;
  --mastery-novice: oklch(0.64 0.18 30);
  --mastery-learning: oklch(0.78 0.16 85);
  --mastery-mastered: oklch(0.74 0.18 155);
  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", system-ui, sans-serif;
  --font-mono: "SF Mono", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

*, *::before, *::after {
  box-sizing: border-box;
  border-color: var(--border);
}

html, body {
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background-color: var(--background);
  color: var(--foreground);
  font-family: var(--font-sans);
  -webkit-font-smoothing: antialiased;
}

/* Core layout & theme utilities */
.bg-background { background-color: var(--background); }
.text-foreground { color: var(--foreground); }
.bg-card { background-color: var(--card); }
.text-card-foreground { color: var(--card-foreground); }
.bg-popover { background-color: var(--popover); }
.text-popover-foreground { color: var(--popover-foreground); }
.bg-primary { background-color: var(--primary); }
.text-primary-foreground { color: var(--primary-foreground); }
.bg-secondary { background-color: var(--secondary); }
.text-secondary-foreground { color: var(--secondary-foreground); }
.bg-muted { background-color: var(--muted); }
.text-muted-foreground { color: var(--muted-foreground); }
.bg-accent { background-color: var(--accent); }
.text-accent-foreground { color: var(--accent-foreground); }
.bg-destructive { background-color: var(--destructive); }
.text-destructive-foreground { color: var(--destructive-foreground); }
.border-border { border-color: var(--border); }

.h-screen { height: 100vh; }
.w-screen { width: 100vw; }
.overflow-hidden { overflow: hidden; }
.overflow-y-auto { overflow-y: auto; }
.overflow-x-auto { overflow-x: auto; }
.select-none { user-select: none; -webkit-user-select: none; }
.flex { display: flex; }
.inline-flex { display: inline-flex; }
.flex-col { flex-direction: column; }
.flex-row { flex-direction: row; }
.items-center { align-items: center; }
.justify-center { justify-content: center; }
.justify-between { justify-content: space-between; }

/* Component primitives */
.lp-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  white-space: nowrap;
  border-radius: var(--radius);
  font-size: 0.875rem;
  font-weight: 500;
  transition: background-color 120ms ease, color 120ms ease, border-color 120ms ease, opacity 120ms ease;
  cursor: pointer;
  border: 1px solid transparent;
  outline: none;
}
.lp-btn:focus-visible {
  box-shadow: 0 0 0 2px var(--background), 0 0 0 4px var(--ring);
}
.lp-btn:disabled, .lp-btn[aria-disabled="true"] {
  pointer-events: none;
  opacity: 0.5;
}
.lp-btn-primary {
  background-color: var(--primary);
  color: var(--primary-foreground);
}
.lp-btn-secondary {
  background-color: var(--secondary);
  color: var(--secondary-foreground);
}
.lp-btn-outline {
  background-color: transparent;
  border-color: var(--border);
  color: var(--foreground);
}
.lp-btn-ghost {
  background-color: transparent;
  color: var(--foreground);
}
.lp-btn-destructive {
  background-color: var(--destructive);
  color: var(--destructive-foreground);
}
.lp-btn-link {
  background-color: transparent;
  color: var(--primary);
  text-decoration: underline;
  text-underline-offset: 4px;
}
.lp-btn-xs { height: 1.75rem; padding: 0 0.5rem; font-size: 0.75rem; }
.lp-btn-sm { height: 2rem; padding: 0 0.75rem; font-size: 0.8125rem; }
.lp-btn-default { height: 2.25rem; padding: 0 1rem; }
.lp-btn-lg { height: 2.5rem; padding: 0 1.25rem; font-size: 0.9375rem; }
.lp-btn-icon { height: 2.25rem; width: 2.25rem; padding: 0; }

.lp-badge {
  display: inline-flex;
  align-items: center;
  border-radius: 9999px;
  border: 1px solid transparent;
  padding: 0.125rem 0.625rem;
  font-size: 0.75rem;
  font-weight: 600;
  line-height: 1.25;
}
.lp-badge-default { background-color: var(--primary); color: var(--primary-foreground); }
.lp-badge-secondary { background-color: var(--secondary); color: var(--secondary-foreground); }
.lp-badge-success { background-color: var(--success); color: var(--success-foreground); }
.lp-badge-warning { background-color: var(--warning); color: var(--warning-foreground); }
.lp-badge-destructive { background-color: var(--destructive); color: var(--destructive-foreground); }
.lp-badge-outline { background-color: transparent; border-color: var(--border); color: var(--foreground); }
.lp-badge-phase { background-color: var(--info); color: var(--info-foreground); letter-spacing: 0.04em; text-transform: uppercase; }

.lp-card {
  border-radius: calc(var(--radius) + 2px);
  border: 1px solid var(--border);
  background-color: var(--card);
  color: var(--card-foreground);
  box-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.35);
}
.lp-card-header { display: flex; flex-direction: column; gap: 0.375rem; padding: 1.25rem; }
.lp-card-title { font-size: 1.0625rem; font-weight: 600; line-height: 1.25; margin: 0; }
.lp-card-description { font-size: 0.875rem; color: var(--muted-foreground); margin: 0; }
.lp-card-content { padding: 0 1.25rem 1.25rem 1.25rem; }
.lp-card-footer { display: flex; align-items: center; padding: 0 1.25rem 1.25rem 1.25rem; gap: 0.5rem; }

.lp-separator {
  flex-shrink: 0;
  background-color: var(--border);
}
.lp-separator-horizontal { height: 1px; width: 100%; }
.lp-separator-vertical { width: 1px; height: 100%; }

.lp-progress {
  position: relative;
  height: 0.5rem;
  width: 100%;
  overflow: hidden;
  border-radius: 9999px;
  background-color: var(--secondary);
}
.lp-progress-indicator {
  height: 100%;
  background-color: var(--primary);
  transition: width 180ms ease;
}

.lp-code-block {
  border-radius: var(--radius);
  border: 1px solid var(--border);
  background-color: var(--card);
  color: var(--card-foreground);
  font-family: var(--font-mono);
  font-size: 0.8125rem;
  overflow: hidden;
}
.lp-code-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.375rem 0.75rem;
  background-color: var(--muted);
  border-bottom: 1px solid var(--border);
}
.lp-code-lines {
  display: flex;
  flex-direction: column;
  padding: 0.5rem 0;
  overflow-x: auto;
}
.lp-code-line {
  display: flex;
  align-items: baseline;
  padding: 0 0.75rem;
  line-height: 1.5;
}
.lp-code-gutter {
  user-select: none;
  -webkit-user-select: none;
  min-width: 2.25rem;
  padding-right: 0.75rem;
  text-align: right;
  color: var(--muted-foreground);
  font-variant-numeric: tabular-nums;
}
.lp-code-content {
  white-space: pre;
  flex: 1;
}
"#;

/// Injects the embedded `LP_THEME_CSS` dark-first OKLCH stylesheet into the document.
#[component]
pub fn ThemeStylesheet() -> Element {
    rsx! {
        style {
            "data-testid": "lp-theme-stylesheet",
            "{LP_THEME_CSS}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_css_contains_required_oklch_tokens() {
        let required_tokens = [
            "--background:",
            "--foreground:",
            "--card:",
            "--popover:",
            "--primary:",
            "--secondary:",
            "--muted:",
            "--accent:",
            "--destructive:",
            "--success:",
            "--warning:",
            "--info:",
            "--border:",
            "--ring:",
            "--mastery-novice:",
            "--mastery-learning:",
            "--mastery-mastered:",
            "oklch(",
        ];
        for token in required_tokens {
            assert!(
                LP_THEME_CSS.contains(token),
                "LP_THEME_CSS is missing required token `{token}`"
            );
        }
    }

    #[test]
    fn theme_stylesheet_renders_ssr() {
        let html = dioxus::ssr::render_element(rsx! { ThemeStylesheet {} });
        assert!(html.contains("data-testid=\"lp-theme-stylesheet\""));
        assert!(html.contains("--mastery-mastered"));
    }
}
