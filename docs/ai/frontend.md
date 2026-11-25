# Frontend

## Behaviour

### Viewport and scrolling

- The application fits the viewport on desktop and mobile without a vertical document scrollbar
  during normal use.
- The main results workspace owns primary vertical scrolling; country navigation can scroll
  independently.
- Layout responds to available content width. Scrolling and browser controls use native behaviour.

### Navigation and history

- Selecting a country updates the URL fragment and scrolls to its result without removing other
  results. Selecting World scrolls to the World result.
- The A–Z index jumps within the country list to a letter represented by the current search results;
  unavailable letters are disabled. Jumps preserve the selected result, URL, and workspace scroll,
  and respect reduced-motion preferences.
- Activating the PopCircles logo clears the fragment and country selection, returns to the start of
  the results, and keeps the loaded page in place.
- Direct fragments and browser Back and Forward restore the selected card.

### Responsive navigation

- On narrow screens, country navigation opens as a modal dialog, closes with Escape, and returns focus
  to its trigger.
- On wide screens, navigation can be collapsed, and its state survives a reload.
- Icon-only desktop navigation controls explain their actions with tooltips on hover and keyboard focus; the buttons retain accessible names.
- The PopCircles logo is 2rem in the expanded desktop header and 1.25rem when collapsed; on mobile it stays 1.25rem in both states.

### Map presentation

- Maps preserve their aspect ratios without layout shifts as images load.
- The territory-coverage percentage explains that it measures land area, separately from population share.

Verification gap: existing assertions do not measure surrounding content before and after delayed
image loading.

### Themes

- The theme control switches all page surfaces and text between readable light and dark palettes.

Partial verification: token-pair contrast assertions and screenshots do not establish readability of
every rendered component. Review both palettes, including component states and focus visibility, when
changing them.

### Palette

[`app.css`](../../frontend/src/app.css) owns the exact theme colors. Keep its semantic tokens as the
single source; do not add component-local colors for page chrome. The visual direction is cool neutral
surfaces with restrained mint emphasis in dark mode and deep green emphasis in light mode. Map imagery
has its own data colors and is not part of the page palette.

| Token | Role |
| --- | --- |
| `--bg` | Page canvas |
| `--surface` | Primary workspace surfaces |
| `--surface-2` | Raised controls and supporting panels |
| `--surface-quiet` | Quiet inset panels |
| `--muted` | Quiet component surfaces |
| `--muted-foreground` | Supporting text |
| `--text` | Primary text on neutral surfaces |
| `--accent` | Brand emphasis and active or focused states |
| `--line` | Borders and dividers |
| `--glow` | Ambient page background |
| `--chrome-bg` | Translucent viewport chrome |
| `--shadow` | Raised-element shadow |
| `--destructive` | Destructive control background |
| `--destructive-foreground` | Text and icons on destructive controls |

shadcn-svelte's surface, text, border, and focus tokens map to this palette in `app.css`; keep the
light and dark values there.

When adding a color role, define its intentional light and dark values in `app.css` and update this
table. Do not derive a theme by inversion or choose colors by eye alone. Check rendered foreground and
background pairs in both themes: WCAG 2.2 AA requires 4.5:1 for normal text, 3:1 for large text, and
3:1 for visual information needed to identify controls and their states. Check focus indicators
against adjacent colors, and evaluate translucent surfaces after compositing. Never use color as the
only way to convey meaning. Automated checks cover semantic token pairs and worst-case chrome
compositing; inspect component-specific color combinations separately.

### Keyboard and accessibility

- All navigation, theme, and dialog actions work by keyboard, with visible focus and an accessible name
  or role for each control.

Partial verification: keyboard assertions cover selected flows, not every action or
assistive-technology usability. Follow the
[accessibility contributor expectations](../../ACCESSIBILITY.md#contributor-expectations) for manual
checks and their environments.

## Architecture boundaries

- Svelte presents generated Gallery data and maps within the
  [presentation boundary](../decisions/0019-svelte-presents-the-static-gallery.md).
- Keep UI components modular and reusable.
- Separate presentation logic from data processing and map generation.

### Semantic HTML

Prefer semantic HTML over generic containers. Preserve a logical heading hierarchy and use links,
buttons, forms, and dialogs according to their native purpose.

## Data contract

Python writes `gallery-data.json`; the Svelte app reads it at runtime. The `PageData`, `PageCountry`,
and `PageFigure` types in [Gallery page data](../../python/src/population_circles/gallery/page_data.py),
together with the caption types in
[Gallery rendering](../../python/src/population_circles/gallery/rendering.py), own the wire shape.
[Frontend types](../../frontend/src/lib/types/gallery.ts) mirror those types for static checking. Keep required
fields, optional omitted fields, and nullable fields aligned in both places. Add generated-data
assertions to the [Python Gallery page tests](../../python/tests/test_gallery_page.py) when changing
the contract.
Do not add a second runtime schema or move map generation into the frontend.

## Contributing

Validate UI changes in the rendered application. Follow existing Svelte conventions.
[Platform guidance](platform.md#tooling-hierarchy) owns tooling and validation entry points.

### Finding verification

Tests that verify a Behaviour section use its anchor in their title, such as
`[navigation-and-history]`. Search from the repository root:

```sh
rg -F '[navigation-and-history]' frontend/src frontend/e2e
```

The token in a test failure points back to the section here. A test can reference more than one
section; a token locates related assertions, not proof that every requirement in the section is
covered. Additional implementation-level regression tests need no requirement token.

See the [unit tests](../../frontend/src/lib/utils/gallery.unit.test.ts),
[component tests](../../frontend/src/lib/components/navigation/CountryNavigation.component.test.ts),
[browser tests and visual snapshots](../../frontend/e2e/gallery.spec.js),
[Vitest configuration](../../frontend/vitest.config.js), and
[Playwright configuration](../../frontend/playwright.config.js).

### Changing behaviour

Update the approved requirement, implementation, affected assertions, and verification notes together.
Resolve disagreements explicitly; do not rewrite intent merely because current code or a test differs.
Keep anchor-bearing headings stable. If an anchor changes, update links and test tokens together;
retire obsolete requirements and references together, leaving history in Git rather than a second
register. New repeatable interaction rules need focused assertions; manual checks and missing
automation remain explicit beside the requirement.

When a design change intentionally changes rendered screens, update and review the affected Playwright
screenshot snapshots as part of the change.
