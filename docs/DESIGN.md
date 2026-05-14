# flov — Design System

Premium, dense, single-pane desktop UI for a productivity tool.
Inspiration: Linear (calmer interface, dense hierarchy), Raycast (refined typography,
compact mode), shadcn (composable tokens), macOS Sonoma+ (system feel).
**Anti-pattern**: macOS Ventura System Settings — sparse, low density.

## Principles

1. **One pane, one task.** No tabs inside the settings window. Everything visible at once.
2. **Density via hierarchy, not cramming.** Use weight/size/color contrast — never <11px text.
3. **Hairline 1px borders + soft shadows.** No heavy chrome.
4. **Accent = action.** Everything else neutral grayscale; accent only on selected/primary.
5. **Hover = subtle bg tint** (4–8 % `fg` over `bg`). Never toggle borders on hover.
6. **Numbers tabular.** Sizes / counts / percentages in `font-variant-numeric: tabular-nums`.
7. **Theme-aware via CSS vars.** Set `:root` once, light defaults, override under `prefers-color-scheme: dark`.

## Color tokens

```
                  light             dark
--bg              #f7f7f8           #1c1c1e
--bg-elevated     #ffffff           #2c2c2e
--bg-subtle       #efeff2           #161618
--fg              #1c1c1e           #f5f5f7
--fg-muted        #6a6a70           #8e8e93
--fg-subtle       #a0a0a8           #6a6a70
--border          #e5e5ea           #3a3a3c
--border-strong   #d1d1d6           #48484a
--accent          #0a84ff           #0a84ff
--accent-fg       #ffffff           #ffffff
--accent-soft     rgba(10,132,255,.12)
--ok              #34c759           #30d158
--warn            #ff9f0a           #ff9f0a
--danger          #ff3b30           #ff453a
--track           #e5e5ea           #3a3a3c
```

Hover/active modifiers (computed via `color-mix` so they follow theme):
- `--hover-bg`: `color-mix(in srgb, var(--fg) 6%, transparent)`
- `--press-bg`: `color-mix(in srgb, var(--fg) 12%, transparent)`

## Type scale

```
caption    11px / 1.3  500 weight   uppercase, letter-spacing .6  → section headers, badges
meta       12px / 1.4  500          → tertiary text, hints
body       13px / 1.5  400          → default text, list rows
list-title 15px / 1.3  600          → primary item label inside a row
section    18px / 1.25 600          → zone headlines (MODELS / BACKEND / ACTIVITY)
page       24px / 1.2  700          → window title (rarely used; prefer titlebar)
counter    24px / 1.05 700  tabular → big stat numbers
```

Font stack: `-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif`.
Mono: `ui-monospace, SFMono-Regular, Menlo, "Cascadia Mono", monospace`.

## Spacing scale

```
4 8 12 16 20 24 32 48
```

Apply through `padding` / `gap` / `margin`. Avoid odd values.

## Radius scale

```
6   small (chip, badge)
8   button
10  card, list-row
12  window, toggle pill
999 round (radio, dot, switch)
```

## Component recipes

### Card
```css
background: var(--bg-elevated);
border: 1px solid var(--border);
border-radius: 10px;
padding: 12px 14px;
```

### Section block (named zone)
```html
<section class="zone">
  <h2>MODELS</h2>
  …
</section>
```
```css
.zone h2 { font: 600 11px/1 var(--font); letter-spacing: .6px; color: var(--fg-muted); }
```

### Compact row (model item, list cell)
```
height: 36–40 px
padding: 8 12
gap: 12
hover: --hover-bg
selected: 1px var(--accent) outline
```

### Radio card (backend pick)
```
height: 44 px
padding: 8 10
icon dot 14px on the left, label + sub stacked on right
disabled: opacity .55, cursor not-allowed
```

### Switch (toggle)
```
track: 38 × 22, radius 999, var(--track) → var(--accent) when on
knob: 18 px, white, 2 px inset shadow
transition: 120 ms ease-out
```

### Heatmap cell
```
size: 11 px (was 12 in v1)
gap: 2 px
radius: 2 px
levels: lvl-0 = --track; lvl-1..4 = mix(--accent, --track, 25/50/75/100)
```

### Counter tile
```css
padding: 10px 12px;
border-radius: 10px;
.num { font: 700 24px/1.05 var(--font); font-variant-numeric: tabular-nums; }
.cap { font: 500 11px/1 var(--font); text-transform: uppercase; letter-spacing: .6px; color: var(--fg-muted); }
```

## Layout grid (single-view settings window)

Default: 1180 × 800. Min: 960 × 640.

```
.settings {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  grid-template-rows: minmax(0, 1fr) auto;
  grid-template-areas:
    "models  right"
    "activity activity";
  gap: 16px;
  padding: 16px 20px 20px;
}
```

- **models** (top-left, takes most of the height) — Whisper variants list, future vendors stacked under
- **right** column (320px) — Backend card on top, Post-process card below
- **activity** (full-width bottom) — 4 counters in a row + heatmap

## Anti-patterns to avoid

- ❌ Multiple settings tabs / sidebar with 4+ items in a small window
- ❌ Big colored gradient header strips
- ❌ Card-in-card-in-card nesting
- ❌ Animated transitions longer than 200 ms inside a dense pane
- ❌ Per-component re-declared `:global(html, body)` — set once in the shell
