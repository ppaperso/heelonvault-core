# HeelonVault application design guidelines

Language: EN | [FR](CHARTE_GRAPHIQUE.md)

These guidelines describe the desktop application's look (direction C of the 3.0 visual
refresh, #105): white surfaces, neutral greys, and the Heelonys teal reserved for actions.
They build on the libadwaita (Adwaita) style instead of replacing it.

The code is the source of truth: colours are defined **only** in
`crates/heelonvault-app/assets/style/00-tokens.css`, and the other stylesheets in
`assets/style/` refer to them by name. If this document and the tokens disagree, the tokens
win; fix this document.

The Heelonys website has its own guidelines (Space Grotesk font, gradients, dark sections).
They do **not** apply to the application: only the logo's teal scale is shared.

## Principles

1. **White carries the content.** Cards, lists and forms are white (`hv_surface`) on a
   near-white canvas (`hv_canvas`). Title bars are white.
2. **Text and structure are neutral.** Text, borders and separators are grey, with no brand
   tint.
3. **Teal signals an action or a selection.** Primary button, selected item, focus, active
   switches, links. It is never decorative and never used as a warning.
4. **Semantic colours are for states only.** Success, warning, danger and information, each
   with a light background (`*_bg`) and dark text (`*_strong`).
5. **No gradients, one elevation level.** No coloured banners, no coloured stripe on cards,
   no lift effect on hover. The only exception is the Heelonys certification seal (see
   below).
6. **Sentence case everywhere.** Labels, badges and pills read "Duplicate", "Weak password",
   never all caps and never with an emoji.

## Colours

### Brand (accent only)

| Token | Value | Use |
| --- | --- | --- |
| `hv_brand_900` | `#07393A` | Pressed primary button, text on a `brand_100` background |
| `hv_brand_800` | `#0A5F5C` | Secondary button and link text, action icons, primary button hover |
| `hv_brand_700` | `#0E706B` | Primary button, selected pill, libadwaita accent, secret type icon |
| `hv_brand_600` | `#0D8F8A` | Focus ring, selected card, gauges |
| `hv_brand_200` | `#A4DFCF` | Hover on the selected sidebar item |
| `hv_brand_100` | `#D6F1E9` | Active toggle button background, category pill hover |
| `hv_brand_50` | `#EEF8F7` | Category, role and checked-chip backgrounds, information notes |

`hv_brand_500` (`#13A1A1`), `hv_brand_green` (`#1F8678`) and `hv_brand_300` (`#57B9B1`)
remain defined as a record of the logo scale, but no rule uses them: their contrast on
white is too low for text.

### Neutrals

| Token | Value | Use |
| --- | --- | --- |
| `hv_text` | `#1F2526` | Main text (14:1 on white) |
| `hv_text_muted` | `#586466` | Secondary text, hints, information icons (5.6:1) |
| `hv_text_subtle` | `#667375` | Tertiary text (4.6:1) |
| `hv_surface` | `#FFFFFF` | Cards, lists, fields, title bars |
| `hv_surface_subtle` | `#F3F5F5` | Neutral pills, read-only rows, generator panel |
| `hv_canvas` | `#FAFAFB` | Page background |
| `hv_border` | `#E4E7E8` | Card hairlines and separators |
| `hv_border_strong` | `#CDD3D5` | Field and secondary button borders, card hover |

### Semantic

| Family | Base | Text (`*_strong`) | Background (`*_bg`) |
| --- | --- | --- | --- |
| Success | `#178A64` | `#176C53` | `#DBF3E8` |
| Warning | `#B26A00` | `#805000` | `#FFF0D3` |
| Danger | `#C01C28` | `#A42625` | `#FFE5E3` |
| Information | `#2F6FDE` | `#1F4FA8` | `#E3EDFF` |

A semantic badge always pairs its `*_bg` background with its `*_strong` text. The `*_bright`
variants are for graphical indicators (gauges, colour dots), not text.

### Heelonys certification seal

