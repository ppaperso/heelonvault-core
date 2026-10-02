# HEELONYS Design Guidelines

Language: EN | [FR](CHARTE_GRAPHIQUE.md)

## Color Palette

### Primary tones (from logo)

- Deep Teal: `#07393A` (headings, key text, active navigation)
- Dark Teal: `#0A5F5C` (secondary UI elements)
- Medium Teal: `#1F8678` (primary actions)
- Bright Teal: `#13A1A1` (accents and highlights)
- Soft Teal: `#57B9B1`
- Light Teal: `#6FD3B4`
- Very Light Teal: `#A4DFCF`

### Neutral tones

- Off White: `#F3F6F3`
- Pure White: `#FFFFFF`
- Charcoal: `#2C3E50`
- Gray: `#7F8C9A`

### Support tones

- Success Green: `#6FD3B4`
- Warning Teal: `#13A1A1`
- Error Red: `#E74C3C`

## Typography

Primary family: Space Grotesk

- H1: 3.4rem / 54px (700)
- H2: 2rem / 32px (600)
- H3: 1.2rem / 19px (600)
- Body: 1rem / 16px (400)
- Small: 0.85rem / 14px (400)

Line height:

- Titles: 1.2
- Paragraphs: 1.6

## UI Principles

### Primary color usage (`#07393A`)

- section titles
- logo text
- active navigation
- footer background

### Accent color usage (`#13A1A1`, `#1F8678`)

- call-to-action buttons
- interactive links
- important icons
- active underline

### Backgrounds

1. Main: `#F3F6F3`
2. Cards: `#FFFFFF`
3. Dark sections: gradient `#07393A -> #0A5F5C`
4. Light sections: `#A4DFCF`

## Components

### Primary button

- background: `linear-gradient(120deg, #13A1A1, #1F8678)`
- text: white
- radius: 999px
- hover: slight upward translation and stronger shadow

### Outline button

- transparent background
- border: 1px `#07393A`
- text: `#07393A`
- radius: 999px

### Card

- white background
- border: 1px `#A4DFCF`
- radius: 20px
- shadow: `0 20px 40px rgba(7, 57, 58, 0.08)`

## Accessibility

- WCAG 2.1 AA minimum
- visible focus states on all interactive elements
- support reduced-motion preferences

## Responsive

- Mobile: < 600px
- Tablet: 600px - 960px
- Desktop: > 960px

Grid suggestion:

- `repeat(auto-fit, minmax(280px, 1fr))`

## Standard Animations

- duration: 0.2s to 0.25s
- timing: `ease` or `cubic-bezier(0.4, 0, 0.2, 1)`
