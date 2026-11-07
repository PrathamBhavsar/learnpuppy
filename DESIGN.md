---
name: LearnPuppy site
description: Violet-black, pixel-typed landing and docs site where a pixel pup sits behind a knowledge graph and a synthetic forgetting curve jumps back at each review.
colors:
  accent: "#ff6ad5"
  accent-ink: "#09080d"
  ink: "#09080d"
  ink-raised: "#100e17"
  ink-sunken: "#181522"
  bone: "#f0ecf7"
  bone-dim: "#a8a0bd"
  hairline: "#1f1b2b"
  hairline-strong: "#342e47"
typography:
  display:
    fontFamily: "Geist Pixel Square, ui-monospace, SF Mono, monospace"
    fontSize: "clamp(2.5rem, 6vw, 5rem)"
    fontWeight: 400
    lineHeight: 1.04
    letterSpacing: "0"
  headline:
    fontFamily: "Geist Pixel Square, ui-monospace, SF Mono, monospace"
    fontSize: "clamp(2rem, 4.4vw, 3.5rem)"
    fontWeight: 400
    lineHeight: 1.04
    letterSpacing: "0"
  title:
    fontFamily: "Geist Pixel Square, ui-monospace, SF Mono, monospace"
    fontSize: "clamp(1.25rem, 2vw, 1.625rem)"
    fontWeight: 400
    lineHeight: 1.2
    letterSpacing: "0"
  body:
    fontFamily: "Satoshi, system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1.0625rem"
    fontWeight: 400
    lineHeight: 1.6
  label:
    fontFamily: "Geist Mono, ui-monospace, SF Mono, Menlo, monospace"
    fontSize: "0.8125rem"
    fontWeight: 400
    letterSpacing: "0.04em"
  code:
    fontFamily: "Geist Mono, ui-monospace, SF Mono, Menlo, monospace"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.75
rounded:
  none: "0"
spacing:
  gutter: "clamp(16px, 4vw, 48px)"
  section: "clamp(64px, 10vw, 128px)"
  nav-height: "64px"
  max-width: "1240px"
components:
  button-default:
    backgroundColor: "transparent"
    textColor: "{colors.bone}"
    rounded: "{rounded.none}"
    height: "48px"
    padding: "0 20px"
  button-default-hover:
    backgroundColor: "{colors.ink-sunken}"
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-ink}"
    rounded: "{rounded.none}"
    height: "48px"
    padding: "0 20px"
  button-primary-hover:
    backgroundColor: "{colors.bone}"
  button-ink:
    backgroundColor: "{colors.accent-ink}"
    textColor: "{colors.accent}"
    rounded: "{rounded.none}"
    height: "48px"
    padding: "0 20px"
  copy-line:
    backgroundColor: "{colors.ink-raised}"
    textColor: "{colors.bone}"
    rounded: "{rounded.none}"
  code-panel:
    backgroundColor: "{colors.accent-ink}"
    textColor: "{colors.bone}"
    rounded: "{rounded.none}"
    padding: "20px"
---

# Design System: LearnPuppy site

## Overview

**Creative North Star: "The Pup at the Graph"**

The site is a dark study sheet: violet-black ink ground, lavender-bone text, 1px hairlines, square corners, pixel-grid display type. Warm and a little playful in the hero, plain and exact everywhere else. The hero shows the mechanism: an ASCII pixel pup behind a knowledge graph whose edges carry pulses and whose picked nodes light up, plus a forgetting curve that decays and jumps back at each review. The curve is labelled a synthetic illustration, never a result; the rest of the page is receipts, rows, and rules rather than cards and tiles.

One saturated field per page carries the brand color at scale: the install band, drenched in hot pink with ink-black type. Elsewhere the accent is a sparse mark (a link, an icon, a focus ring, a node). Dark only; there is no light theme.

The UI kit (`site/src/kit/`) is shared by two sibling sites, bunfiiliate (acid green) and Deskpost (amber), alongside LearnPuppy (pink). Each overrides only the `--accent`, `--bg*`, and `--fg*` custom properties in `site/src/brand.css`. This document records the LearnPuppy skin; the structure, type, hairlines, and components below belong to the kit and must not be forked per brand.