The `hv_seal_*` tokens (dark background, gold trim) belong to the certification seal (sign-in
screen and main window header). It is the application's only dark and only gradient
surface, kept on purpose as a brand mark; do not reuse its tokens or gradients elsewhere.

### libadwaita accent

`00-tokens.css` maps `accent_bg_color`, `accent_fg_color` and `accent_color` to the brand.
From libadwaita 1.6 on, which follows the system accent, `accent-modern.css` sets the same
values as CSS variables: keep both files aligned.

## Typography

- System font (Cantarell on GNOME, Segoe UI on Windows): no font is bundled.
- Hierarchy through libadwaita classes (`title-1` to `title-4`, `heading`, `caption`,
  `dim-label`) rather than fixed sizes.
- Field label: 600, `hv_text`, 0.88rem (`field-label`). Hint: `hv_text_muted`, 0.86rem
  (`field-hint`). Error: `hv_danger`, 600 (`form-error`).
- Counters use tabular figures (`font-feature-settings: "tnum"`).
- Technical data (SSH keys, fingerprints) is set in `monospace`.

## Shapes and elevation

| Element | Radius | Border | Shadow |
| --- | --- | --- | --- |
| Card, boxed list, field, button | 12px | `hv_border` (field: `hv_border_strong`) | `0 1px 3px hv_shadow_soft` (cards and lists) |
| Compact list row | 10px | `hv_border` | same |
| Pill, badge, chip | 999px | none | none |
| Card hover | unchanged | `hv_border_strong` | `0 2px 8px hv_shadow_key` |

Hover never changes an element's geometry or position: only colour, border and shadow
change, over 150ms.

## Components

- **Primary button** (`suggested-action`, `primary-pill`): `hv_brand_700` background, white
  text, 700; hover `brand_800`, pressed `brand_900`. One per group or view.
- **Secondary button** (`text-button`, `secondary-pill`): white background,
  `hv_border_strong` border, `hv_brand_800` text.
- **Flat button** (`flat`): transparent, borderless; for card quick actions, `brand_50` on
  hover.
- **Destructive action** (`destructive-action`): `hv_danger_strong` background, white text;
  hover `hv_danger`. Never teal.
- **Back button**: always the `BackButton` widget (`ui/widgets/back_button.rs`), flat,
  `hv-back-symbolic` icon and label in `hv_brand_800`. Never an arrow in the text. Every page
  opened from the sidebar starts with a "Back, then left-aligned title" header.
- **Secret cards**: white, no stripe; the type is carried by the icon tint. The login and
  domain slots are always reserved so cards line up.
- **Sidebar**: flat `navigation-sidebar` style. Audit counters and status chips are hidden
  when they are zero or still loading.
- **Forms**: `adw::EntryRow`, `adw::PasswordEntryRow` and `boxed-list` lists; one white card
  per form, no nested frames.
- **Choice pills** (type, vault): neutral at rest, `hv_brand_700` with white text once
  selected. Chip-style check buttons (generator families) switch to a `hv_brand_700` border
  on a `hv_brand_50` background.
- **Inline states** (`inline-status-*`): neutral while loading, then a semantic pair
  (success or danger); never teal for a success.

## Accessibility

- WCAG 2.1 AA minimum contrast: 4.5:1 for text and meaningful icons, checked against the
  real background by the native tests (`theme_tests.rs`, `assert_text_contrast`).
- Visible focus everywhere: `0 0 0 3px alpha(hv_brand_600, 0.16)` ring and a `hv_brand_700`
  border on fields.
- Information never relies on colour alone: every badge has a label, and every state icon
  has a tooltip or adjacent text.

## Adding or changing a style

1. Add a missing colour as a token in `00-tokens.css`, never hard-coded in another sheet.
2. Put the rule in the stylesheet for its area (`assets/style/NN-*.css`); a new sheet must be
   imported in `style.css` and listed in `gresources.xml`.
3. Target a class owned by the component rather than one borrowed from another screen.
4. Run the native suite (`native_`): it fails on a GTK warning or insufficient contrast.
