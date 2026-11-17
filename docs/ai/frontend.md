# Frontend

## Web page architecture

- PopCircles web page is a Svelte application for exploring geographic maps and associated data.
- Keep UI components modular, semantic, and reusable.
- Separate presentation logic from data processing and map generation.

## Data contract

Python writes `gallery-data.json`; the Svelte app reads it at runtime. The `PageData`, `PageCountry`,
`PageFigure`, and caption `TypedDict`s in `python/src/population_circles/gallery.py` own the wire
shape. `frontend/src/gallery.js` mirrors those types for static checking. Keep required fields,
optional omitted fields, and nullable fields aligned in both places. Add generated-data assertions to
the Python Gallery tests when changing the contract. Do not add a second runtime schema or move map
generation into the frontend.

## UI/UX

- Use a viewport-contained application shell (`100dvh`).
- The document must not scroll vertically during normal application use.
- The main workspace owns primary vertical scrolling.
- Country navigation may scroll independently.
- Use native scrolling and browser controls.
- Keep the World result and every country result rendered in one continuously visible list.
- Preserve map aspect ratios and avoid layout shifts.
- Use responsive layouts based on available content width.
- Preserve light and dark theme support.

## Acceptance criteria

- At desktop and mobile viewport sizes, the page shell fits the viewport and the document has no
  vertical scrollbar; map results and country navigation scroll within their own regions.
- Selecting a country updates the URL fragment and scrolls the results region to that country's
  card. Direct fragments and browser back and forward restore the selected card; selecting World
  scrolls to the World result.
- On narrow screens, country navigation opens as a modal dialog, closes with Escape, and returns focus
  to its trigger. On wide screens, navigation can be collapsed and its state survives a reload.
- The theme control switches all page surfaces and text between readable light and dark palettes.
- All navigation, theme, and dialog actions work by keyboard, with visible focus and an
  accessible name or role for each control.

## Development Principles

- Follow existing Svelte conventions and project tooling.
- Prefer semantic HTML and native browser behavior.
- Avoid unnecessary dependencies and custom abstractions.
- Keep changes focused on the requested scope.
- Validate UI changes in the rendered application.
- Run relevant existing checks after modifications.
