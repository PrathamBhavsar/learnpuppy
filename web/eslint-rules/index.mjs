// Project lint rules (docs/web/07 "Lint rules worth automating").

/** @param {{ type: string, value?: unknown }} node */
function stringsOf(node) {
  if (node.type === "Literal" && typeof node.value === "string")
    return [node.value]
  if (node.type === "TemplateElement")
    return [node.value.cooked ?? node.value.raw]
  return []
}

/** Build a rule that reports string literals / template chunks matching `pattern`. */
function stringPatternRule({ pattern, message, description }) {
  return {
    meta: {
      type: "problem",
      docs: { description },
      schema: [],
      messages: { found: message },
    },
    create(context) {
      const check = (node) => {
        for (const text of stringsOf(node)) {
          const match = pattern.exec(text)
          if (match)
            context.report({
              node,
              messageId: "found",
              data: { match: match[0].trim() },
            })
        }
      }
      return { Literal: check, TemplateElement: check }
    },
  }
}

// Variant prefixes (`md:`, `hover:`, `rtl:`) and the important/negative markers may precede a class.
const CLASS_START = String.raw`(?:^|[\s"'\`:\[(])[!-]?`

export default {
  rules: {
    "no-physical-direction": stringPatternRule({
      description:
        "Use logical Tailwind utilities (ms-, me-, ps-, pe-, start-, end-, text-start) so RTL mirrors.",
      message:
        "Physical direction class '{{match}}' breaks RTL. Use ms-/me-/ps-/pe-/start-/end-/text-start/text-end/rounded-s/rounded-e/border-s/border-e.",
      pattern: new RegExp(
        `${CLASS_START}(?:(?:ml|mr|pl|pr|left|right|scroll-ml|scroll-mr|scroll-pl|scroll-pr)-(?:\\d|\\[|\\(|px\\b|auto\\b|full\\b)|text-(?:left|right)\\b|float-(?:left|right)\\b|(?:rounded|border)-(?:l|r|tl|tr|bl|br)(?:-|\\b))`
      ),
    }),
    "no-color-literal": stringPatternRule({
      description:
        "Colors come from tokens (src/styles/tokens.css); no hex/rgb/hsl/oklch literals in code.",
      message:
        "Color literal '{{match}}' outside tokens. Use a token class or var(--token).",
      pattern:
        /#[0-9a-fA-F]{8}\b|#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{3,4}\b|\b(?:rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color)\(/,
    }),
    "no-layout-transition": stringPatternRule({
      description: "Animate transform/opacity only; never layout properties.",
      message:
        "'{{match}}' animates layout. Transition transform/opacity (or use motion layout animations).",
      pattern: new RegExp(
        `${CLASS_START}transition-(?:all\\b|\\[[^\\]]*\\b(?:width|height|top|left|right|bottom|margin|padding)\\b)`
      ),
    }),
  },
}