**Key Characteristics:**
- Pixel display type over a quiet humanist body; mono only for code, figures, and facts.
- Square corners everywhere; separation by 1px hairlines and tonal steps, never shadows.
- One drenched accent band; sparse accent marks elsewhere.
- Receipt forms (dashed rows, dotted leaders, real facts only) instead of icon-card grids.
- Canvas is decoration only: aria-hidden, reduced-motion renders a still frame, all copy is real text.

## Colors

A violet-black ink palette with lavender-bone text and a single hot-pink accent.

### Primary
- **Hot Pink** (`accent`, #ff6ad5): the install band field, primary buttons, links in prose, list markers, icons in feature rows, stars count, focus ring, selection, ASCII canvas tone 2.
- **Ink on Pink** (`accent-ink`, #09080d): text, borders, and code panel on top of the pink field; also text on primary buttons.

### Neutral
- **Ink** (`ink`, #09080d): page ground and nav.
- **Raised Ink** (`ink-raised`, #100e17): copy line, steps band, footer, doc code blocks, blockquotes, search dialog.
- **Sunken Ink** (`ink-sunken`, #181522): button hover, inline code chips.
- **Bone** (`bone`, #f0ecf7): body text, headings.
- **Dim Bone** (`bone-dim`, #a8a0bd): ledes, secondary copy, inactive nav links, canvas tone 0.
- **Hairline** (`hairline`, #1f1b2b): section and row dividers.
- **Strong Hairline** (`hairline-strong`, #342e47): control borders, receipt dashes, scrollbar thumbs.

### Named Rules
**The One Field Rule.** Exactly one saturated hot-pink field per page, the install band. Everything else uses the accent as a mark, never as a second block of color.

**The Skin Seam Rule.** A brand skin changes only `--accent`, `--accent-ink`, `--bg*`, `--fg*`. Never hardcode a hex in component CSS; read the token (the canvas reads tones from the computed custom properties).

## Typography

**Display Font:** Geist Pixel Square (fallback ui-monospace, SF Mono)
**Body Font:** Satoshi 400/500/700 (fallback system-ui)
**Label/Mono Font:** Geist Mono 400 (fallback ui-monospace, Menlo)

**Character:** A bitmap-cut display face gives the pixel-grid precision; Satoshi keeps long reading calm. Mono is reserved for things that are literally code or figures.

### Hierarchy
- **Display** (400, clamp(2.5rem, 6vw, 5rem), 1.04): hero h1, max 15ch, accent `em` for the single emphasized word.
- **Headline** (400, clamp(2rem, 4.4vw, 3.5rem), 1.04): section h2 (`.display`), capped at 22ch.
- **Title** (400, clamp(1.25rem, 2vw, 1.625rem), 1.2): group heads; flow step h3 at 1.375rem; doc h1 clamp(2.25rem, 4vw, 3.25rem), doc h2 1.75rem.
- **Body** (400, 1.0625rem, 1.6): prose; ledes in dim bone at clamp(1.0625rem, 1.5vw, 1.25rem), 46-52ch; doc prose 72ch; row titles Satoshi 700 at 1.0625rem.
- **Label** (Geist Mono 400, 0.8125rem, 0.04em): receipt keys (uppercase), hero meta, star count, footer end line. Buttons and tabs are Satoshi 700 at about 0.9375rem.

### Named Rules
**The Mono Is Literal Rule.** Geist Mono appears only for code, commands, figures, and facts; never for decorative headings or labels that are not data.

**The Pixel Is Display Rule.** The pixel face is used for headings, wordmark, and the faq "+" only, always weight 400 and letter-spacing 0; body text is never pixel.

## Layout

Single column inside `.wrap` (max 1240px, gutter clamp(16px, 4vw, 48px)), sections separated by 1px hairlines and vertical padding clamp(64px, 10vw, 128px). Asymmetric two-column grids (5fr/6fr live facts, 4fr/8fr feature groups with a sticky head, 5fr/7fr install, 3fr/7fr flow) collapse to one column at 860px; the nav collapses to a toggle menu at 760px. The hero is min(100svh minus nav, 860px) with a left-to-right scrim keeping text legible over the canvas; on mobile the canvas sits below the copy. Docs use a 240px sidebar, fluid 78ch article, and 200px TOC, dropping the TOC at 1100px and the sidebar into a toggle at 760px. Spacing rhythm is 4/8 steps (8, 12, 14, 16, 20, 28, 32) inside components.

## Elevation & Depth

Flat. There are no box-shadows. Depth is tonal (ink, raised ink, sunken ink) plus 1px hairlines, and the one pink band. The only overlay is the search dialog, which uses a 72% black backdrop and a hairline border. Hover is a border or fill change, active is a 1px downward nudge.

### Named Rules
**The Hairline Depth Rule.** Separate surfaces with a 1px line or a tonal step. No shadows, blur, or glass.

## Shapes

Square. Radius is 0 on buttons, inputs, code, tabs, dialogs, chips, and nodes. Borders are 1px solid hairline-strong (controls) or hairline (dividers); receipts use dashed and dotted 1px lines. Recurring silhouette: the 23px square timeline node with a 9px accent square inside, and the segmented tablist with hard 1px dividers.

## Components

### Buttons
- **Shape:** Square, 48px tall (40px in nav), 1px border, 20px inline padding, Satoshi 700.
- **Default:** transparent, hairline-strong border; hover brightens border to bone and fills sunken ink; active nudges 1px down.
- **Primary:** pink fill, ink text; hover flips to bone fill.
- **Ink:** ink fill with pink text, used only on the pink band; hover goes transparent with ink text.
- **Motion:** 0.2s on `cubic-bezier(0.16, 1, 0.3, 1)`.

### Copy line and code panels
Raised ink box with a pink `$` prompt, Geist Mono, and a 48px square copy button that fills pink on hover and shows accent when copied. On the pink band the code panel is ink-black with a tablist above it (selected tab inverts to ink fill, pink text).

### Navigation
Sticky, ink ground, 64px, 1px bottom hairline. Wordmark in pixel face with the accent letter in pink; links Satoshi 500, dim bone, bone on hover or `aria-current`. GitHub button carries a live star count in mono pink (hidden until real data loads). Docs sidebar marks the current page with an pink text and a 1px left rule.

### Receipt (live facts)
Mono list with dashed top and bottom rules per row, uppercase dim key, dotted leader, right-aligned bone value; links in pink.

### Feature rows, flow, FAQ
Feature rows pair a Hugeicons icon (pink, 28px column) with a bold title and dim copy under a sticky group head. Flow is a vertical hairline with square nodes. FAQ uses native `details` with a rotating "+" marker.

### ASCII pup and knowledge graph canvas (signature)
Full-bleed `x-ascii` canvas behind the hero (scene `src/scenes/graph.js`), aria-hidden, three tones (dim bone, bone, pink) read from tokens. A 30x20 pup built from ellipses (`#` body and head, `@` ears, `o` muzzle and paws, `N` nose, `E` eyes) sits behind topic nodes (recursion, hashing, trees...) joined by edges that carry pulses; picked nodes light up. A 36x8 forgetting-curve plot with review ticks decays and jumps back on a 9s cycle, with made-up intervals. `prefers-reduced-motion` renders a stable still frame. Hero copy rises in once (0.9s) on load; no other entrance motion.

## Do's and Don'ts

### Do:
- **Do** keep every corner square and every divider a 1px hairline.
- **Do** use Hugeicons through the kit icon set for all icons.
- **Do** keep real facts only (bind address, data folder, supported CLIs, known limits) and hide a figure rather than invent it; no benchmarks.
- **Do** keep text in real DOM; the canvas is decorative and aria-hidden.
- **Do** keep a 2px accent focus ring with 3px offset (ink on the pink band).
- **Do** let sibling brands re-skin through `brand.css` tokens only.

### Don't:
- **Don't** add shadows, glass, gradients as decoration, or rounded corners; the only gradient is the hero legibility scrim.
- **Don't** introduce a second saturated field or a light theme.
- **Don't** build icon-card grids, hero metric tiles, fake logos, invented user counts, or retention claims; the forgetting curve stays labelled synthetic.
- **Don't** use Geist Mono or the pixel face for body copy.
- **Don't** hardcode brand colors in kit CSS or JS.

## Not canonized

Drift carried by the build, not recorded as rules: `brand.css` overrides hero h1 size and receipt wrapping as one-offs, not new tokens; the FAQ "+" marker is a typed glyph in the pixel face rather than an icon, and some doc sizes (0.9688rem, 0.8438rem) are one-off fractional values.
