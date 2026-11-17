# Frontend

## Behaviour

### Viewport and scrolling

- The application shell fits the viewport (`100dvh`) on desktop and mobile, without a vertical document
  scrollbar during normal use.
- The main results workspace owns primary vertical scrolling; country navigation can scroll
  independently.
- Layout responds to available content width. Scrolling and browser controls use native behaviour.

### Navigation and history

- The World result and every country result remain rendered in one continuous list.
- Selecting a country updates the URL fragment and scrolls the results region to that country's card.
  Selecting World scrolls to the World result.
- Direct fragments and browser Back and Forward restore the selected card.

### Responsive navigation

- On narrow screens, country navigation opens as a modal dialog, closes with Escape, and returns focus
  to its trigger.
- On wide screens, navigation can be collapsed, and its state survives a reload.

### Map presentation

- Maps preserve their aspect ratios without layout shifts as images load.

Verification gap: existing assertions do not measure surrounding content before and after delayed
image loading.

### Themes

- The theme control switches all page surfaces and text between readable light and dark palettes.

Partial verification: selected colour assertions and screenshots do not establish readability of
every surface. Review both palettes, including contrast and focus visibility, when changing them.

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
- Keep UI components modular, semantic, and reusable.
- Separate presentation logic from data processing and map generation.

## Data contract

Python writes `gallery-data.json`; the Svelte app reads it at runtime. The `PageData`, `PageCountry`,
`PageFigure`, and caption `TypedDict`s in
[the Gallery producer](../../python/src/population_circles/gallery.py) own the wire shape.
[Frontend types](../../frontend/src/gallery.js) mirror those types for static checking. Keep required
fields, optional omitted fields, and nullable fields aligned in both places. Add generated-data
assertions to the [Python Gallery tests](../../python/tests/test_gallery.py) when changing the contract.
Do not add a second runtime schema or move map generation into the frontend.

## Contributing

Validate UI changes in the rendered application. Use semantic HTML and existing Svelte conventions.
[Platform guidance](platform.md#tooling-hierarchy) owns tooling and validation entry points.

### Finding verification

Tests that verify a Behaviour section use its anchor in their title, such as
`[navigation-and-history]`. Search from the repository root:

```sh
rg -F '[navigation-and-history]' frontend/test frontend/e2e
```

The token in a test failure points back to the section here. A test can reference more than one
section; a token locates related assertions, not proof that every requirement in the section is
covered. Additional implementation-level regression tests need no requirement token.

See the [unit tests](../../frontend/test/gallery.test.js),
[browser tests](../../frontend/e2e/gallery.spec.js), and
[browser configuration](../../frontend/playwright.config.js).

### Changing behaviour

Update the approved requirement, implementation, affected assertions, and verification notes together.
Resolve disagreements explicitly; do not rewrite intent merely because current code or a test differs.
Keep anchor-bearing headings stable. If an anchor changes, update links and test tokens together;
retire obsolete requirements and references together, leaving history in Git rather than a second
register. New repeatable interaction rules need focused assertions; manual checks and missing
automation remain explicit beside the requirement.
